#!/usr/bin/env bash
# Self-test for the gate failure-packet wrapper.
#
# Proves both directions: a failing gate produces a packet whose extracted
# file/line/error match the failure, and a passing gate produces no packet at
# all. Without the second assertion the wrapper could pass by writing packets
# unconditionally, which would make every future packet worthless.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

WORK="$(mktemp -d -t x3-failure-packet-test.XXXXXX)"
trap 'rm -rf "$WORK"' EXIT

fail=0

echo "test_x3_failure_packet: failing gate must produce a packet"
set +e
bash scripts/x3-failure-packet.sh --label selftest-fail --packet-dir "$WORK/fail-packets" -- \
  bash -c 'printf "running 1 test\n"; printf "thread '"'"'main'"'"' panicked at pallets/x3-supply-ledger/src/lib.rs:412:9:\nassertion `left == right` failed\n"; exit 101' >/dev/null 2>&1
status=$?
set -e

if [ "$status" -ne 101 ]; then
  echo "  FAIL: wrapper exit was $status, expected the wrapped command's 101" >&2
  fail=1
fi

packet="$(find "$WORK/fail-packets" -name '*.json' | head -1)"
if [ -z "$packet" ]; then
  echo "  FAIL: no packet written for a failing gate" >&2
  fail=1
else
  python3 - "$packet" <<'PY' || fail=1
import json
import sys

packet = json.load(open(sys.argv[1]))
assert packet["schema"] == "x3-gate-failure-packet-v1", packet["schema"]
assert packet["exit_code"] == 101, packet["exit_code"]
assert packet["suspected_file"] == "pallets/x3-supply-ledger/src/lib.rs", packet["suspected_file"]
assert 412 in packet["suspected_lines"], packet["suspected_lines"]
assert "panicked at" in packet["first_error"], packet["first_error"]
assert packet["commit"], "the packet must pin the commit it ran on"
print("  packet fields verified")
PY
fi

echo "test_x3_failure_packet: passing gate must write no packet"
bash scripts/x3-failure-packet.sh --label selftest-pass --packet-dir "$WORK/pass-packets" -- \
  bash -c 'exit 0' >/dev/null 2>&1
if [ -d "$WORK/pass-packets" ] && [ -n "$(find "$WORK/pass-packets" -type f 2>/dev/null)" ]; then
  echo "  FAIL: a passing gate wrote a packet" >&2
  fail=1
fi

echo "test_x3_failure_packet: usage error must be exit 2"
set +e
bash scripts/x3-failure-packet.sh --label missing-command >/dev/null 2>&1
status=$?
set -e
if [ "$status" -ne 2 ]; then
  echo "  FAIL: usage error exit was $status, expected 2" >&2
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  echo "test_x3_failure_packet: FAILED" >&2
  exit 1
fi
echo "test_x3_failure_packet: OK"
