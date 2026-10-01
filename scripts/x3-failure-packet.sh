#!/usr/bin/env bash
# Wrap any X3 gate and turn a failure into a failure packet.
#
# The deterministic simulator emits packets for simulated failures. The same
# pattern has to cover the subsystems it cannot simulate yet — the atomic
# kernel, the settlement engine, the supply ledger, the runtime — where the
# failure arrives as a failing cargo test instead of a violated invariant.
#
# This wrapper runs the command, and on a non-zero exit extracts what a
# root-cause agent needs: the first error, the failing tests, the file/line
# each panic points at, the exact commit, and the replay command. Nothing is
# invented: a passing gate writes no packet, and fields that cannot be
# extracted are absent rather than guessed.
#
# Usage:
#   scripts/x3-failure-packet.sh --label pallet-x3-settlement-engine -- \
#       cargo test -p pallet-x3-settlement-engine
#   scripts/x3-failure-packet.sh --label runtime --root-cause -- \
#       cargo check -p x3-chain-runtime
#
# Exit code is the wrapped command's exit code, so this drops into CI directly.
set -uo pipefail

LABEL=""
ROOT_CAUSE=0
PACKET_DIR="${X3_FAILURE_PACKET_DIR:-failure-packets}"

while [ $# -gt 0 ]; do
  case "$1" in
    --label) LABEL="${2:?--label needs a value}"; shift 2 ;;
    --packet-dir) PACKET_DIR="${2:?--packet-dir needs a value}"; shift 2 ;;
    --root-cause) ROOT_CAUSE=1; shift ;;
    --) shift; break ;;
    -h|--help) sed -n '2,26p' "$0"; exit 0 ;;
    *) echo "x3-failure-packet: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

if [ -z "$LABEL" ] || [ $# -eq 0 ]; then
  echo "x3-failure-packet: usage: $0 --label <name> [--root-cause] -- <command...>" >&2
  exit 2
fi

# `X3_FAILURE_PACKET_ROOT` lets the wrapper run a gate in another checkout
# (for example a warm main tree while the wrapper itself is being reviewed in
# a worktree). Defaults to the repository containing the script.
ROOT="${X3_FAILURE_PACKET_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
cd "$ROOT"

LOG="$(mktemp -t x3-gate-log.XXXXXX)"
trap 'rm -f "$LOG"' EXIT

echo "x3-failure-packet: running [$LABEL]: $*"
started="$(date +%s)"
set +e
"$@" 2>&1 | tee "$LOG"
status="${PIPESTATUS[0]}"
set -e
elapsed="$(( $(date +%s) - started ))"

if [ "$status" -eq 0 ]; then
  echo "x3-failure-packet: [$LABEL] PASS in ${elapsed}s"
  exit 0
fi

mkdir -p "$PACKET_DIR"
PACKET_PATH="$(python3 - "$LABEL" "$status" "$LOG" "$PACKET_DIR" "$ROOT" "$@" <<'PY'
import hashlib
import json
import re
import shlex
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

label, status, log_path, packet_dir, root, *command = sys.argv[1:]
log = Path(log_path).read_text(errors="replace")
lines = log.splitlines()

def git(*args):
    try:
        return subprocess.run(["git", *args], cwd=root, capture_output=True, text=True, timeout=10).stdout.strip()
    except Exception:
        return ""

first_error = next(
    (line.strip() for line in lines
     if re.search(r"panicked at|^error(\[|:)| assertion|left == right|test result: FAILED", line)),
    lines[-1].strip() if lines else "",
)
failing_tests = sorted({
    match.group(1)
    for line in lines
    for match in [re.search(r"^test (.*) \.\.\. FAILED", line), re.search(r"^---- (.*) stdout ----", line)]
    if match
})
locations = []
for line in lines:
    for pattern in (r"panicked at ([^:]+):(\d+):(\d+)", r"--> ([^:]+):(\d+):(\d+)"):
        match = re.search(pattern, line)
        if match:
            locations.append({"file": match.group(1), "line": int(match.group(2))})
locations = [dict(pair) for pair in {tuple(item.items()) for item in locations}]
locations.sort(key=lambda item: (item["file"], item["line"]))

identity = f"{label}|{status}|{command}|{first_error}|{locations}|{failing_tests}"
failure_id = hashlib.sha256(identity.encode()).hexdigest()[:16]

packet = {
    "schema": "x3-gate-failure-packet-v1",
    "failure_id": failure_id,
    "gate": label,
    "producer": "scripts/x3-failure-packet.sh",
    "commit": git("rev-parse", "HEAD"),
    "branch": git("branch", "--show-current"),
    "worktree_dirty": bool(git("status", "--porcelain")),
    "stored_at": datetime.now(timezone.utc).isoformat(),
    # Saved with shell quoting, so the replay line runs the failing command
    # exactly even when an argument contains spaces.
    "command": shlex.join(command),
    "exit_code": int(status),
    "duration_seconds": None,
    "first_error": first_error,
    "failing_tests": failing_tests,
    "suspected_file": locations[0]["file"] if locations else "",
    "suspected_lines": sorted({item["line"] for item in locations}),
    "locations": locations,
    "log_excerpt": "\n".join(lines[-200:]),
    "replay_command": shlex.join(command),
    "next_step": "Verify the cause before changing code: rerun the replay command, confirm the failure, then run the same command after the fix.",
}

stem = f"{datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')}-{label}-{failure_id}"
json_path = Path(packet_dir) / f"{stem}.json"
json_path.write_text(json.dumps(packet, indent=2) + "\n")

markdown = [
    f"# Gate failure packet {failure_id}",
    "",
    f"- gate: `{label}`",
    f"- command: `{' '.join(command)}`",
    f"- exit code: {status}",
    f"- commit: `{packet['commit']}` on `{packet['branch']}` (dirty: {packet['worktree_dirty']})",
    f"- first error: {first_error}",
    "",
    "## Failing tests",
    "",
    *([f"- `{test}`" for test in failing_tests] or ["- (none parsed; see the log excerpt)"]),
    "",
    "## Suspected locations",
    "",
    *([f"- `{item['file']}:{item['line']}`" for item in locations] or ["- (no file:line in the output)"]),
    "",
    "## Repro",
    "",
    "```bash",
    packet["replay_command"],
    "```",
]
(Path(packet_dir) / f"{stem}.md").write_text("\n".join(markdown) + "\n")
print(json_path)
PY
)"

echo "x3-failure-packet: [$LABEL] FAIL (exit $status) — packet: $PACKET_PATH" >&2
echo "x3-failure-packet: replay: $*" >&2

if [ "$ROOT_CAUSE" -eq 1 ]; then
  if [ ! -f "$ROOT/crates/x3-sim/scripts/root_cause.py" ]; then
    echo "x3-failure-packet: root-cause dispatcher missing; packet kept, nothing dispatched" >&2
    exit "$status"
  fi
  python3 "$ROOT/crates/x3-sim/scripts/root_cause.py" "$PACKET_PATH" \
    --out "${X3_ROOT_CAUSE_DIR:-reports/root-cause}" || \
    echo "x3-failure-packet: root-cause dispatch failed; the packet above is still the evidence" >&2
fi

exit "$status"
