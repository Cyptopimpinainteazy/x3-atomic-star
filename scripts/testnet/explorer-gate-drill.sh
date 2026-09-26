#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# scripts/testnet/explorer-gate-drill.sh
#
# Criterion 14 of the public testnet gate ("explorer/dashboard reachable") is the one
# criterion that depends on something outside the chain, and it has been wrong in both
# directions on this box:
#
#   * it passed while nothing served a block explorer — any HTTP service on 3000/3001/8080
#     satisfied it, including the wallet app's own `next dev`; the recorded "14 of 15 PASS"
#     run was one of those.
#   * it then failed on a healthy seven-validator network, because `apps/explorer` exists but
#     nothing starts it. A criterion that is red for a reason the gate cannot fix is a
#     criterion an operator learns to ignore.
#
# This drill makes the criterion mean something in both directions and leaves the evidence:
#
#   1. decoy    — a plain HTTP server on the pinned port answers 200 with a page that is not
#                 an explorer. Criterion 14 must FAIL, and say the URL answered without
#                 identifying itself.
#   2. positive — the real `apps/explorer` (Next.js) serves on the same port. Criterion 14
#                 must PASS, and the report must name the URL it reached.
#
# It does not boot a chain: the other fourteen criteria are not this drill's business, and the
# run is expected to end `public_testnet_gate: FAIL` for reasons that have nothing to do with
# the explorer. What is asserted is criterion 14's row in reports/public_testnet_gate.md, both
# times, plus the exit code of each phase (decoy must fail the gate, positive need not pass it).
#
# Usage:
#   bash scripts/testnet/explorer-gate-drill.sh [--port 3410] [--keep]
#
# Exit 0 -> criterion 14 refused the decoy and passed on the real explorer, naming it.
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PORT=3410
KEEP=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --port) PORT="$2"; shift 2 ;;
        --keep) KEEP=1; shift ;;
        -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

EXPLORER_URL="http://127.0.0.1:${PORT}"
REPORT="$ROOT_DIR/reports/public_testnet_gate.md"
REPORT_BEFORE="$(mktemp)"
WORK_DIR="$(mktemp -d)"
DECOY_PID=""
EXPLORER_PID=""

# Keep the operator's report: this drill overwrites reports/public_testnet_gate.md twice.
if [[ -f "$REPORT" ]]; then cp "$REPORT" "$REPORT_BEFORE"; fi

cleanup() {
    local status=$?
    [[ -n "$DECOY_PID" ]] && kill "$DECOY_PID" 2>/dev/null || true
    # `npx next start` is a shell that execs a child, and it was the child (`next-server`) that
    # held the port: killing the pid we recorded left a listener behind, and the next run then
    # failed its own port preflight. The server is started with `setsid`, so its pid is its
    # process-group id and killing the group takes the whole tree.
    [[ -n "$EXPLORER_PID" ]] && kill -TERM -"$EXPLORER_PID" 2>/dev/null || true
    [[ -n "$EXPLORER_PID" ]] && kill "$EXPLORER_PID" 2>/dev/null || true
    pkill -f -- "http.server ${PORT}" 2>/dev/null || true
    if [[ -s "$REPORT_BEFORE" ]]; then cp "$REPORT_BEFORE" "$REPORT"; fi
    rm -f "$REPORT_BEFORE"
    if [[ "$KEEP" == "1" ]]; then
        echo "kept: $WORK_DIR"
    else
        rm -rf "$WORK_DIR"
    fi
    exit "$status"
}
trap cleanup EXIT

say()  { echo "  $*"; }
die()  { echo "FAIL: $*" >&2; exit 1; }

# The same port must be free before either phase starts: a leftover listener is exactly the
# confusion the observability check hit on 2026-09-26 (two runs, one network, a PASS that
# scraped the other run's nodes).
if command -v ss >/dev/null 2>&1 && ss -ltn 2>/dev/null | grep -q ":${PORT}\b"; then
    die "port ${PORT} is already in use (the drill cannot tell its own server from a leftover)"
fi

# criterion_row <artifact> -> the row 14 line of the report
criterion_row() {
    grep -E '^\| 14 \|' "$1" || die "no criterion 14 row in the report"
}

# run_gate <basename> — runs the gate and keeps both its terminal output (which is where the
# per-URL reasoning goes: "[info] … does not identify itself as the X3 explorer") and its report
# (which is where criterion 14's verdict goes).
run_gate() {
    local base="$1"
    # An unreachable RPC keeps the chain criteria deterministic: they are expected to fail,
    # and this drill only reads criterion 14.
    X3_EXPLORER_URL="$EXPLORER_URL" \
    X3_RPC_URL="http://127.0.0.1:9" \
    X3_TESTNET_HOURS=0 \
        bash "$ROOT_DIR/scripts/mainnet/public_testnet_gate.sh" > "$base.log" 2>&1 || true
    cp "$REPORT" "$base"
}

