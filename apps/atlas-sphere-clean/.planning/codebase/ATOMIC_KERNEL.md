# Deep Dive: Atomic Kernel (Cross‑VM Orchestration)

**Purpose:** Provide a single atomic transaction boundary across EVM and SVM execution. The X3 Kernel pallet (and its associated libraries) orchestrates fee reservation, account locking, execution ordering, verification, commit, and rollback.

---

## Key Concepts

### Comit (Atomic Bundle)
A **Comit** is the unit of atomic work:
- Contains EVM payload + SVM payload
- Has `origin` account, `nonce`, `fee`, `prepare_root`
- Must succeed (both VMs) or fail (rollback)

Core struct lives in:
- `pallets/x3-kernel/src/types.rs` (likely)

### Prepare Root
A cryptographic commitment to input parameters (accounts, payloads, gas) used to validate that the execution phase corresponds to the prepared request.

### Account Locking
Deterministic lock order (e.g., sorted hashes of account IDs) prevents deadlocks across concurrent Comits.

---

## Key Files

### Pallet (on-chain logic)
- `pallets/x3-kernel/src/lib.rs` — main pallet definition, dispatchables, storage items.
- `pallets/x3-kernel/src/adapters.rs` — defines VM adapter traits and implementations.
- `pallets/x3-kernel/src/lock.rs` — account locking logic.
- `pallets/x3-kernel/src/events.rs` (if exists) — `ComitSubmitted`, `ComitFinalized`, `ComitFailed`.
- `pallets/x3-kernel/src/tests.rs` — unit tests for kernel state transitions.

### Off-chain / RPC interaction
- `crates/x3-atomic-client/` — RPC client that submits Comits and tracks status.
- `node/src/rpc_atlas.rs` (or similar) — exposes `atlasKernel_submit_comit`.

### Supporting libraries
- `crates/cross-vm-bridge/` — shared primitives for cross-VM coordination.
- `crates/x3-vm/` — VM execution engine common utilities (likely includes `vm_nested_call_with_global_state`).

---

## Workflow (Expected)

1. **Submit COMIT** via RPC (e.g., `atlasKernel_submit_comit`).
2. **Prepare Phase**
   - Reserve fees (lock funds).
   - Lock affected accounts deterministically.
   - Store `PendingComits` record with `prepare_root`.
3. **Execute Phase**
   - Execute EVM adapter (Frontier) with `evm_payload`.
   - Execute SVM adapter (solana-rbpf) with `svm_payload`.
   - Collect receipts + state diffs.
4. **Verify Phase**
   - Confirm `prepare_root` matches expected commitment.
   - Ensure both receipts indicate success.
5. **Finalize Phase (Success)**
   - Apply state diffs to canonical storage.
   - Release locks.
   - Distribute fees.
6. **Rollback Phase (Failure)**
   - Revert state (Substrate handles via runtime rollback).
   - Release locks.
   - Refund fees.

---

## Key Risks & Failure Modes

- **Deadlocks** if locking order is not deterministic.
- **Fee griefing** if fees are not reserved before execution.
- **State divergence** if EVM and SVM state changes aren’t merged into canonical ledger.
- **Non‑deterministic execution** (GPU/parallel execution) can break finality.

---

## What to Inspect for a Deep Audit

- How `prepare_root` is computed and validated.
- Where pending commits are stored and how they're garbage-collected.
- How state diffs are captured (e.g., `StorageChanges` / `StateChange` objects).
- Cross‑VM execution ordering (EVM first vs SVM first) and whether it’s configurable.
- Failure reporting: does `ComitFailed` provide enough reason for debugging?

---

## Next Steps for Engineers

1. Run unit tests for `pallets/x3-kernel` and ensure they cover:
   - Successful commit.
   - Failure in EVM executes rollback.
   - Failure in SVM executes rollback.
   - Lock contention (two concurrent commits touching same accounts).
2. Add integration test that submits a real EVM contract + SVM program in one commit and verifies final state.
3. Confirm `prepare_root` is collision-resistant and includes:
   - account IDs
   - payload hashes
   - nonce + fee

---

## References
- `docs/ARCHITECTURE.md` (transaction flow, comit lifecycle)
- `X3_GAPS_REPORT.md` (atomicity gap list)
