# X3 Chain Repository Structure

**Document:** Directory & Module Layout  
**Date:** 2026-03-15  
**Scope:** Top-level structure and key paths

---

## Top-Level Layout

```
/home/lojak/Desktop/x3-chain-master
├── node/                     # Substrate node (service, RPC, networking)
├── runtime/                  # WASM runtime (pallet composition)
├── pallets/                  # FRAME pallets (on-chain modules)
├── crates/                   # Core Rust crates (VMs, tools, services)
├── apps/                     # End-user apps and services
├── packages/                 # TypeScript/JS SDKs and adapters
├── contracts/                # Solidity contracts (EVM)
├── programs/                 # Solana programs / SVM utilities
├── x3-lang/                  # X3 language toolchain
├── infra/, infra-structure/  # Infrastructure, deployment, dashboards
├── deployment/               # Docker/K8s and monitoring stacks
├── scripts/                  # Operational scripts and tooling
├── tests/, integration-tests/ # Test suites and harnesses
└── tools/                    # Utility tools (incl. GSD)
```

---

## Key Directories (with examples)

### Runtime & Node
- `node/src/main.rs` — Substrate node entry point
- `node/src/service.rs` — Service wiring and consensus stack
- `runtime/src/lib.rs` — Runtime composition
- `runtime/src/precompiles.rs` — EVM precompiles

### Pallets
FRAME pallets live under `pallets/` and represent on-chain modules:
- `pallets/x3-kernel/` — Core kernel
- `pallets/x3-atomic-kernel/` — Cross-VM atomic execution
- `pallets/svm-runtime/` — SVM runtime pallet
- `pallets/x3-da/` — Data availability

### Core Crates
Primary Rust crates in `crates/`:
- `crates/evm-integration/` — EVM integration
- `crates/svm-integration/` — SVM integration
- `crates/cross-vm-bridge/` — Cross-VM coordination
- `crates/x3-indexer/` — Indexer service
- `crates/x3-gateway/` — Gateway service

### Applications & SDKs
- `apps/x3-desktop/` — Desktop app (Tauri + TS)
- `apps/wallet/` — Wallet UI
- `apps/dex/` — DEX UI
- `packages/ts-sdk/` — TypeScript SDK
- `packages/blockchain-connector/` — Connector + OpenAPI docs

### Smart Contract & Program Artifacts
- `contracts/` — Solidity contracts + Hardhat configs
- `programs/` — Solana program crates (SVM)
- `botchain-tri-vm-genesis/hardhat/` — Hardhat workspace

### Infrastructure & Deployment
- `deployment/monitoring/` — Prometheus/Grafana/Loki stack
- `docker-compose.yml` — Local monitoring + services
- `k8s-deployment.yaml` — Kubernetes deployment template

---

## Workspace Files

Common repo-level files and artifacts:
- `Cargo.toml` — Workspace root (many members)
- `package.json` — JS/TS monorepo tooling
- `docker-compose*.yml` — Local/monitoring stacks
- `RUN_ALL_TESTS.sh` — Test orchestration
- `run-dev-node.sh` / `run-production-node.sh` — Node runner scripts

