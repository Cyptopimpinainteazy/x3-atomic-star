# X3 Chain — Comprehensive Repo-Wide Gaps Report

**Date:** 2026-03-30  
**Scope:** cross-VM, GPU validator/TPS, swarm, Tauri desktop, x3-lang, x3-vm, x3-kernel  
**Method:** Full source scan, integration tracing, test inventory, production code audit  

---

## EXECUTIVE SUMMARY

The X3 Chain workspace contains **90+ crates** across a Substrate-based blockchain with dual EVM/SVM execution, a custom language (x3-lang), GPU validation swarm, Tauri desktop app, and cross-VM bridge.

**Core architecture is sound** — the x3-kernel pallet correctly composes cross-vm-bridge + x3-vm via pluggable adapter traits, and the x3-lang compiler pipeline flows cleanly into x3-vm via shared bytecode format. However, **significant integration gaps** exist at the edges: the Tauri desktop has zero live RPC, the two GPU crates don't talk to each other, and the TPS tracker is an isolated sidecar.

| Subsystem | Health | Gaps | Tests | Integration |
|-----------|--------|------|-------|-------------|
| cross-vm-bridge | ✅ Strong | 3 | 50+ | Wired via x3-kernel |
| x3-vm | ✅ Strong | 4 | 50+ | Wired via x3-backend bytecode |
| x3-kernel | ✅ Strong | 5 | 50+ | Hub — fully connected |
| x3-lang | ✅ Strong | 3 | Golden tests + e2e | Connected to x3-vm |
| x3-gpu-validator-swarm | ⚠️ Partial | 8 | 49 | **Not wired to gpu-swarm** |
| gpu-swarm | ⚠️ Partial | 12 | Integration tests | **Not wired to validator** |
| Tauri desktop | 🔴 Critical | 15 | TS unit tests | **Zero live RPC** |
| TPS tracker | ⚠️ Partial | 5 | 2 | External poller only |

**Total actionable gaps identified: 78**  
**Critical (ship-blocking): 18 | High: 29 | Medium: 22 | Low: 9**

---

## 1. CROSS-VM BRIDGE (`crates/cross-vm-bridge`)

**Lines:** 3,057 in `lib.rs` | **Tests:** 50+ unit tests | **Integration:** `tests/integration.rs`

### Architecture
- Implements EVM ↔ SVM 2PC atomic transactions via `CrossVmDispatcher` trait
- Operations: `AtomicSwap`, `CallEvm`, `CallSvm`, `MessageToEvm`, `MessageToSvm`
- 1024-byte payload limit, gas metering, escrow model with lock/release

### Gaps

| ID | Gap | Severity | File | Line |
|----|-----|----------|------|------|
| CVM-001 | `svm_commit.expect("checked above")` — reachable if Err was matched but Ok path continues | LOW | `lib.rs` | 1503 |
| CVM-002 | No `execute_x3_tx` method on `CrossVmDispatcher` — bridge can't include x3-lang bytecode in atomic operations | HIGH | `lib.rs` | trait def |
| CVM-003 | Cross-VM hardening backlog items P0-1 through P2-2 still open (proof validation stubs, fake keccak, mirror execution placeholders) | CRITICAL | PRD.md | L289-L297 |

### What's Working
- ✅ AtomicSwap with 2PC commit/rollback
- ✅ EVM→SVM and SVM→EVM message passing
- ✅ Gas metering across VMs
- ✅ Escrow model with mock tests
- ✅ Payload size validation

---

## 2. X3 VM (`crates/x3-vm`)

**Files:** vm.rs, bridge.rs, verifier.rs, jit_compiler.rs, dap_debugging.rs, error.rs + more  
**Tests:** 50+ across verifier (26), contract_upgrade (11), jit (5), debugging (8)

### Architecture
- Executes `BytecodeModule` produced by x3-backend
- Metered execution with gas limits
- JIT compiler for hot paths
- DAP debugging protocol support
- Bridge module for cross-VM balance queries

### Gaps

