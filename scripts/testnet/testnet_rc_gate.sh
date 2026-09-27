#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# testnet_rc_gate.sh — the release-candidate gate for a testnet launch.
#
# Every check in the previous body ended in `|| true`, and the final line said
# "COMPLETED" rather than "PASSED", so this gate exited 0 no matter what failed —
# including the panic ratchet `scripts/mainnet/panic_unwrap_audit.sh` was fixed to
# return, and including `scripts/testnet/generate_testnet_chain_spec.sh`, which
# did not exist at all. `scripts/x3/yolo_autoprove.sh` runs this script, so the
# autoprove path was reporting an RC gate that could not fail.
#
# Every check now fails closed and names itself; the body is a function so the
# readiness registry can cite it as
# `scripts/testnet/testnet_rc_gate.sh::testnet_rc_gate`; and
# `tests/test_rc_gates.py` drives the function through a fixture root to prove a
# failing prerequisite reddens it.
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# The checks this gate runs, in order. Named here so the existence check and the
# run loop cannot drift apart.
TESTNET_RC_PREREQS=(
  scripts/mainnet/fresh_build_check.sh
  scripts/mainnet/panic_unwrap_audit.sh
  scripts/testnet/generate_testnet_chain_spec.sh
  scripts/testnet/testnet_genesis_lint.sh
  scripts/testnet/runtime_upgrade_rehearsal.sh
)

require_prereqs() {
  local missing=()
  local rel
  for rel in "${TESTNET_RC_PREREQS[@]}"; do
    [ -e "$ROOT_DIR/$rel" ] || missing+=("$rel")
  done
  if [ "${#missing[@]}" -gt 0 ]; then
    echo "FAILED: the testnet RC gate needs ${#missing[@]} prerequisite(s) that do not exist:" >&2
    printf '  %s\n' "${missing[@]}" >&2
    exit 1
  fi
}

testnet_rc_gate() {
  echo "== X3 TESTNET RC GATE =="

  # Increase stack to avoid rustc 1.88 LLVM ICE on cc crate.
  export RUST_MIN_STACK=16777216

  require_prereqs
  cd "$ROOT_DIR"

  # Testnet gate must run
  ./scripts/mainnet/fresh_build_check.sh || { echo "FAILED: fresh build check"; exit 1; }
  ./scripts/mainnet/panic_unwrap_audit.sh || { echo "FAILED: panic unwrap audit"; exit 1; }
  ./scripts/testnet/generate_testnet_chain_spec.sh || { echo "FAILED: testnet chain spec generation"; exit 1; }
  ./scripts/testnet/testnet_genesis_lint.sh || { echo "FAILED: genesis lint"; exit 1; }
  ./scripts/testnet/runtime_upgrade_rehearsal.sh || { echo "FAILED: runtime upgrade rehearsal"; exit 1; }
  cargo fmt --check || { echo "FAILED: code formatting"; exit 1; }
  cargo test -p pallet-x3-cross-vm-router -- --nocapture || { echo "FAILED: cross-vm-router tests"; exit 1; }
  cargo test -p pallet-x3-supply-ledger -- --nocapture || { echo "FAILED: supply-ledger tests"; exit 1; }
  cargo test -p pallet-x3-atomic-kernel -- --nocapture || { echo "FAILED: atomic-kernel tests"; exit 1; }
  cargo test -p x3-ixl -- --nocapture || { echo "FAILED: x3-ixl tests"; exit 1; }
  cargo test -p x3-proof -- --nocapture || { echo "FAILED: x3-proof tests"; exit 1; }
  cargo test -p x3-sidecar -- --nocapture || { echo "FAILED: x3-sidecar tests"; exit 1; }
  cargo test -p x3-gateway -- --nocapture || { echo "FAILED: x3-gateway tests"; exit 1; }
  cargo run -p x3-readiness -- testnet-report --out reports/testnet_readiness_report.md || { echo "FAILED: testnet report"; exit 1; }

  echo "== X3 TESTNET RC GATE PASSED =="
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  testnet_rc_gate "$@"
fi
