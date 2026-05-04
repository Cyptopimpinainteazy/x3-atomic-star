# Architecture Patterns

**Domain:** Modular multi-VM blockchain runtime
**Researched:** 2026-03-15

## Recommended Architecture

X3 Chain is built as a Substrate-based L1 blockchain that co-locates two VM families in a single runtime:
- **EVM** (via Frontier / pallet-evm)
- **SVM** (via solana-rbpf BPF executor)

The core coordination is performed by the **X3 Kernel** (a runtime pallet) which provides:
- Atomic cross-VM transactions (`Comit` bundles)
- Deterministic account locking (prevents deadlocks)
- Unified fee accounting (single token, fee reservation)
- Canonical ledger state (single source of truth)

### Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|---------------|------------------|
| **Node (binary)** | P2P networking, consensus (Aura+Grandpa), RPC server, runtime instantiation | Runtime, RPC clients, peer nodes |
| **Runtime (`runtime/`)** | On-chain state transition logic; compiles to WASM | Pallets, runtime APIs, off-chain worker hooks |
| **X3 Kernel pallet** | Orchestration, comits, locking, fee reservation | EVM/SVM adapters, staking, treasury, governance |
| **EVM Adapter (crates/evm-integration)** | Bridges Frontier EVM into kernel; translates gas/weight | X3 Kernel, Frontier pallet, state trie |
| **SVM Adapter (crates/svm-integration)** | Executes BPF programs deterministically | X3 Kernel, solana-rbpf, state trie |
| **VM Router** | Classifies and routes inbound transactions | RPC layer, adapters |
| **Unified Account Layer** | Maps EVM H160 + SVM pubkey to Substrate AccountId | Kernel, adapters, fee module |
| **RPC Layer** | Exposes eth_*, svm_*, atlasKernel_* APIs | Node, clients, SDKs |

## Data Flow

### 1) Single-VM Transaction
1. Client submits `TxEnvelope` via JSON-RPC.
2. Node RPC layer classifies (EVM/SVM/Native) using `VmRouter`.
3. Runtime dispatches to correct adapter.
4. Adapter executes VM (EVM or SVM) and produces receipt + state diff.
5. Unified fee module charges gas/compute units, updates balances.
6. Substrate commits state to trie.

### 2) Cross-VM Atomic Transaction (Comit)
1. Client submits `Comit` bundle to `atlasKernel_submit_comit`.
2. **Prepare Phase**
   - Reserve fees from origin account.
   - Acquire deterministic locks on involved accounts.
   - Record pending comit with `prepare_root`.
3. **Execute Phase**
   - Execute EVM payload (Frontier).
   - Execute SVM payload (solana-rbpf).
   - Collect receipts and state deltas.
4. **Verify Phase**
   - Ensure `prepare_root` matches commitments.
   - Ensure both receipts signal success.
5. **Finalize** (on success)
   - Commit state diffs to canonical ledger.
   - Release locks; distribute fees.
   - Emit `ComitFinalized` event.
6. **Rollback** (on failure)
   - Revert to pre-prepare state (Substrate rollback).
   - Release locks; refund fees.
   - Emit `ComitFailed` event.

## Patterns to Follow

### Pattern: “Checklist as Code”
**What:** Treat completion checklist as an executable invariant. CI fails if any item is unchecked.
**When:** Always; ensures work cannot be merged if deemed incomplete.
**Example:** `scripts/x3_audit.sh` scans `X3_COMPLETION.md` for `⬜` markers.

### Pattern: Deterministic Lock Ordering
**What:** Acquire account locks in a canonical order (e.g., lexicographic AccountId) to prevent deadlocks.
**When:** In any multi-account atomic operation (Comits, cross-vm operations).
**Example:** Kernel computes lock order using hashed account IDs.

### Pattern: “No unwrap/expect in production”
**What:** Enforce `Result` propagation; disallow `unwrap/expect` in runtime/node code.
**When:** Always; enforced by `scripts/x3_audit.sh` scan.

## Anti-Patterns to Avoid

### Anti-Pattern: VM Execution Side-Effects Outside Kernel
**What:** Adapters mutate global state outside of kernel-managed context.
**Why bad:** Breaks atomicity and invalidates rollback guarantees.
**Instead:** Ensure all state diffs are collected and applied by kernel commit logic.

### Anti-Pattern: Multiple Consensus/State Roots
**What:** Maintaining separate ledgers for EVM and SVM.
**Why bad:** Complicates finality, increases attack surface, and breaks atomicity.
**Instead:** Use a single canonical ledger (Substrate trie) and map all VM state into it.

## Scalability Considerations

| Concern | At 100 nodes | At 1k nodes | At 10k+ nodes |
|---------|-------------|------------|---------------|
| Consensus throughput | Substrate Aura/Grandpa handles modest load | Tune block time and weights | Consider migrating to higher-throughput consensus (e.g., BABE + GRPA) |
| RPC load | Standard `jsonrpsee` scaling | Add caching, rate limits, sharding | Deploy RPC farms + load balancers |
| Cross-VM atomicity | Single-threaded per block currently | Introduce parallel proposer (in progress) | Partition state / sharding (future)
| Validator resource use | CPU bound for VM execution | Offload to GPU validator (in progress) | Ensure deterministic GPU execution and replay

## Sources
- `docs/ARCHITECTURE.md`
- `docs/X3_SYSTEMS.md`
- `docs/X3_INDEX.md`
- `X3_GAPS_REPORT.md`