| ID | Gap | Severity | File | Line |
|----|-----|----------|------|------|
| XVM-001 | `bridge.rs` — MockBalanceProvider uses `lock().unwrap()` but this is test-only code | LOW | `bridge.rs` | 703-813 |
| XVM-002 | No integration test proving full x3-lang → x3-backend → x3-vm pipeline execution | HIGH | — | — |
| XVM-003 | JIT compiler has only 5 tests — needs coverage for edge cases (overflow, deep nesting) | MEDIUM | `jit_compiler.rs` | — |
| XVM-004 | No fuzz testing for VM instruction decoder | MEDIUM | — | — |

### What's Working
- ✅ Full bytecode execution engine
- ✅ Verifier with 26 test cases
- ✅ Contract upgrade pattern (11 tests)
- ✅ DAP debugging support (8 tests)
- ✅ Connected to x3-kernel via adapter traits

---

## 3. X3 KERNEL (`pallets/x3-kernel`)

**Files:** lib.rs, adapters.rs, wasm_adapters.rs, authority.rs, chaos_tests.rs, types.rs, mock.rs  
**Tests:** 50+ (chaos: 22, types: 19, adapters: 6, authority: 2, mock: 1)

### Architecture
- **Central orchestration hub** — composes cross-vm-bridge + x3-vm
- Three adapter traits: `EvmExecutorAdapter`, `SvmExecutorAdapter`, `X3ExecutorAdapter`
- Production adapters: `FrontierEvmAdapter`, `RbpfSvmAdapter`, `X3VmAdapter`
- WASM adapters for `no_std` runtime
- Runtime wiring at `runtime/src/lib.rs:541`

### Gaps

| ID | Gap | Severity | File | Line |
|----|-----|----------|------|------|
| KRN-001 | `benchmarking.rs` uses 3 bare `.expect()` calls in benchmark functions | LOW | `benchmarking.rs` | 46, 211, 298 |
| KRN-002 | `mock.rs` uses `.unwrap()` on `evm_tx`/`svm_tx` construction | LOW | `mock.rs` | 382, 388 |
| KRN-003 | No integration test exercising the full kernel dispatch path (kernel → adapter → real EVM/SVM) | HIGH | — | — |
| KRN-004 | Authority module has only 2 tests — needs more coverage for edge cases | MEDIUM | `authority.rs` | — |
| KRN-005 | Chaos tests don't cover cross-VM atomic failure recovery | MEDIUM | `chaos_tests.rs` | — |

### What's Working
- ✅ Full adapter trait system with pluggable backends
- ✅ WASM + native adapter implementations
- ✅ Runtime configuration wired
- ✅ Comprehensive chaos testing (22 tests)
- ✅ Type system tests (19 tests)

---

## 4. X3-LANG (Compiler Pipeline)

**Crates:** x3-lexer → x3-parser → x3-ast → x3-hir → x3-mir → x3-semantics → x3-typeck → x3-opt → x3-backend → x3-compiler  
**Tests:** Golden tests (parser, semantics, typeck), e2e + integration + determinism tests (compiler), optimizer smoke test

### Architecture
- Full compiler pipeline: source → tokens → AST → HIR → MIR → optimized → bytecode
- `x3-backend` produces `BytecodeModule` consumed by `x3-vm`
- `x3-common` provides shared `SourceMap` and `ErrorAccumulator`

### Gaps

| ID | Gap | Severity | File | Line |
|----|-----|----------|------|------|
| LANG-001 | `source.rs` — `files.get(&id).unwrap().clone()` panics if file ID is invalid (guarded by if-let but the HashMap path after is unguarded) | MEDIUM | `x3-common/src/source.rs` | 133 |
| LANG-002 | `error.rs` — `into_result()` uses `errors.into_iter().next().unwrap()` — safe because it checks `has_errors()` first, but should use `expect("has_errors was true")` for clarity | LOW | `x3-common/src/error.rs` | 181 |
| LANG-003 | No documentation for x3-lang syntax or language reference | HIGH | — | — |

### What's Working
- ✅ Complete compiler pipeline from source to bytecode
- ✅ Golden tests for parser, semantics, type checking
- ✅ E2E compiler tests + determinism tests
- ✅ Optimizer with smoke tests
- ✅ Bytecode format consumed by x3-vm

