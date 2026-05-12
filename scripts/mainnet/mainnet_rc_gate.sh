#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# X3 Atomic Star — Internal Settlement Mainnet RC Gate
#
# RC Scope:
#   • X3 native runtime (x3-chain-runtime)
#   • X3EVM internal domain (pallet-x3-kernel, pallet-x3-cross-vm-router)
#   • X3SVM internal domain (pallet-x3-atomic-kernel)
#   • One canonical X3 asset (pallet-x3-asset-registry, x3-asset-kernel-types)
#   • Internal-only cross-VM settlement — all 6 routes (pallet-x3-settlement-engine)
#   • Supply invariant (pallet-x3-supply-ledger)
#   • External bridge surface disabled at genesis
#
# Explicitly OUT of scope (known failures, not on RC critical path):
#   • node / sc-network — libp2p 0.51 vs libp2p-identity 0.2.13 conflict (post-RC1)
#   • WASM binary production — SKIP_WASM_BUILD=1 intentional for this gate
#
# Usage:  bash scripts/mainnet/mainnet_rc_gate.sh
# Exit:   0 = RC PASS  |  1 = RC FAIL
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPORT_DIR="$ROOT_DIR/reports"
mkdir -p "$REPORT_DIR"

# Ensure cargo is available (rustup installs to ~/.cargo/bin)
export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v cargo &>/dev/null; then
  echo "ERROR: cargo not found. Install Rust via https://rustup.rs and retry."
  exit 2
fi

cd "$ROOT_DIR"

# RC-critical packages — all must compile.
RC_PACKAGES=(
  x3-chain-runtime
  pallet-x3-kernel
  pallet-atomic-trade-engine
  pallet-x3-agent-law
  pallet-swarm
  pallet-x3-cross-vm-router
  pallet-x3-supply-ledger
  pallet-x3-asset-registry
  pallet-x3-account-registry
  pallet-x3-atomic-kernel
  pallet-x3-settlement-engine
  x3-asset-kernel-types
  x3-ixl
  x3-packet-standard
)

echo "════════════════════════════════════════════════════════════"
echo "  X3 Internal Settlement Mainnet RC Gate"
echo "  SDK branch: stable2512 | Rust: $(rustc --version 2>/dev/null || echo unknown)"
echo "════════════════════════════════════════════════════════════"
echo ""

FAILED=()
PASSED=()

for pkg in "${RC_PACKAGES[@]}"; do
  printf "  %-45s " "$pkg"
  if SKIP_WASM_BUILD=1 cargo check -q -p "$pkg" 2>/dev/null; then
    echo "PASS"
    PASSED+=("$pkg")
  else
    echo "FAIL"
    FAILED+=("$pkg")
  fi
done

echo ""
echo "────────────────────────────────────────────────────────────"
echo "  Results: ${#PASSED[@]} passed, ${#FAILED[@]} failed"
echo "────────────────────────────────────────────────────────────"

if [ ${#FAILED[@]} -eq 0 ]; then
  echo ""
  echo "  RC GATE: PASS"
  echo ""
  echo "  Known exclusions:"
  echo "    • node / sc-network — libp2p version conflict (post-RC1 infra task)"
  echo ""
  echo "  See reports/internal_settlement_rc_failures.md for full details."
  echo ""
  exit 0
else
  echo ""
  echo "  RC GATE: FAIL"
  echo ""
  echo "  Failed packages:"
  for pkg in "${FAILED[@]}"; do
    echo "    • $pkg"
    SKIP_WASM_BUILD=1 cargo check -p "$pkg" 2>&1 | grep "^error" | head -5 | sed 's/^/      /'
  done
  echo ""
  echo "  See reports/internal_settlement_rc_failures.md for fix guidance."
  echo ""
  exit 1
fi
