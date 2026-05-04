# X3 Truth Pass (2026-04-15)

Goal: reconcile `X3_COMPLETION.md`, `X3_GAPS_REPORT.md`, and the actual repo state before sprinting.

## What I verified

**Repo gates**
- `cargo check --workspace`: **PASS** (warnings only)
- `cargo fmt --all -- --check`: **PASS**

**Key code paths exist (non-empty)**
- `node/src/rpc.rs`
- `node/src/rpc_frontier.rs`
- `node/src/service.rs`
- `node/src/chain_spec.rs`
- `crates/cross-vm-bridge/src/lib.rs`
- `crates/evm-integration/src/lib.rs`
- `crates/svm-integration/src/lib.rs`

**Testnet artifacts currently present**
- `testnet/genesis.json`: **placeholder** (`"code": "0x..."`, fake authority keys)
- `testnet/docker-compose.yml`: **not deployable as-is** (expects `/health` endpoint, `VALIDATOR_1_PEER_ID`, and a real genesis/runtime code blob)

## High-signal mismatches vs docs

- `X3_COMPLETION.md` claims “AUTHORITATIVE SOURCE OF TRUTH” but contains many items that are not currently tied to hard proof (commands + logs + tests). Treat it as **a checklist**, not proof.
- `X3_GAPS_REPORT.md` flags RPC WebSocket as missing, but the node *does* wire an RPC builder via `sc_service::spawn_tasks` (`node/src/service.rs`). Actual RPC usability still needs live verification (Polkadot.js, scripted smoke).

## Truth table (blockers + next actions)

Legend for **current state**: `missing` | `partial` | `implemented but unverified` | `done`

| blocker | owner | current state | proof | next action |
|---|---|---:|---|---|
| Workspace builds from clean checkout | Security/Quality | done | `cargo check --workspace` (PASS) | Add `scripts/health-check.sh` + CI gate once smoke is real |
| Formatting gate | Security/Quality | done | `cargo fmt --all -- --check` (PASS) | Keep rustfmt in CI gate |
| WS + HTTP RPC works end-to-end | Core Node | implemented but unverified | RPC module wired in `node/src/service.rs` via `spawn_tasks` + `node/src/rpc.rs` | Run 1 node and verify: Polkadot.js WS connect + scripted RPC smoke |
| Health endpoint for deploy checks | Core Node | missing | No `health` method in `node/src/rpc.rs`; compose expects `http://.../health` | Add `system_health`-style RPC method or a simple HTTP endpoint and update compose |
| Frontier/EVM JSON-RPC surface | Core Node | partial | `node/src/rpc_frontier.rs` exposes a small subset incl. `eth_sendRawTransaction` | Decide required MVP surface (min: sendRawTx, getBalance, call, estimateGas) + add integration test |
| SVM JSON-RPC surface | Core Node | partial | `node/src/rpc_frontier.rs` only has `svm_getBalance` + `svm_isProgram` | Define MVP RPC surface for SVM execution / deploy and implement |
| Canonical “ledger” sync for EVM | Dual-VM | implemented but unverified | Runtime-backed RPC calls exist; ledger semantics not proven | Define canonical-ledger state target + add integration test proving persistence |
| Canonical “ledger” sync for SVM | Dual-VM | implemented but unverified | Runtime-backed RPC calls exist; ledger semantics not proven | Same as above; add SVM execution → state proof |
| Cross-VM happy-path transfer (atomic) | Dual-VM | implemented but unverified | `crates/cross-vm-bridge` implements 2PC; runtime wiring not proven | Add runtime dispatcher wiring + `tests/integration/cross_vm_test.rs` happy path |
| 3-validator chain spec + genesis artifacts | Consensus/Testnet | partial | `node/src/chain_spec.rs` has `testnet` entry; `testnet/genesis.json` is placeholder | Produce real `testnet/chain-spec.json` from `build-spec --raw` and document key provisioning |
| 3-node sustained consensus + restart recovery | Consensus/Testnet | implemented but unverified | Node has Aura/GRANDPA wiring in `node/src/service.rs` | Create 3-node run script + integration test for block production/finality/restart |
| Deployable local testnet stack | Ops | partial | `testnet/docker-compose.yml` references missing `/health`, peer id env var, placeholder genesis | Replace with minimal compose that uses generated chain spec + explicit bootnode peer id discovery |
| Metrics live (Prometheus) | Ops | implemented but unverified | Node exposes Prometheus flags in compose; not validated live | Add `prometheus.yml` targets + smoke query in `scripts/health-check.sh` |
| E2E smoke suite passes | Ops | implemented but unverified | `tests/e2e` exists; not executed in this pass | Run focused smoke suite against a 1-node dev chain, then extend to 3-node |