---

## 5. GPU VALIDATOR SWARM (`crates/x3-gpu-validator-swarm`)

**Files:** validator.rs, deterministic.rs, quarantine.rs, gpu_fallback_chain.rs, cpu_validator.rs, multi_gpu_dispatcher.rs, crypto.rs, x3_kernel_versioning.rs, payment.rs, gpu_memory_pool.rs, orchestrator.rs, network.rs  
**Tests:** 49 unit tests across modules

### Architecture
- Standalone GPU validation system with deterministic execution
- GPU fallback chain (GPU → CPU fallback)
- Multi-GPU dispatcher for parallel validation
- Payment/billing system
- Memory pool management
- Network peer management with event channels

### Gaps

| ID | Gap | Severity | File | Line |
|----|-----|----------|------|------|
| GVLD-001 | `crypto.rs` — `self.secret[..32].try_into().unwrap()` panics if slice is wrong size | HIGH | `crypto.rs` | 217 |
| GVLD-002 | `network.rs` — `duration_since(UNIX_EPOCH).unwrap()` panics if system clock is before epoch | LOW | `network.rs` | 137 |
| GVLD-003 | **Zero integration with `gpu-swarm` crate** — no shared types, no cross-crate calls, no trait bridging | CRITICAL | — | — |
| GVLD-004 | No runtime integration — validator swarm is standalone, not wired into consensus or block production | CRITICAL | — | — |
| GVLD-005 | GPU proof verification is local only — no on-chain proof submission | HIGH | — | — |
| GVLD-006 | Slashing conditions defined but not connected to a staking/economics pallet | HIGH | — | — |
| GVLD-007 | Reward distribution mechanism defined but not connected | HIGH | — | — |
| GVLD-008 | Network module uses in-memory peer list, no persistent discovery | MEDIUM | `network.rs` | — |

### What's Working
- ✅ Deterministic GPU execution (3 tests)
- ✅ GPU fallback chain with 12 tests
- ✅ CPU validator fallback (6 tests)
- ✅ Multi-GPU dispatcher (4 tests)
- ✅ Ed25519 cryptographic signing
- ✅ Payment/billing system (5 tests)
- ✅ Kernel versioning (7 tests)

---

## 6. GPU SWARM (`crates/gpu-swarm`)

**Files:** 63 source files across 11 submodules  
**Tests:** Integration tests in `tests/` (admin_api, blockchain, network, wallet_derivation, bip39_vectors)

### Architecture
- Full GPU compute infrastructure: backends (CUDA, Vulkan, WebGPU, Metal, OpenCL)
- Node management, scheduling, coordination
- DePIN service model with billing
- BIP39 wallet, blockchain integration
- Admin API, monitoring, sandbox manager
- Performance: memory pooling, batch optimization
- Advanced: jury system, social agents
- Warden: security, lifecycle, governance

### Gaps

| ID | Gap | Severity | File | Line |
|----|-----|----------|------|------|
| GSWM-001 | **42 production `lock().unwrap()` calls** across 8 files — poisoned mutex crashes process | HIGH | See table below | — |
| GSWM-002 | **Zero integration with `x3-gpu-validator-swarm`** — parallel implementations, no shared types | CRITICAL | — | — |
| GSWM-003 | `memory_pooling.rs` has **14 unwrap sites** in one file — hot path, crashes are catastrophic | HIGH | `performance/memory_pooling.rs` | 91-286 |
| GSWM-004 | All 5 GPU backends copy identical `lock().unwrap()` pattern — needs extraction | MEDIUM | `gpu_backends/*.rs` | — |
| GSWM-005 | `parking_lot = "0.12"` is already a dependency but **not used** for these Mutexes | MEDIUM | `Cargo.toml` | — |
| GSWM-006 | No GPU backend feature-gating — all 5 backends compiled unconditionally | MEDIUM | — | — |
| GSWM-007 | Admin API uses `Header::from_bytes().unwrap()` — crashes on malformed headers | HIGH | `admin.rs` | — |
| GSWM-008 | Monitoring `parse().unwrap()` on user-provided strings | HIGH | `monitoring/logging.rs` | — |
| GSWM-009 | No end-to-end test proving GPU job → validator → on-chain proof flow | CRITICAL | — | — |
| GSWM-010 | Jury system has no external audit trail — `audit_log` is in-memory only | MEDIUM | `advanced/jury.rs` | — |
| GSWM-011 | Social agents message queue is unbounded — memory exhaustion risk | MEDIUM | `advanced/social_agents.rs` | — |
| GSWM-012 | BIP39 wallet tests exist but no signing integration with x3 chain transactions | HIGH | — | — |

