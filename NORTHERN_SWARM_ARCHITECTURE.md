# Northern Swarm Architecture

> **Status:** runtime wiring exists · signed metadata-driven executor ↔ chain contract exists · on-chain M-of-N quorum + reward settlement tested · RC3–RC5 still required
>
> **Evidence (2026-09-27, local x3star1):** `pallets/northern-swarm` 7/7 tests,
> `crates/northern-swarm` 13/13 tests, `python3 scripts/mainnet/swarm_reactor_gate.py` PASS.
> Weights in `pallets/northern-swarm/src/weights.rs` are benchmark-CLI output
> (`Weight::from_parts(ref_time, proof_size)` with real proof sizes), not hand-written.
> This document describes the current tree; it is not a mainnet-ready claim.
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
| Chain watcher (polling) | `src/chain_watcher.rs` | ✅ reads the real `NorthernSwarm::Tasks` map |
| Result submitter | `src/result_submitter.rs` | ✅ metadata-encoded, signed, watched to inclusion |
| Compute backends | `src/backend.rs` | ⚠️ `ComputeBackend` + `CpuBackend` + `GpuBackend`; no `AutoBackend` selection yet |

**Current executor contract (what is real, and what is still weak):**
- The watcher derives the `NorthernSwarm::Tasks` storage prefix from the canonical Twox128 pallet/item
  names, reads it with `state_getKeys`, and decodes values as the pallet's own
  `TaskRecord`/`TaskKind`/`TaskStatus` through a compile-time dependency on the pallet. A hard-coded
  `PendingTasks` key and a fabricated full storage key are both gone. *Weak point:* the pallet/item
  names are still string literals here, not read out of runtime metadata.
- IPFS (`ipfs://`) and inline `hex:` payload fetching exist.
- The submitter builds Subxt `DynamicPayload`s (`tx("NorthernSwarm", "claim_task", …)`) and signs them
  with the executor's sr25519 key via `sign_and_submit_then_watch_default`, so the extrinsic is signed
  by the registered account and inclusion/finality is awaited. *Weak point:* there is no live-node
  integration test — these guarantees are proven at the encoder/signer/decoder boundary only.
- `TaskPayload` preserves `TaskKind`, so `AiInference` can be routed rather than silently hashed.
- `GpuBackend` executes through `x3_accel::sha256_with_parity`, which recomputes on CPU and refuses a
  divergent accelerator result. It is fail-closed (returns an error) rather than silently falling back;
  the auto-selector/quarantine policy is still missing.

---

### RC1.5 — Live chain watcher (PARTIAL)

**Goal:** connect the executor to a running node with metadata-compatible reads and signed submissions.

- [x] Read the real `Tasks` storage (Twox128-derived prefix, pallet types decoded).
- [ ] Derive the pallet/item names from runtime metadata instead of the literals `"NorthernSwarm"`/`"Tasks"`.
- [x] Implement IPFS payload fetch (`ipfs://` URI scheme).
- [x] Sign `claim_task` / `submit_result` with the configured executor key.
- [x] Remove hard-coded pallet/call indices; call encoding is metadata-driven through Subxt.
- [ ] Prove all of the above against a running node (no live-node test exists yet).

---

### RC2 — On-chain registry pallet (`pallets/northern-swarm`) ✅ (scaffold + quorum + settled rewards)

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

**RC2 status:**
- [x] Wire pallet into `runtime/src/lib.rs` — `Config` impl + `construct_runtime!`.
- [x] Add `pallet-northern-swarm` to `runtime/Cargo.toml` and std feature propagation.
- [x] Benchmark all 8 extrinsics and replace placeholder `Weight::from_parts` constants with measured
      ones (`BENCHMARK_STEPS=50 BENCHMARK_REPEAT=20 bash scripts/run-frame-benchmarks.sh run pallet-northern-swarm`).
- [x] Add mock runtime and unit tests (`pallets/northern-swarm/src/tests.rs`, 7 tests).
- [x] Replace single-result auto-finalization with M-of-N quorum verification (`finalise_with_quorum`,
      `QuorumThreshold`, `TaskStatus::Disputed`).
- [x] Settle reserved task rewards to accepted executor(s) and prove balance invariants
      (`task_reward_moves_reserved_balance_to_winner`, `task_reward_preserves_total_issuance`).
- [ ] Give a `Disputed` task a resolution/refund path — today the submitter's bond stays reserved
      and only root-level `slash_executor` can touch an executor's stake.
- [ ] Prove the adversarial matrix (duplicate claim, claim after finalisation, non-claimant result,
      suspended executor, exact-balance boundary, heartbeat expiry) — in flight.
- [ ] Bind result acceptance to something stronger than hash equality (re-execution, fraud proof, or
      verifiable computation). An M-of-N set that agrees on a wrong hash is currently paid.

---

### RC3 — Quorum verification (PARTIAL: on-chain quorum exists, verification does not)

**Goal:** require M-of-N executors to independently commit identical result hashes
before finalising a task and releasing the reward.

- [x] Quorum is evaluated inline when a result is committed, not in `on_finalize`; a single committed
      result no longer finalises a task.
- [x] Winning executors receive an equal share of the reserved task reward; the remainder is unreserved
      back to the submitter.
- [x] A full set of non-matching commits moves the task to `TaskStatus::Disputed` and pays nobody.
- [ ] Non-matching executors are **not** auto-slashed. `SlashReason::QuorumMismatch` exists as a
      variant, but no code path constructs it: a non-matching commit set disputes the task, pays
      nobody and leaves every stake untouched. Slashing is a manual root-only dispatchable.
- [ ] `MaxClaimedTasksPerExecutor` is enforced at claim time but has no test exercising the limit.
- [ ] Agreement is over a self-reported hash. Nothing re-executes the work.

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
