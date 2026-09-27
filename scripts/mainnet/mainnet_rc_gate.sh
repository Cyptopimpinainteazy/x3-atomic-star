#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# mainnet_rc_gate.sh — the release-candidate gate for a mainnet launch.
#
# The body is a function (`mainnet_rc_gate`) so the readiness registry can cite
# it as `scripts/mainnet/mainnet_rc_gate.sh::mainnet_rc_gate`. The citation used
# to be the bare name `mainnet_rc_gate`, which resolved to nothing anywhere in
# the tree, so this row's test evidence had never been checked by anything.
#
# Every prerequisite is now resolved before the first one runs. This script used
# to call `scripts/testnet/generate_testnet_chain_spec.sh`, which did not exist,
# so the gate exited 127 on its third line and could never pass.
# `tests/test_rc_gates.py` drives this script and its testnet twin through a
# fixture root and asserts each one exits non-zero when a prerequisite fails.
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# The checks this gate runs, in order. Named here so the existence check and the
# run loop cannot drift apart.
MAINNET_RC_PREREQS=(
  scripts/mainnet/fresh_build_check.sh
  scripts/mainnet/panic_unwrap_audit.sh
  scripts/testnet/generate_testnet_chain_spec.sh
  scripts/testnet/testnet_genesis_lint.sh
  scripts/testnet/runtime_upgrade_rehearsal.sh
)

require_prereqs() {
  local missing=()
  local rel
  for rel in "${MAINNET_RC_PREREQS[@]}"; do
    [ -e "$ROOT_DIR/$rel" ] || missing+=("$rel")
  done
  if [ "${#missing[@]}" -gt 0 ]; then
    echo "FAILED: the mainnet RC gate needs ${#missing[@]} prerequisite(s) that do not exist:" >&2
    printf '  %s\n' "${missing[@]}" >&2
    exit 1
  fi
}

mainnet_rc_gate() {
  echo "== X3 MAINNET RC GATE =="

  # Increase stack to avoid rustc 1.88 LLVM ICE on cc crate.
  export RUST_MIN_STACK=16777216

  require_prereqs
  cd "$ROOT_DIR"

  # Mainnet gate must be harsher
  ./scripts/mainnet/fresh_build_check.sh || { echo "FAILED: fresh build check"; exit 1; }
  ./scripts/mainnet/panic_unwrap_audit.sh || { echo "FAILED: panic unwrap audit"; exit 1; }
  ./scripts/testnet/generate_testnet_chain_spec.sh || { echo "FAILED: chain spec generation"; exit 1; }
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

  echo "== X3 MAINNET RC GATE PASSED =="
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  mainnet_rc_gate "$@"
fi
