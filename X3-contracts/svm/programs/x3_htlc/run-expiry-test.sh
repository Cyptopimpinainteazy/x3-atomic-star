#!/usr/bin/env bash
# Clock-warped expiry suite for `x3_htlc` (see tests-live/tests/expiry_lifecycle.rs).
#
# The suite runs the real compiled SBF artifact inside `solana-program-test`, so
# it needs the artifact built first: this wrapper builds it with
# `cargo build-sbf` and points `BPF_OUT_DIR` at the directory the toolchain
# writes it to. Nothing here falls back to a native stand-in — if the artifact
# is missing, the program id has no executable program and the tests fail.
#
# Usage: bash X3-contracts/svm/programs/x3_htlc/run-expiry-test.sh
# Requires: the Solana toolchain (`cargo build-sbf`) on PATH.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# `x3_htlc` is a member of the nested `X3-contracts/svm` workspace, so the SBF
# build writes into that workspace's `target/deploy`.
SVM_WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TEST_TARGET_DIR="${X3_HTLC_EXPIRY_TARGET_DIR:-/tmp/x3-htlc-expiry-tests}"

command -v cargo-build-sbf >/dev/null 2>&1 || {
  echo "Error: cargo-build-sbf not installed (Solana toolchain)"
  exit 1
}

echo "=== build the SBF program ==="
# Unset an ambient CARGO_TARGET_DIR so the artifact lands where we look for it.
( cd "$SCRIPT_DIR" && env -u CARGO_TARGET_DIR cargo build-sbf 2>&1 | tail -3 )

BPF_OUT_DIR="$SVM_WORKSPACE_ROOT/target/deploy"
[ -f "$BPF_OUT_DIR/x3_htlc.so" ] || {
  echo "Error: $BPF_OUT_DIR/x3_htlc.so missing after cargo build-sbf"
  exit 1
}

echo "=== run the clock-warped expiry suite ==="
cd "$SCRIPT_DIR/tests-live"
BPF_OUT_DIR="$BPF_OUT_DIR" CARGO_TARGET_DIR="$TEST_TARGET_DIR" \
  cargo test --locked
