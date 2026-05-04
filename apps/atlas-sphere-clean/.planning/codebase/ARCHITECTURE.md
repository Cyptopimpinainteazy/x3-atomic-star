# X3 Chain Architecture

**Document:** Architecture Overview  
**Date:** 2026-03-15  
**Scope:** Runtime, node, pallets, and cross-VM execution flow

---

## System Overview

X3 Chain is a Substrate-based L1 with dual execution environments (EVM + SVM) and a large constellation of off-chain services, SDKs, and apps. The core execution path is the Substrate node (`node/`) running a WASM runtime (`runtime/`) composed of pallets (`pallets/`). Cross-VM execution is coordinated via runtime pallets plus supporting crates in `crates/`.

```
┌────────────────────────────────────────────────────────────┐
│                         Clients / Apps                      │
│  Web apps, desktop apps, SDKs, bots                          │
│  `apps/`, `packages/`, `crates/x3-sdk`, `crates/x3-cli`       │
└───────────────┬─────────────────────────────────────────────┘
                │ RPC / WebSocket
┌───────────────▼─────────────────────────────────────────────┐
│                          Node Layer                          │
│  Substrate node, RPC, networking, consensus                  │
│  `node/src/main.rs`, `node/src/service.rs`, `node/src/rpc.rs` │
└───────────────┬─────────────────────────────────────────────┘
                │ Runtime calls
┌───────────────▼─────────────────────────────────────────────┐
│                         Runtime Layer                        │
│  WASM runtime + pallets                                      │
│  `runtime/src/lib.rs`, `pallets/*`                            │
└───────────────┬─────────────────────────────────────────────┘
                │ Cross-VM / Execution
┌───────────────▼─────────────────────────────────────────────┐
│                      Execution Environments                  │
│  EVM (Frontier) + SVM (Solana RBPF)                           │
│  `crates/evm-integration`, `crates/svm-integration`           │
└───────────────┬─────────────────────────────────────────────┘
                │ Off-chain services
┌───────────────▼─────────────────────────────────────────────┐
│                   Indexers / Sidecars / Gateways             │
│  `crates/x3-indexer`, `crates/x3-sidecar`, `crates/x3-gateway`│
└─────────────────────────────────────────────────────────────┘
```

---

## Core Components

### Node (Substrate service)
- **Entry point:** `node/src/main.rs`
- **Service wiring:** `node/src/service.rs`
- **RPC:** `node/src/rpc.rs`, `node/src/rpc_frontier.rs`, `node/src/rpc_middleware.rs`
- **Consensus extensions:** `node/src/flash_finality.rs`

### Runtime (WASM)
- **Primary runtime definition:** `runtime/src/lib.rs`
- **EVM precompiles:** `runtime/src/precompiles.rs`
- **Fraud proofs:** `runtime/src/fraud_proofs/`

### Pallets (On-chain modules)
Core pallets live in `pallets/` and are composed into the runtime. Examples:
- Kernel + sequencing: `pallets/x3-kernel/`, `pallets/x3-sequencer/`
- Cross-VM orchestration: `pallets/x3-atomic-kernel/`, `pallets/svm-runtime/`
- Governance & economics: `pallets/governance/`, `pallets/treasury/`, `pallets/x3-governance/`

### Cross-VM Execution
Runtime uses both EVM and SVM integration crates:
- **EVM:** `crates/evm-integration/`, `pallets/x3-kernel/`
- **SVM:** `crates/svm-integration/`, `pallets/svm-runtime/`
- **Bridge coordination:** `crates/cross-vm-bridge/`, `crates/cross-vm-coordinator/`

### Off-chain Services
Typical service crates include:
- **Indexer:** `crates/x3-indexer/`
- **Gateway:** `crates/x3-gateway/`
- **Sidecar:** `crates/x3-sidecar/`
- **Analytics service:** `apps/analytics/analytics-service/`

---

## Data Flow (High Level)

1. **Client submits transaction** via RPC or WebSocket  
   `node/src/rpc.rs` exposes the JSON-RPC endpoints.
2. **Node routes transaction** through Substrate transaction pool  
   `node/src/service.rs` wires the service stack.
3. **Runtime dispatch** executes extrinsic through pallet logic  
   `runtime/src/lib.rs` + `pallets/*`.
4. **Cross-VM execution** invokes EVM/SVM pathways  
   `crates/evm-integration/` and `crates/svm-integration/`.
5. **Off-chain services** index and expose data  
   `crates/x3-indexer/`, `crates/x3-sidecar/`, `crates/x3-gateway/`.

---

## Key Boundaries & Interfaces

| Boundary | Interface | Notes |
|----------|-----------|-------|
| Node ↔ Runtime | Substrate runtime API | Runtime compiled to WASM (`runtime/src/lib.rs`) |
| Runtime ↔ Pallets | FRAME macros | Pallets under `pallets/` |
| Runtime ↔ EVM | Frontier pallets + precompiles | `runtime/src/precompiles.rs`, `crates/evm-integration/` |
| Runtime ↔ SVM | SVM runtime pallet | `pallets/svm-runtime/`, `crates/svm-integration/` |
| Node ↔ Off-chain services | JSON-RPC / WS | `node/src/rpc.rs`, `crates/x3-gateway/` |

---

## Architectural Patterns

- **Substrate FRAME modularity:** Pallets provide composable features (`pallets/*`).
- **Dual-VM orchestration:** Atomic logic across EVM and SVM (`pallets/x3-atomic-kernel/`, `crates/cross-vm-bridge/`).
- **Service constellation:** Indexers, gateways, and sidecars extend the chain with external APIs (`crates/x3-indexer/`, `crates/x3-sidecar/`).

---

## Known Architectural Hotspots

- RPC/WS surface area is concentrated in `node/src/rpc.rs`.
- Cross-VM correctness hinges on coordination crates (`crates/cross-vm-bridge/`, `crates/cross-vm-coordinator/`).
- Runtime complexity is centralized in `runtime/src/lib.rs` and `pallets/x3-atomic-kernel/`.