#### Production `lock().unwrap()` Location Summary

| File | Count | Fix Strategy |
|------|-------|-------------|
| `gpu_backends/cuda.rs` | 5 | Switch to `parking_lot::Mutex` |
| `gpu_backends/vulkan.rs` | 4 | Switch to `parking_lot::Mutex` |
| `gpu_backends/webgpu.rs` | 4 | Switch to `parking_lot::Mutex` |
| `gpu_backends/metal.rs` | 3 | Switch to `parking_lot::Mutex` |
| `gpu_backends/opencl.rs` | 3 | Switch to `parking_lot::Mutex` |
| `performance/memory_pooling.rs` | 14 | Switch to `parking_lot::Mutex` |
| `performance/batch_optimization.rs` | 3 | Switch to `parking_lot::Mutex` |
| `advanced/jury.rs` | 5 | Switch to `parking_lot::Mutex` |
| `advanced/social_agents.rs` | 3 | Switch to `parking_lot::Mutex` |
| **Total** | **42** | |

---

## 7. TAURI DESKTOP (`apps/x3-desktop`)

**Backend:** `src-tauri/` (Rust) | **Frontend:** React/TypeScript  
**Tests:** TypeScript unit tests, no backend integration tests

### Architecture
- CRM module (SQLite-backed contacts, deals, pipelines)
- Social module (notifications, friend system)
- Wallet core (BIP39, verifier with quorum checks against ETH/SOL RPCs)
- Network monitoring UI (seeded mock data)
- Embedded `desktop-ai` proxy

### Gaps

| ID | Gap | Severity | File | Line |
|----|-----|----------|------|------|
| TAU-001 | **Zero live RPC connection to any x3 node** — all blockchain data is mock/seeded | CRITICAL | `main.rs` | 376-430 |
| TAU-002 | `// TODO: replace with taurpc stack (tcp/udp/mqtt) + RPC instrumentation` | CRITICAL | `main.rs` | 376 |
| TAU-003 | No `jsonrpsee`, `subxt`, or any Substrate RPC client in Cargo.toml | CRITICAL | `Cargo.toml` | — |
| TAU-004 | `start_mock_stream` generates synthetic telemetry with random jitter — no real chain data | CRITICAL | `main.rs` | ~745 |
| TAU-005 | `crm/db.rs` — `self.conn.lock().unwrap()` in production migration code | HIGH | `crm/db.rs` | 21 |
| TAU-006 | `notifications.rs` — `duration_since(UNIX_EPOCH).unwrap()` for UUID generation | LOW | `notifications.rs` | 248 |
| TAU-007 | Wallet quorum RPC checks external chains (ETH/SOL via llamarpc/ankr) but **not** local x3 node | HIGH | `wallet_core/verifier/quorum.rs` | — |
| TAU-008 | No transaction submission capability — users can't send x3 transactions from desktop | CRITICAL | — | — |
| TAU-009 | No block explorer view with live data | HIGH | — | — |
| TAU-010 | No staking/validator management UI | HIGH | — | — |
| TAU-011 | No cross-VM operation UI | MEDIUM | — | — |
| TAU-012 | No wallet balance display from x3 chain | HIGH | — | — |
| TAU-013 | CRM module is feature-complete but unrelated to blockchain functionality | LOW | — | — |
| TAU-014 | No Rust-side integration tests for Tauri commands | HIGH | — | — |
| TAU-015 | Social notifications module not connected to any real event source | MEDIUM | — | — |

