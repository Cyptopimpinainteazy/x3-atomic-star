#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

TOOLCHAIN="${X3_CARGO_TOOLCHAIN:-1.90.0}"

resolve_bin() {
  local explicit="$1"
  local fallback_one="$2"
  local fallback_two="$3"

  if [[ -n "$explicit" ]]; then
    echo "$explicit"
    return 0
  fi

  if command -v "$fallback_one" >/dev/null 2>&1; then
    command -v "$fallback_one"
    return 0
  fi

  if [[ -x "$fallback_two" ]]; then
    echo "$fallback_two"
    return 0
  fi

  return 1
}

CARGO_BIN="$(resolve_bin "${X3_CARGO_BIN:-}" "cargo" "/home/lojak/.cargo/bin/cargo" || true)"
RUSTC_BIN="$(resolve_bin "${RUSTC_BIN:-}" "rustc" "/home/lojak/.cargo/bin/rustc" || true)"

if [[ -z "$CARGO_BIN" ]]; then
  echo "[x3] ERROR: cargo binary not found. Set X3_CARGO_BIN=/absolute/path/to/cargo" >&2
  exit 127
fi

if [[ -z "$RUSTC_BIN" ]]; then
  echo "[x3] ERROR: rustc binary not found. Set RUSTC_BIN=/absolute/path/to/rustc" >&2
  exit 127
fi

CARGO_CMD=("$CARGO_BIN" "+$TOOLCHAIN")
RUSTC_CMD=("$RUSTC_BIN" "+$TOOLCHAIN")

TMP_ROOT="${TMPDIR:-$ROOT_DIR/.tmp}"
mkdir -p "$TMP_ROOT"

run() {
  echo "==> $*"
  "$@"
}

echo "[x3] Balance reconciliation verification starting"

echo "[x3] Step 1/4: supply-ledger unit tests"
run "${CARGO_CMD[@]}" test -p pallet-x3-supply-ledger --tests -- --test-threads=1

echo "[x3] Step 2/4: kernel reconciliation-focused tests"
run "${CARGO_CMD[@]}" test -p pallet-x3-kernel test_global_supply_reconciliation -- --test-threads=1
run "${CARGO_CMD[@]}" test -p pallet-x3-kernel test_emergency_reconciliation -- --test-threads=1

echo "[x3] Step 3/4: deterministic invariant fuzz harness"
HARNESS_SRC="tests_phase4/invariant_registry_check.rs"
HARNESS_BIN="$TMP_ROOT/x3_invariant_registry_check"
run "${RUSTC_CMD[@]}" --test "$HARNESS_SRC" -o "$HARNESS_BIN"
run "$HARNESS_BIN" --exact canonical_supply_invariant_fuzz_10k_random_transactions

echo "[x3] Step 4/4: readiness report (deterministic offline snapshot)"
REPORT_DIR="$ROOT_DIR/reports"
mkdir -p "$REPORT_DIR"
READINESS_JSON="$REPORT_DIR/sprint0_readiness.json"
READINESS_TEXT="$REPORT_DIR/sprint0_readiness.txt"

"${CARGO_CMD[@]}" run -p x3-readiness-report --bin readiness-cli -- --offline --json > "$READINESS_JSON"
"${CARGO_CMD[@]}" run -p x3-readiness-report --bin readiness-cli -- --offline --text > "$READINESS_TEXT"

echo "[x3] Readiness artifacts written:"
echo "  - $READINESS_JSON"
echo "  - $READINESS_TEXT"

echo "[x3] Balance reconciliation verification complete"
