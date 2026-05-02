# Mainnet RC-1 Readiness Report

> **Status:** ⚠️  GATES PARTIALLY VERIFIED — live node required for PASS on RPC gates  
> **Generated:** offline (compile-time evidence + in-process test evidence)  
> **Scope:** Internal atomic execution only — external gateway NOT in scope  

---

## Critical Path Gates

| Gate | Status | Evidence |
|---|---|---|
| `ixl_bundle_gate` | ✅ PASS (compile) | `x3-ixl` planner + interpreter wired in cross-vm-router; compile_error! guards active |
| `packet_lifecycle_gate` | ✅ PASS (compile) | `x3-packet-standard` replay + commitment + timeout wired in cross-vm-router |
| `liquidity_core_gate` | ✅ PASS (compile) | `x3-liquidity-core` settlement bounds validated; Settlement::build wired |
| `external_bridges_disabled` | ✅ PASS (storage default) | `ExternalBridgesEnabled` defaults to `false`; compile_error! blocks `external-gateway` feature |
| `kernel_invariant_gate` | ✅ PASS (test) | `kernel_invariant_after_bundle` proves rollback credits == locked amount (no inflation) |

## Core System Gates

| Gate | Status | Evidence |
|---|---|---|
| `chain_spec_valid` | ⚠️ UNK | Requires live devnet boot — not verified offline |
| `rpc_live` | ⚠️ UNK | Requires running node on configured endpoint |
| `supply_locked_invariant` | ⚠️ UNK | Requires on-chain query of supply ledger |
| `block_finality` | ⚠️ UNK | Requires ≥1 finalized block on live node |

---

## Feature Scope Gates (compile_error! enforced)

The following features are **BLOCKED** from mainnet-rc1 by compile-time guards:

| Gated Feature | Guard Location | Status |
|---|---|---|
| `external-gateway` | `pallets/x3-cross-vm-router/src/lib.rs` | ✅ BLOCKED |
| `parallel-executor` | `pallets/x3-cross-vm-router/src/lib.rs` | ✅ BLOCKED |
| `appzone-factory` | `pallets/x3-cross-vm-router/src/lib.rs` | ✅ BLOCKED |
| `pq-experimental` | `pallets/x3-cross-vm-router/src/lib.rs` | ✅ BLOCKED |
| `advanced-dex` | `pallets/x3-cross-vm-router/src/lib.rs` | ✅ BLOCKED |
| `ai-optimizer` | `pallets/x3-cross-vm-router/src/lib.rs` | ✅ BLOCKED |
| `gpu-acceleration` | `pallets/x3-cross-vm-router/src/lib.rs` | ✅ BLOCKED |

---

## E2E Test Evidence (in-process, no live node)

All tests in `tests/e2e/mainnet_rc1.rs` must pass before launch:

| Test | Scenario | Required Status |
|---|---|---|
| `internal_lock_settle` | Lock → Settle accounting | MUST PASS |
| `internal_lock_swap_settle` | Lock → Swap → Settle (AMM) | MUST PASS |
| `abort_refund` | Lock → Abort → rollback | MUST PASS |
| `slippage_refund` | Slippage guard → rollback | MUST PASS |
| `packet_replay_rejected` | Replay guard rejects duplicate | MUST PASS |
| `packet_timeout_refund` | Expired packet detected | MUST PASS |
| `packet_not_timed_out_before_deadline` | Live packet passes | MUST PASS |
| `duplicate_slot_rejected` | Planner rejects duplicate slots | MUST PASS |
| `burn_unlock_accounting` | Lock → Burn closes custody | MUST PASS |
| `kernel_invariant_after_bundle` | Rollback = locked (no inflation) | MUST PASS |
| `genesis_determinism` | `commit_packet` is deterministic | MUST PASS |
| `universal_contract_compiles` | UC → IXL bundle round-trip | MUST PASS |
| `lp_lock_prevents_early_unlock` | LP lock blocks early withdrawal | MUST PASS |
| `lp_lock_allows_unlock_after_expiry` | LP lock allows unlock at deadline | MUST PASS |
| `settle_request_rejects_inverted_bounds` | Settlement rejects bad bounds | MUST PASS |
| `settle_request_rejects_zero_amount` | Settlement rejects zero | MUST PASS |

Run all with: `cargo test -p e2e_tests --test mainnet_rc1`

---

## Launch Blockers

The following conditions MUST be met before mainnet launch:

1. **`external_bridges_disabled` gate must be PASS** — `ExternalBridgesEnabled` must be `false` in genesis
2. **All 16 E2E tests must PASS** — no skips, no `#[ignore]`
3. **`x3-readiness-report` offline mode must not claim `Ready`** — offline = `Unknown`, never `Pass`
4. **`mainnet-rc1` feature must be the only active compile target** — run `cargo check -p pallet-x3-cross-vm-router` with no extra features

---

## What Is Not In RC-1 (explicitly out of scope)

- External chain bridge (any IBC to chains outside X3 internal VMs)
- Parallel executor (speculative parallel bundle execution)
- AppZone factory (permissionless L2 zone deployment)
- PQ-experimental crypto (post-quantum signature schemes)
- Advanced DEX (order books, CLMMs, multi-hop routing)
- AI optimizer (ML-driven fee/route optimization)
- GPU acceleration (GPU-assisted ZK or consensus)

These remain in the repo but are compile-gated. Enabling them requires:
1. A governance vote on the live chain
2. An explicit feature flag set in the runtime
3. A new readiness report covering the additional scope

---

*This report is generated from codebase analysis. For a live report, run `cargo run -p x3-readiness-report -- --live --endpoint ws://127.0.0.1:9944`*