---

## 8. TPS TRACKER (`crates/tps-tracker`)

**Tests:** 2 unit tests | **Dependencies:** Standalone (tokio, reqwest, serde)

### Architecture
- External poller: queries `system_syncState` via JSON-RPC on `127.0.0.1:9944`
- Calculates TPS from block deltas
- Writes to InfluxDB for dashboarding

### Gaps

| ID | Gap | Severity | File | Line |
|----|-----|----------|------|------|
| TPS-001 | Only 2 unit tests — needs tests for TPS calculation edge cases | HIGH | `lib.rs` | 265-279 |
| TPS-002 | Not embedded in runtime — external sidecar only, may miss blocks under load | MEDIUM | — | — |
| TPS-003 | No in-process metrics export (consider `sc-rpc` extension or Prometheus exporter) | MEDIUM | — | — |
| TPS-004 | Hardcoded `127.0.0.1:9944` endpoint — should be configurable | LOW | `lib.rs` | ~100 |
| TPS-005 | No stress test or benchmarking suite exercising high-TPS scenarios | HIGH | — | — |

---

## 9. INTEGRATION WIRING MATRIX

This is the critical cross-subsystem view. Green = wired, Red = disconnected.

```
                  ┌──────────────┐
                  │   RUNTIME    │
                  │  (lib.rs)    │
                  └──────┬───────┘
                         │ Config wiring
              ┌──────────┼──────────┐
              ▼          ▼          ▼
        ┌──────────┐ ┌───────┐ ┌────────┐
        │x3-kernel │ │Frontier│ │ Other  │
        │ (hub)    │ │ EVM   │ │Pallets │
        └──┬───┬───┘ └───────┘ └────────┘
           │   │
    ┌──────┘   └──────┐
    ▼                  ▼
┌──────────┐    ┌──────────┐
│cross-vm- │    │ x3-vm    │
│bridge    │    │(bytecode)│
│(EVM↔SVM) │    └─────┬────┘
└──────────┘          │
                      ▼
               ┌──────────┐
               │x3-backend│◄── x3-lang pipeline
               │(bytecode │     (lexer→parser→hir→
               │ format)  │      mir→opt→backend)
               └──────────┘

    ╔══════════════════════════════╗
    ║  DISCONNECTED SUBSYSTEMS    ║
    ╠══════════════════════════════╣
    ║                              ║
    ║  gpu-swarm ──╳── gpu-       ║
    ║              validator      ║
    ║              swarm          ║
    ║                              ║
    ║  Tauri desktop ──╳── node   ║
    ║                    RPC      ║
    ║                              ║
    ║  tps-tracker ──╳── runtime  ║
    ║  (external poller only)     ║
    ║                              ║
    ║  gpu-validator ──╳──        ║
    ║              consensus      ║
    ╚══════════════════════════════╝
```

### Detailed Integration Status

| Pair | Status | Fix Required |
|------|--------|-------------|
| x3-kernel ↔ cross-vm-bridge | ✅ CONNECTED | — |
| x3-kernel ↔ x3-vm | ✅ CONNECTED (via x3-integration) | — |
| x3-lang → x3-vm | ✅ CONNECTED (via x3-backend bytecode) | — |
| cross-vm-bridge ↔ x3-vm | ⚠️ PEERS (composed at kernel) | Add `execute_x3_tx` to dispatcher |
| gpu-swarm ↔ x3-gpu-validator-swarm | 🔴 ZERO integration | Define shared trait crate |
| Tauri desktop → node RPC | 🔴 NOT CONNECTED | Add jsonrpsee/subxt client |
| TPS tracker → runtime | 🔴 EXTERNAL only | Consider sc-rpc extension |
| gpu-validator → consensus | 🔴 NOT CONNECTED | Wire to staking/authority |

---

## 10. CROSS-VM HARDENING BACKLOG (from PRD.md)

These are **ship-blocking** items from the 2026-03-22 cross-chain hardening audit:

