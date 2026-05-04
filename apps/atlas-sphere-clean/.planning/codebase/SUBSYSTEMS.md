# X3 Chain Subsystem Deep Dive

**Date:** 2026-03-15
**Scope:** Key internal subsystems (atomic kernel, RPC layer, validator stack)

---

## 1. Atomic Kernel (Cross‑VM Orchestrator)

### Location
- `pallets/x3-kernel/` — core orchestration pallet
- `crates/cross-vm-bridge/` — cross‑VM coordination libraries
- `crates/x3-atomic-client/` — off‑chain RPC client + bundle submission

### Responsibility
The Kernel is the **single source of truth** for cross‑VM atomicity. It:
- Accepts `Comit` bundles (EVM + SVM payloads)
- Reserves fees, locks accounts, and records pending operations
- Executes VMs in a controlled order (EVM then SVM)
- Verifies execution receipts and prepares commit roots
- Finalizes state on success or rolls back on failure

### Key Data Structures
- `Comit<AccountId, Balance>` — transaction bundle (EVM ↔ SVM payloads)
- `PendingComits` storage map — in‑flight atomic operations
- `prepare_root` — deterministic commitment to the prepare phase

### Execution Phases
1. **Prepare**: Reserve fees, lock accounts in deterministic order
2. **Execute**: Run EVM then SVM, collect receipts + diffs
3. **Verify**: Ensure `prepare_root` matches actual execution inputs
4. **Finalize**: Commit state diffs, release locks, distribute fees
5. **Rollback**: Revert state, unlock accounts, refund fees

### Core Files
- `pallets/x3-kernel/src/lib.rs` — pallet definition + dispatchable functions
- `pallets/x3-kernel/src/adapters.rs` — VM adapter trait + implementations
- `pallets/x3-kernel/src/lock.rs` — deterministic lock ordering
- `crates/cross-vm-bridge/src/lib.rs` — cross‑VM coordination primitives

### Known Risks / Open Questions
- Lock ordering must be deterministic to avoid deadlocks (account hash ordering is used)
- Fee reservation must be atomic and refunded correctly on rollback
- Cross‑VM state sync requires canonical ledger mapping; current gaps exist (see `X3_GAPS_REPORT.md`)

---

## 2. RPC Layer (eth_*, svm_*, atlasKernel_*)

### Location
- `node/src/rpc.rs` — main RPC wiring
- `node/src/rpc_frontier.rs` — Frontier (Ethereum) RPC handlers
- `node/src/rpc_svm.rs` (if present) / `node/src/rpc_svm_*` — SVM handlers
- `node/src/rpc_atlas.rs` (or similar) — Kernel RPC & governance APIs

### Responsibility
Expose a unified JSON‑RPC surface for:
- Standard Ethereum RPC (`eth_*, net_*, web3_*`)
- SVM-specific endpoints (`svm_sendTransaction`, `svm_simulate`, etc.)
- Kernel / atlas APIs (`atlasKernel_getCanonicalBalance`, `atlasKernel_getNonce`, etc.)

### Deployment Patterns
- `run-dev-node.sh` and `run-production-node.sh` configure RPC binding and enable/disable unsafe methods
- `docker-compose*.yml` setup includes proper RPC exposure and (optionally) HTTP Basic auth for secure deployments

### Key Concerns / Gaps
- WebSocket support appears incomplete (gap in `X3_GAPS_REPORT.md`) and must be implemented for many dApps
- Rate limiting + DDoS protection missing; should be integrated into `jsonrpsee` layers
- RPC method exposure must be locked down for production (avoid `--rpc-methods Unsafe`)

---

## 3. Validator Stack (Consensus + Scaling)

### Location
- `node/` — Substrate node binary (networking, consensus, executor)
- `runtime/` — Runtime definitions (pallet composition, weights)
- `crates/flash-finality/` — Flash finality consensus component
- `crates/poh-generator/` — Proof-of-history generator (related to GPU validator)
- `crates/x3-gpu-validator-swarm/` — GPU validator orchestration

### Responsibility
The validator stack provides:
- Block production (Aura / custom proposer) + finality (Grandpa / Flash Finality)
- Transaction execution (runtime + VM layers)
- Health/metrics reporting (via Prometheus / Grafana stack)
- Validator-specific features (GPU acceleration, parallel proposer)

### Known Gaps
- GPU validator and parallel proposer are still work-in-progress; full deterministic replay needs validation
- Consensus layering (Flash Finality + Grandpa) may be incompatible; requires clear failover strategy
- Multi-node testnet coverage is limited (most testing is single-node / local)

---

## 4. Where to Dive Deeper

If you want to explore specific subsystems further, start with these entry points:
- **Atomic Kernel:** `pallets/x3-kernel/src/lib.rs` + `crates/cross-vm-bridge/src/lib.rs`
- **RPC:** `node/src/rpc.rs` + `node/src/rpc_frontier.rs`
- **Validator / Consensus:** `node/src/service.rs` + `runtime/src/lib.rs` + `crates/flash-finality`

---

## Notes
This document is intentionally concise; it is meant as an entrypoint for engineers who need to traverse the main subsystems quickly. For full architecture and roadmap context, see `.planning/codebase/ARCHITECTURE.md` and `.planning/research/ARCHITECTURE.md`.
