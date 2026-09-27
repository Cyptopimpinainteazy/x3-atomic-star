# Northern Swarm Architecture

> **Status:** runtime wiring exists · executor/pallet contract is incomplete · mainnet hard gate intentionally RED · RC3–RC5 still required
>
> `pallets/swarm` is **legacy-only reference** — do not add new production dependencies to it.

---

## Overview

Northern Swarm is a three-layer system for verified off-chain compute on X3:

```
┌─────────────────────────────────────────────────────────┐
│  X3 Runtime (on-chain)                                  │
│  └── pallets/northern-swarm  (RC2 registry pallet)      │
│       stake / tasks / result commits / slash-reward     │
├─────────────────────────────────────────────────────────┤
│  Off-chain executor network                             │
│  └── crates/northern-swarm  (RC1 executor binary)       │
│       chain watcher → task fetch → execute → submit     │
├─────────────────────────────────────────────────────────┤
│  Payload layer                                          │
│  └── IPFS / inline hex URIs                             │
│       task body, params, model weights                  │
└─────────────────────────────────────────────────────────┘
```

---

## RC Roadmap

### RC1 — Off-chain executor skeleton (`crates/northern-swarm`) ✅

**Goal:** prove the off-chain execution loop end-to-end on a local testnet.

| Component | File | Status |
|-----------|------|--------|
| Binary entry point | `src/main.rs` | ✅ done |
| Crate root / re-exports | `src/lib.rs` | ✅ done |
| Canonical types | `src/types.rs` | ✅ done |
| Deterministic executor | `src/executor.rs` | ✅ done |
| Chain watcher (polling) | `src/chain_watcher.rs` | ✅ done (poll stub) |
| Result submitter | `src/result_submitter.rs` | ✅ done (local proof store) |

**Current executor limitations (release-blocking):**
- The watcher now calls JSON-RPC, but it targets a hard-coded `PendingTasks` storage key while the pallet exposes `Tasks`; this is not a valid pallet/executor contract.
- IPFS and inline `hex:` payload fetching exist.
- The submitter calls `author_submitExtrinsic`, but it constructs an unsigned call with hard-coded pallet index `82`; `submit_result` requires a signed executor origin.
- `TaskPayload` currently drops `TaskKind`, so `AiInference` cannot be routed to a GPU/NPU backend.
- No typed `ComputeBackend` abstraction, GPU backend, or explicit CPU fallback is present.

---

### RC1.5 — Live chain watcher (PARTIAL / NOT RELEASEABLE)

**Goal:** connect the executor to a running node with metadata-compatible reads and signed submissions.

- [ ] Subscribe to `NorthernSwarm::TaskSubmitted` events or read the real `Tasks` storage through runtime metadata.
- [x] Implement IPFS payload fetch (`ipfs://` URI scheme).
- [ ] Sign `NorthernSwarm::submit_result` with the registered executor key.
- [ ] Remove hard-coded pallet/call indices; derive call encoding from runtime metadata.

---

### RC2 — On-chain registry pallet (`pallets/northern-swarm`) ✅ (scaffold)

**Goal:** minimal FRAME pallet to anchor executor stake and result hashes.

| Storage | Key → Value |
|---------|-------------|
| `Executors` | `AccountId → ExecutorRecord` |
| `Tasks` | `Hash → TaskRecord` |
| `ResultCommits` | `(Hash, AccountId) → Hash` |
| `ClaimedTaskCount` | `AccountId → u32` |

| Extrinsic | Index | Who |
|-----------|-------|-----|
| `register_executor` | 0 | Any |
| `deregister_executor` | 1 | Self |
| `release_stake` | 2 | Self (after cooldown) |
| `submit_heartbeat` | 3 | Executor |
| `submit_task` | 4 | Any |
| `claim_task` | 5 | Executor |
| `submit_result` | 6 | Executor |
| `slash_executor` | 7 | Root |

**RC2 TODO before mainnet:**
- [x] Wire pallet into `runtime/src/lib.rs` — `Config` impl + `construct_runtime!`.
- [x] Add `pallet-northern-swarm` to `runtime/Cargo.toml` and std feature propagation.
- [ ] Benchmark all extrinsics and replace placeholder `Weight::from_parts` constants.
- [ ] Add mock runtime and unit tests (`pallets/northern-swarm/src/tests.rs`).
- [ ] Replace single-result auto-finalization with M-of-N quorum verification.
- [ ] Settle reserved task rewards to accepted executor(s) and prove balance invariants.

---

### RC3 — Quorum verification (planned)

**Goal:** require M-of-N executors to independently commit identical result hashes
before finalising a task and releasing the reward.

- `on_finalize` hook scans `ResultCommits` and checks for quorum threshold.
- Non-matching executors are auto-slashed (`SlashReason::QuorumMismatch`).
- Winning executors receive reward split from the task bond.
- `MaxClaimedTasksPerExecutor` becomes meaningful for task assignment fairness.

---

### RC4 — X3 Lang job compiler (planned)

**Goal:** compile X3 Lang job definitions into `TaskPayload` bytecode executed
by `TaskExecutor::run_deterministic()`.

- Replace the RC1 stub `run_deterministic()` with a WASM-sandboxed evaluator.
- `TaskKind::X3LangAgent` dispatches to the X3 interpreter.
- Proof generation includes WASM execution trace hash.

---

### RC5 — Full Northern Swarm launch (planned)

- GPU-accelerated `TaskKind::AiInference` dispatch.
- Integration with `gpu-swarm/` scheduling layer.
- Validator set key registration (executor ECDSA key on-chain).
- Dispute resolution protocol (RC3 jury vote removed — replaced by ZK proof).

---

---

## Mainnet release hard gate

`python3 scripts/mainnet/swarm_reactor_gate.py` is called by
`scripts/mainnet_release_gate.py`. It fails closed until RC1.5/RC2 correctness,
RC3 quorum verification, and the first GPU/CPU backend contract are implemented.

The gate deliberately separates **consensus correctness** from accelerator
performance: CPU-verifiable deterministic execution remains the reference path;
GPU/NPU/FPGA acceleration may improve throughput but may not change accepted
results.


## Legacy reference

`pallets/swarm/src/lib.rs` is preserved **read-only** for reference.
It was deprecated in `//! **DEPRECATED**` at the top of that file.

Do not add new runtime imports, storage migrations, or extrinsics to `pallet-swarm`.
The Northern Swarm pallet is its sole replacement.