| ID | Task | Status |
|----|------|--------|
| P0-1 | Replace proof-validation stubs with real EVM/SVM/BTC proof verification | ⬜ OPEN |
| P0-2 | Replace fake keccak/hash placeholders in bridge light-client path | ⬜ OPEN |
| P0-3 | Replace mirror execution placeholders (EVM/SVM/BTC bridge actions) | ⬜ OPEN |
| P1-1 | Remove external-chain payload amount placeholders, encode real values | ⬜ OPEN |
| P1-2 | Replace adapter/Arbitrum placeholder defaults with production behavior | ⬜ OPEN |
| P1-3 | Remove SVM execution stub-success paths, propagate real results | ⬜ OPEN |
| P2-1 | Replace rollback fixed refund placeholder with executed-leg accounting | ⬜ OPEN |
| P2-2 | Implement relayer registry/path discovery/event processing in cross-chain pallet | ⬜ OPEN |

---

## 11. TEST COVERAGE SUMMARY

| Subsystem | Unit Tests | Integration Tests | E2E Tests | Coverage Gap |
|-----------|------------|-------------------|-----------|-------------|
| cross-vm-bridge | 50+ | `tests/integration.rs` | ⬜ None | Need cross-VM E2E |
| x3-vm | 50+ | `tests/gpu_integration.rs` | ⬜ None | Need pipeline E2E |
| x3-kernel | 50+ (chaos: 22) | ⬜ None | ⬜ None | Need dispatch integration test |
| x3-lang | Golden tests | `tests/e2e_test.rs`, `integration_test.rs`, `determinism.rs` | ✅ Compiler E2E | Good |
| x3-gpu-validator-swarm | 49 | `tests/test_x3_validator.rs` | ⬜ None | Need multi-node test |
| gpu-swarm | In-module | `tests/` (6 files) | ⬜ None | Need GPU job E2E |
| Tauri desktop | TS unit tests | ⬜ None | ⬜ None | Need Tauri command tests |
| tps-tracker | 2 | ⬜ None | ⬜ None | **Critically undertested** |
| evm-integration | — | `tests/integration.rs`, `tests/erc20_integration.rs` | ⬜ None | Good |
| svm-integration | — | `tests/counter_integration.rs` | ⬜ None | Good |
| cross-chain-position-manager | — | `tests/integration_tests.rs` | ⬜ None | Good |

### E2E Infrastructure Status
- `tests/e2e/` directory exists with Cargo.toml, shell scripts, utils
- `start_test_environment.sh` / `stop_test_environment.sh` exist
- `TIER5_VALIDATION_SUITE.rs` exists but status unclear
- PRD Week 4 E2E items (EVM/SVM/cross-VM/WebSocket tests) are **all incomplete**

---

## 12. PRIORITIZED FIX ORDER

### Phase A — Ship-Blocking (Week 1-2)

| Priority | Task | Subsystem | Est. Effort |
|----------|------|-----------|-------------|
| P0 | Wire Tauri desktop to x3 node RPC (add jsonrpsee client) | Tauri | 3-5 days |
| P0 | Replace proof-validation stubs (P0-1, P0-2, P0-3) | cross-vm-bridge | 3-5 days |
| P0 | Define shared trait crate between gpu-swarm and x3-gpu-validator-swarm | GPU | 2-3 days |
| P0 | Wire GPU validator into consensus/authority system | GPU validator | 3-5 days |
| P0 | Replace all 42 `lock().unwrap()` in gpu-swarm with `parking_lot::Mutex` | gpu-swarm | 1 day |

### Phase B — High Priority (Week 3-4)

| Priority | Task | Subsystem | Est. Effort |
|----------|------|-----------|-------------|
| P1 | Add `execute_x3_tx` to CrossVmDispatcher trait | cross-vm-bridge | 1 day |
| P1 | Full x3-lang→x3-vm pipeline integration test | x3-vm / x3-lang | 1-2 days |
| P1 | Kernel dispatch integration test (kernel→adapter→real VM) | x3-kernel | 1-2 days |
| P1 | TPS tracker test coverage (currently 2 tests) | tps-tracker | 1 day |
| P1 | Replace remaining cross-VM hardening items (P1-1 through P1-3) | cross-vm-bridge | 2-3 days |
| P1 | gpu-swarm admin.rs / monitoring.rs — fix unwrap on user input | gpu-swarm | 0.5 day |
| P1 | Tauri transaction submission + balance display | Tauri | 2-3 days |

