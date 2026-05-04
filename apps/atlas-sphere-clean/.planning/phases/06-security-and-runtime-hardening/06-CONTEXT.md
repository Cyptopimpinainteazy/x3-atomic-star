---
phase: 06-security-and-runtime-hardening
milestone: v1.1
status: in-progress
created: 2026-03-20
---

# Phase 6: Security and Runtime Hardening — Context

## Objective
Remove known production-safety hazards across node, runtime, pallets, and RPC surfaces.

## Dependencies
- **Depends on:** Phase 5 (Dual-VM completion — COMPLETE 2026-03-20)
- **Enables:** Phase 8 (Testnet proving and go/no-go)

## Scope

### Task 06-01: Eliminate critical `unwrap()` / `expect()` / `panic!()` paths in production code
**Status:** ✅ **COMPLETE**

**Scan results:**
- `node/src/` + `crates/x3-rpc/src/`: zero production panics (grep returned empty)
- `crates/cross-vm-bridge/src/lib.rs`: tests start at line 1477; all 81 unwrap hits are `#[cfg(test)]` only
- `pallets/`: all unwrap/expect in `tests.rs` / `mock.rs` — none in production dispatch code
- `runtime/src/lib.rs:2066,2083`: in `#[test]` functions

**One real production panic found and fixed:**
- `runtime/src/fraud_proofs/startup_gate.rs:134` — `reference_commitment_1tx_nodeps()` called
  `.expect("reference vector must decode without error")` on a `Result<H256, WitnessError>`.
  **Fix:** Changed function to return `Result<H256, WitnessError>`, changed `required_vectors()`
  to return `Result<Vec<TestVector>, GateError>` and updated `run_startup_gate()` to propagate
  via `required_vectors()?`.
- **Completion date:** 2026-03-20

### Task 06-02: Harden RPC, rate limiting, and abuse controls
**Status:** ✅ **COMPLETE**

**Findings:**
- `crates/x3-rpc/src/wallet_dex_rpc.rs`: no input size limits on `display_message`, `signatures`
  vector, `approval_signature`, or `account` fields — all potential DoS vectors.
- `crates/x3-rpc/src/gas_estimation.rs`: no calldata size limit or batch size limit — O(n) loops
  on `tx.data` (intrinsic gas loop, opcode sim loop) exploitable via huge payloads.

**Fixes applied:**
- `wallet_dex_rpc.rs`: Added constants `MAX_DISPLAY_MESSAGE_LEN=256`, `MAX_SIGNATURES_COUNT=10`,
  `MAX_SIGNATURE_LEN=130`, `MAX_APPROVAL_SIGNATURE_LEN=130`, `MAX_ACCOUNT_LEN=256`.
  Validation guards added at the start of `execute_swap`, `request_hardware_signature`,
  `approve_transaction`, `get_balance`.
- `gas_estimation.rs`: Added constants `MAX_CALLDATA_LEN=128_000`, `MAX_BATCH_SIZE=50`,
  `MAX_ADDRESS_LEN=128`. Validation added to `estimate_gas`, `estimate_gas_many`, `call`.

**Note:** `crates/x3-rpc/` is standalone source (no Cargo.toml, not yet wired into workspace).
Rate limiting at the connection level is deferred to the node JSON-RPC server middleware layer.

- **Completion date:** 2026-03-20

### Task 06-03: Audit pallet permissions, events, and runtime safety invariants
**Status:** ✅ **COMPLETE**

**Audit scope and results:**

| Pallet | Dispatchers | Origin Pattern | Event Emission | Status |
|--------|-------------|----------------|----------------|--------|
| governance | 16 dispatchers | `T::FastTrackOrigin`, `T::CancelOrigin`, `T::RuntimeUpgradeOrigin`, `T::EmergencyOrigin`, `T::AIReviewOrigin`, `ensure_signed` | All state changes emit events | ✅ PASS |
| x3-verifier | 6 dispatchers | `ensure_signed` (user), `T::ExecutorRegistrar` (admin) | Events on register/submit/dispute/slash | ✅ PASS |
| agent-memory | 7 dispatchers | `ensure_signed` + permission checks | Events on all mutations | ✅ PASS |

**Key findings:**
- 186 `ensure_signed/root/none/origin` calls across all pallet lib.rs files
- All privileged operations (kill switch, fast track, config update, AI authorization) use
  configurable authority origins — not raw `ensure_root!`
- `slash_executor` is an internal helper (`impl Pallet<T>`, not `#[pallet::call]`); only called
  from the `dispute_receipt` dispute resolution path — not externally dispatchable
- No origin bypass or missing checks found

- **Completion date:** 2026-03-20

## Key Artifacts
- Fixed: `runtime/src/fraud_proofs/startup_gate.rs` — production panic removed
- Hardened: `crates/x3-rpc/src/wallet_dex_rpc.rs` — input size limits added
- Hardened: `crates/x3-rpc/src/gas_estimation.rs` — calldata/batch limits added
- Audit: 186 origin checks across pallets — all verified

## Success Criteria
- [x] No `unwrap()` / `expect()` in non-test, non-infallible production paths
- [x] RPC inputs validated with hard size limits
- [x] All pallet dispatchers have explicit origin checks
- [x] Events emitted after every state change
- [ ] Connection-level rate limiting (deferred to node infrastructure layer — Phase 8)

## Tech Stack
- Rust / Substrate FRAME (pallets)
- jsonrpc-core (RPC server)
- runtime/src/fraud_proofs/ (scheduler validation gate)