echo "explorer-gate-drill: decoy phase (a plain HTTP server must not satisfy criterion 14)"
cat > "$WORK_DIR/index.html" <<'HTML'
<!doctype html><title>Not an explorer</title><h1>hello</h1>
HTML
(cd "$WORK_DIR" && exec python3 -m http.server "$PORT" --bind 127.0.0.1) > "$WORK_DIR/decoy.log" 2>&1 &
DECOY_PID=$!
for _ in $(seq 1 20); do
    curl -sf -m 2 "$EXPLORER_URL" >/dev/null 2>&1 && break
    sleep 0.5
done
curl -sf -m 5 "$EXPLORER_URL" >/dev/null || die "the decoy server never came up on ${PORT}"
say "decoy answering on $EXPLORER_URL with a page that is not an X3 explorer"

run_gate "$WORK_DIR/decoy-report.md"
DECOY_ROW="$(criterion_row "$WORK_DIR/decoy-report.md")"
say "report row: $DECOY_ROW"
grep -q '| FAIL' <<<"$DECOY_ROW" || die "criterion 14 PASSED on a decoy HTTP server: ${DECOY_ROW}"
grep -q 'does not identify itself as the X3 explorer' "$WORK_DIR/decoy-report.md.log" \
    || die "the gate failed the decoy without saying why (expected a line about the body not identifying itself)"
say "the gate said why: $(grep -m1 'does not identify itself as the X3 explorer' "$WORK_DIR/decoy-report.md.log" | sed 's/^ *//')"
kill "$DECOY_PID" 2>/dev/null || true
wait "$DECOY_PID" 2>/dev/null || true
DECOY_PID=""
sleep 1
say "criterion 14 refused the decoy ✔"

echo "explorer-gate-drill: positive phase (the real apps/explorer must satisfy it, and be named)"
if [[ ! -f "$ROOT_DIR/apps/explorer/package.json" ]]; then
    die "apps/explorer is missing"
fi
if [[ ! -d "$ROOT_DIR/apps/explorer/.next" ]]; then
    say "no .next build; running 'npm run build' in apps/explorer (this takes a few minutes)"
    (cd "$ROOT_DIR/apps/explorer" && npm run build) > "$WORK_DIR/explorer-build.log" 2>&1 \
        || die "apps/explorer failed to build — see $WORK_DIR/explorer-build.log"
fi
# `npx next start` is a shell that execs `next-server`, and it is the child that holds the
# port — killing the captured pid alone left a listener behind and the next run failed its own
# port preflight. Start the server in its own session so cleanup can signal the whole group.
setsid bash -c "cd '$ROOT_DIR/apps/explorer' && exec npx --no-install next start -p '$PORT' -H 127.0.0.1" \
    > "$WORK_DIR/explorer.log" 2>&1 &
EXPLORER_PID=$!

BODY=""
for _ in $(seq 1 60); do
    BODY="$(curl -sf -m 3 "$EXPLORER_URL" 2>/dev/null || true)"
    if grep -qiE "X3 Chain Explorer|X3 Chain Block Explorer|Block explorer for X3" <<<"$BODY"; then
        break
    fi
    BODY=""
    sleep 1
done
if [[ -z "$BODY" ]]; then
    die "apps/explorer never served a body identifying as the X3 explorer on ${PORT} (log: $WORK_DIR/explorer.log)"
fi
say "apps/explorer answering on $EXPLORER_URL ($(wc -c <<<"$BODY" | tr -d '[:space:]') bytes)"

run_gate "$WORK_DIR/explorer-report.md"
PASS_ROW="$(criterion_row "$WORK_DIR/explorer-report.md")"
say "report row: $PASS_ROW"
grep -q '| PASS' <<<"$PASS_ROW" || die "criterion 14 did not PASS on the real explorer: ${PASS_ROW}"
grep -qF "$EXPLORER_URL" <<<"$PASS_ROW" \
    || die "criterion 14 passed without naming the explorer it reached: ${PASS_ROW}"
say "criterion 14 passed and named $EXPLORER_URL ✔"

# Proof artifacts, so the claim does not live only in this terminal (AGENTS.md: evidence that
# survives the run). Written outside the operator's working set.
ARTIFACT_DIR="$ROOT_DIR/.ai/runlogs/explorer-gate-drill-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$ARTIFACT_DIR"
cp "$WORK_DIR/decoy-report.md" "$WORK_DIR/decoy-report.md.log" \
   "$WORK_DIR/explorer-report.md" "$WORK_DIR/explorer-report.md.log" "$ARTIFACT_DIR/"
{
    echo "# explorer-gate-drill — $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo
    echo "Criterion 14 of the public testnet gate, both directions, on the bytes in this tree."
    echo
    echo "- decoy (plain \`python3 -m http.server\` on ${EXPLORER_URL}): criterion 14 = FAIL"
    echo "  \`${DECOY_ROW}\`"
    echo "- apps/explorer (Next.js, \`.next\` build in the tree): criterion 14 = PASS and named"
    echo "  \`${PASS_ROW}\`"
} > "$ARTIFACT_DIR/summary.md"

echo
echo "explorer-gate-drill: PASS"
echo "  decoy    (not an explorer) -> FAIL, as required"
echo "  positive (apps/explorer)   -> PASS, named in the report"
echo "  evidence: $ARTIFACT_DIR/"