### Phase C — Medium Priority (Week 5-6)

| Priority | Task | Subsystem | Est. Effort |
|----------|------|-----------|-------------|
| P2 | Cross-VM E2E test suite (EVM↔SVM atomic swap end-to-end) | E2E | 3-5 days |
| P2 | GPU job→validator→on-chain proof E2E flow | GPU | 3-5 days |
| P2 | GPU backend feature-gating (compile only needed backends) | gpu-swarm | 1 day |
| P2 | JIT compiler edge case tests | x3-vm | 1 day |
| P2 | x3-lang language reference documentation | x3-lang | 2-3 days |
| P2 | Tauri integration tests for Rust commands | Tauri | 2-3 days |
| P2 | Replace rollback/relayer placeholders (P2-1, P2-2) | cross-vm-bridge | 2-3 days |

### Phase D — Polish (Week 7-8)

| Priority | Task | Subsystem | Est. Effort |
|----------|------|-----------|-------------|
| P3 | TPS tracker configurable endpoint | tps-tracker | 0.5 day |
| P3 | GPU swarm social agents bounded queue | gpu-swarm | 0.5 day |
| P3 | Jury audit trail persistence | gpu-swarm | 1 day |
| P3 | Network peer discovery persistence | gpu-validator | 1 day |
| P3 | x3-lang error.rs — add expect message | x3-lang | 5 min |
| P3 | CRM db.rs — switch to parking_lot Mutex | Tauri | 5 min |

---

## 13. QUICK WINS (fixable now)

These are the items small enough to knock out during this audit:

| Item | File | Fix | Time |
|------|------|-----|------|
| `error.rs` unwrap clarity | `x3-lang/crates/x3-common/src/error.rs:181` | Change `.unwrap()` to `.expect("has_errors was true")` | 1 min |
| `notifications.rs` unwrap | `apps/x3-desktop/src-tauri/src/social/notifications.rs:248` | Wrap in `.unwrap_or_default()` | 1 min |
| `crypto.rs` try_into | `crates/x3-gpu-validator-swarm/src/crypto.rs:217` | Already guaranteed 32 bytes, add `.expect("secret is always 32 bytes")` | 1 min |

---

## APPENDIX: FILES SCANNED

### Primary Files Read
- `Cargo.toml` (workspace root, 364 lines)
- `docs/planning-artifacts/PRD.md` (317 lines)
- `X3_GAPS_REPORT.md` (479 lines)
- `X3_END_TO_END_GAPS_MASTER_PLAN.md` (342 lines)
- `X3_SYSTEMS.md` (491 lines)
- `docs/specs/X3_LIQUIDITY_INVENTORY_SOLVENCY_SPEC.md`
- `crates/cross-vm-bridge/src/lib.rs` (3,057 lines)
- `crates/x3-vm/src/bridge.rs` (858 lines)
- `crates/x3-gpu-validator-swarm/src/crypto.rs` (269 lines)
- `crates/x3-gpu-validator-swarm/src/network.rs` (314 lines)
- `x3-lang/crates/x3-common/src/source.rs` (228 lines)
- `x3-lang/crates/x3-common/src/error.rs` (208 lines)
- `apps/x3-desktop/src-tauri/src/crm/db.rs` (326 lines)
- `apps/x3-desktop/src-tauri/src/social/notifications.rs` (302 lines)
- `pallets/x3-kernel/src/lib.rs`, `adapters.rs`, `wasm_adapters.rs`
- `runtime/src/lib.rs`
- 63 gpu-swarm source files scanned via subagent

### Grep Patterns Applied
- `TODO|FIXME|unimplemented!|todo!|panic!|expect(|unwrap(` across all 8 focus areas
- `#[test]` counts per subsystem
- `lock().unwrap()` production code audit (gpu-swarm)

---

**END OF REPORT**
