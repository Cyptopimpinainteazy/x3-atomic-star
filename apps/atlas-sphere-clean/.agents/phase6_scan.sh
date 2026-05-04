#!/bin/bash
set -euo pipefail

REPO=/home/lojak/Desktop/x3-chain-master

echo "=== PRODUCTION PANIC SCAN - Phase 6 ==="
echo ""

# Scan crates + node + runtime (exclude test files)
echo "--- node/ runtime/ crates/ ---"
grep -rn '\.unwrap()\|\.expect(\|panic!(' \
  "$REPO/node/src/" "$REPO/runtime/src/" "$REPO/crates/" \
  --include="*.rs" \
  | grep -v '^\s*//' \
  | grep -vE 'tests\.rs:|test_fixtures|/tests/|_test\.rs:|mock\.rs:|benchmarking\.rs:' \
  | grep -vE '#\[cfg\(test\)\]' \
  || echo "(none)"

echo ""
echo "--- pallets/ ---"
grep -rn '\.unwrap()\|\.expect(\|panic!(' \
  "$REPO/pallets/" \
  --include="*.rs" \
  | grep -v '^\s*//' \
  | grep -vE 'tests\.rs:|mock\.rs:|benchmarking\.rs:' \
  | grep -vE '#\[cfg\(test\)\]' \
  || echo "(none)"

echo ""
echo "=== SCAN COMPLETE ==="
