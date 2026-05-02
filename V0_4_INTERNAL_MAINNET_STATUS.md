# v0.4 Internal Mainnet RC-1 — Status

> **Status:** ✅ GO FOR MAINNET RC-1 (v0.4 Internal-Only)
> **Score:** 100% | **S0 Verified:** 16/16 | **Blockers:** 0
> **Commit:** `2e0c3bdac9de8b60`
> **Report:** [launch-gates/reports/X3-MAINNET-GO-NO-GO-20260501-203300.md](../launch-gates/reports/X3-MAINNET-GO-NO-GO-20260501-203300.md)

---

## ✅ RC-1 Gates: ALL PASSED

All ProofForge gates verified and passed as of 2026-05-02.

### ProofForge Results

| Gate | Status | Score |
|------|--------|-------|
| SecurityGate | ✅ PASS | 16/16 S0 verified |
| MainnetGate | ✅ PASS | 100% |
| GapGate | ✅ PASS | 0 gaps |
| TodoGate | ✅ PASS | 0 stale |

---

## 📦 What's Included in RC-1

### Core Components

1. **`crates/x3-readiness-report`** - Tri-state readiness checks
   - Pass/Fail/Unknown with honest gap reporting
   - JSON-RPC collector with 1.5s timeout
   - 10 unit tests for offline behavior

2. **`crates/x3-packet-standard`** - IBC-ICS-04-style packet lifecycle
   - `MAX_PAYLOAD = 64 KiB`
   - Domain-separated commitments (blake2b256)
   - `ReplayGuard` for idempotent re-receipt
   - `TimeoutPolicy` with height-then-timestamp
   - 11 unit + 6 property tests

3. **`crates/x3-ixl`** - Minimal Instruction eXecution Layer (8 opcodes)
   - Opcodes: Lock, Mint, Burn, Swap, Settle, EmitProof, Refund, Abort
   - `MAX_BUNDLE = 64`
   - `Planner` validates DAG constraints
   - `Interpreter` executes against host swap function
   - `Rollback` from partial receipts
   - 12 unit + 4 property tests

4. **`pallet-x3-cross-vm-router`** - Router with scope freeze
   - `ExternalBridgesEnabled: bool` storage (default `false`)
   - Kill-switch: returns `Error::ExternalBridgesDisabled` unless governance enables
   - Full lib suite: 18/18 passing

### Critical Path Tests (48/48 passing)

- `x3-packet-standard`: 17 tests (11 unit + 6 property)
- `x3-ixl`: 16 tests (12 unit + 4 property)
- `x3-readiness-report`: 10 tests
- `pallet-x3-cross-vm-router`: 5 scope-freeze tests

---

## 🚫 What's NOT Included (Feature-Gated)

- External bridge extrinsics (Phase-C stubs, disabled by kill-switch)
- Runtime API `canonical_ledger_reconcile` (follow-up)
- Benchmark weights for planned router-pallet entry points
- Public testnet relayer paths (audit required separately)

---

## 🔧 Verification Commands

```bash
# Check workspace compiles
cargo check --workspace --all-targets

# Run critical path tests (48/48)
cargo test --package x3-packet-standard
cargo test --package x3-ixl
cargo test --package x3-readiness-report
cargo test --package pallet-x3-cross-vm-router

# Run full router suite (18/18)
cargo test -p pallet-x3-cross-vm-router --lib

# Run CI gate locally
# See: .github/workflows/v04-ship-gate.yml
```

---

## 📁 Key Files

- **CI Gate:** [.github/workflows/v04-ship-gate.yml](.github/workflows/v04-ship-gate.yml)
- **RC-1 Scope:** [MAINNET_RC1_SCOPE.md](../MAINNET_RC1_SCOPE.md)
- **Full Status:** [docs/CURRENT_MAINNET_STATUS.md](../docs/CURRENT_MAINNET_STATUS.md)

---

## 📋 Post-RC-1 Follow-ups

See [GAPS_REPORT_2026_04_27.md](GAPS_REPORT_2026_04_27.md) for tracked items:
- Legacy router failures
- Relayer bypass audit
- Reconciliation runtime API

---

**Last Updated:** 2026-05-02
**Status:** ✅ GO FOR RC-1