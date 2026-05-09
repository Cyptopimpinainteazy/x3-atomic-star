# CURRENT MAINNET STATUS

**Status:** ⚠️ MAINNET READINESS BLOCKED / NOT VERIFIED
**Overall Score:** N/A
**Last Verified Commit:** `2e0c3bdac9de8b60`
**Current Evidence:** mix of machine-generated RC-1 gate outputs and active gap/todo reports

---

## Status Summary
The internal RC-1 scope remains the intended launch boundary, but the current readiness evidence is not consistent enough to support a clean go/no-go decision.

- Some machine-generated artifacts still claim RC-1 readiness.
- Active gap gate outputs report unresolved S0/S1 issues.
- Placeholder readiness reports are present and must be regenerated.
- Public readiness messaging should be based on the canary plan and gate-state reconciliation.

---

## Current Facts
- The RC-1 feature set is the current launch scope:
  - X3Native + X3Evm + X3Svm internal domains
  - internal cross-VM routing and atomic kernel semantics
  - supply ledger enforcement with rollback/refund behavior
  - spot AMM / LP lock path only
- Bridges remain disabled-by-default until audit passage.
- PQ, AI, GPU acceleration, and advanced DEX are explicitly deferred.
- The proof gate infrastructure exists, but the outputs remain partially stale or contradictory.

---

## Current Blockers
The following categories currently block a credible mainnet readiness claim:

- Active gap gate failures in proof artifacts
- Missing or stale catastrophic receipts for core claims
- Placeholder readiness report outputs that have not been refreshed
- Contradictory internal status artifacts and unresolved evidence gaps

---

## Next steps
1. Re-run the readiness pipeline and regenerate all gate artifacts.
2. Reconcile the RC-1 report with active proof gap reports.
3. Publish a single canonical readiness scoreboard.
4. Use `docs/MAINNET_CANARY_PLAN.md`, `docs/MAINNET_READINESS_CHECKLIST.md`, and `.x3/X3_MAINNET_GATES.md` as the current sources of truth.

---

## Current Commands
```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo build --release -p x3-chain-node
cargo build --release -p x3-cli
cargo build --release -p x3-proof
cargo test -p pallet-x3-cross-vm-router
cargo test -p pallet-x3-supply-ledger
cargo test -p pallet-x3-atomic-kernel
cargo test -p x3-ixl
cargo test -p x3-proof
cargo run -p x3-readiness -- testnet-report --out reports/testnet_readiness_report.md
x3-proof mainnet-rc-report --out reports/mainnet_rc_report.md
```

---

## References
- `docs/MAINNET_CANARY_PLAN.md`
- `docs/MAINNET_READINESS_CHECKLIST.md`
- `docs/MAINNET_LAUNCH_CHECKLIST.md`
- `.x3/X3_MAINNET_GATES.md`
- `docs/MAINNET_READINESS_DELTA.md`
- `proof/reports/gap_gate_mainnet_20260426_194429.txt`
- `proof/reports/todo_gate_mainnet_20260426_194331.txt`
- `reports/testnet_readiness_report.md`

*Last updated: 2026-05-09*
*Source: manual reconciliation of active gate artifacts*
- ✅ Cross-thread visibility (S1)
- ✅ Governance bypass (S1)
- ✅ Unauthorized mint (S1)

---

## Fresh Verification Commands

```bash
# Format check
cargo fmt --all -- --check

# Compilation check
cargo check --workspace

# Run tests
cargo test --workspace

# Build binary
cargo build --release -p x3-chain-node

# Build CLI
cargo build --release -p x3-cli

# Build proof tool
cargo build --release -p x3-proof

# Run key pallet tests
cargo test -p pallet-x3-cross-vm-router
cargo test -p pallet-x3-supply-ledger
cargo test -p pallet-x3-atomic-kernel
cargo test -p x3-ixl
cargo test -p x3-proof

# Generate mainnet report
x3-proof mainnet-rc-report --out reports/mainnet_rc_report.md
```

---

## Launch Conditions

Launch is not authorized by the current evidence.

> **Scope note:** This report is scoped to internal v0.4 RC-1 readiness only. It does not imply public mainnet readiness for external gateways, PQ cryptography, advanced DEX, AI optimization, or GPU validator-critical paths.

**RC-1 Scope:** See [../MAINNET_RC1_SCOPE.md](../MAINNET_RC1_SCOPE.md)

**RC-1 Feature Debt:** See [../docs/RC1_FEATURE_DEBT.md](../docs/RC1_FEATURE_DEBT.md)

---

*Last updated: 2026-05-09*
*Source: manual reconciliation of active gate artifacts*
*Commit: 2e0c3bdac9de8b60*