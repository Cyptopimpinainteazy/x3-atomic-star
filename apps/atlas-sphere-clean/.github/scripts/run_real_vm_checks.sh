#!/bin/bash
set -euo pipefail

echo "==> Running real-VM integration checks"

echo "-- Rust toolchain --"
rustc --version || true

echo "-- Add wasm target --"
rustup target add wasm32-unknown-unknown || true

echo "-- Check wasm-opt --"
if ! command -v wasm-opt &>/dev/null; then
  echo "wasm-opt not found (install binaryen)"; exit 1
fi

echo "-- Cargo check (selected crates with std) --"
cargo check -p pallet-x3-kernel -p evm-integration -p runtime --features std

echo "-- Build runtime WASM (no_std compatibility) --"
cd runtime
cargo build --release --target wasm32-unknown-unknown --no-default-features
cd -

echo "-- Run unit tests for key crates (std) --"
cargo test -p runtime --features std --no-fail-fast
cargo test -p pallet-x3-kernel --features std --no-fail-fast
cargo test -p evm-integration --features std --no-fail-fast

if [ "${ALLOW_VM_TEST_FAILURES:-0}" = "1" ]; then
  echo "!! ALLOW_VM_TEST_FAILURES=1 set; VM integration test failures will not fail this job"
  cargo test -p svm-integration --features std --no-fail-fast || true
  cargo test -p x3-integration --features std --no-fail-fast || true
else
  cargo test -p svm-integration --features std --no-fail-fast
  cargo test -p x3-integration --features std --no-fail-fast
fi

echo "-- Optionally run full test script (may need additional packages) --"
if [ -f ./RUN_ALL_TESTS.sh ]; then
  if [ "${ALLOW_VM_TEST_FAILURES:-0}" = "1" ]; then
    ./RUN_ALL_TESTS.sh || true
  else
    ./RUN_ALL_TESTS.sh
  fi
fi

echo "==> Real-VM checks completed"
