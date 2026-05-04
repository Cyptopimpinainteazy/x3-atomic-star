# X3 Chain Technology Stack

**Document:** Technology Stack Analysis  
**Date:** 2026-03-15  
**Scope:** Comprehensive tech stack mapping for X3 Chain L1 blockchain  

---

## Overview

X3 Chain is a Substrate-based Layer-1 blockchain enabling native interoperability between Ethereum Virtual Machine (EVM) and Solana Virtual Machine (SVM) execution. The codebase is predominantly **Rust-based** with supporting **TypeScript/JavaScript**, **Python**, and **Go** components for SDKs, tooling, and applications.

**Core Architecture:**
- **Blockchain Runtime:** Substrate/Polkadot
- **Consensus:** GRANDPA (Grandpa) + Aura
- **VMs:** EVM (Frontier) + SVM (Solana RBPF)
- **Execution:** Dual-VM atomic operations with cross-VM bridge
- **Data Layer:** PostgreSQL + blockchain indexing
- **Monitoring:** Prometheus + Grafana
- **Deployment:** Docker/Kubernetes + Systemd

---

## Languages & Runtimes

### Primary Language: Rust
- **Edition:** 2021
- **Target:** `wasm32-unknown-unknown` (blockchain WASM), `x86_64-unknown-linux-gnu` (native)
- **Files:** `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`
- **Key Constraint:** `solana_rbpf` requires std-only for now (no-std version pending)

### Secondary Languages
- **TypeScript/JavaScript:** Frontend SDKs, tooling, web apps
- **Python:** Utilities, LLM integration, testing frameworks
- **Go:** GUI applications (Fyne framework)
- **Solidity:** Smart contracts (EVM)
- **Solana Program Rust:** Cross-chain program interface (if needed)

### Runtime Environments
- **Substrate Node:** Custom X3 Chain node (`x3-chain-node`)
- **WASM VM:** Blockchain runtime compiled as WASM binary
- **Native EVM:** Frontier pallet with SputnikVM + real EVM execution
- **Native SVM:** Solana RBPF interpreter for program execution
- **Node.js:** TypeScript/JavaScript services (v18+)
- **Python:** v3.8+ for utilities

---

## Core Framework & Dependencies

### Blockchain Framework: Substrate/Polkadot SDK

**Configuration Files:**
- `Cargo.toml` - Workspace root with 80+ member crates/pallets
- `workspace.dependencies` - Centralized dependency management
- Workspace resolver: 2
- Using patches from rev `948fbd2` of paritytech/substrate

**Key Substrate Versions & Components:**

#### Frame System & Support
- `frame-support` - Pallet macro framework
- `frame-system` - Core blockchain system pallet
- `frame-executive` - Block execution logic
- `frame-benchmarking` - Performance testing
- `frame-benchmarking-cli` - CLI for benchmarking
- `frame-try-runtime` - Migration testing

#### Consensus & Finality
- `pallet-aura` - Authority round consensus
- `pallet-grandpa` - GRANDPA finality gadget
- `sp-consensus-aura` - Aura consensus primitives
- `sp-consensus-grandpa` - GRANDPA primitives

#### Core Pallets (Blockchain Features)
- `pallet-timestamp` - Block time tracking
- `pallet-balances` - Token balance management
- `pallet-transaction-payment` - Fee system
- `pallet-scheduler` - Block scheduling
- `pallet-preimage` - Preimage storage
- `pallet-collective` - Collective decision-making
- `pallet-sudo` - Superuser pallet (optional, runtime-gated)

#### Primitives & Types
- `sp-core` - Cryptographic primitives, types
- `sp-runtime` - Runtime types, version, weights
- `sp-api` - Runtime API traits
- `sp-io` - Host function interfaces
- `sp-std` - No-std compatible standard library
- `sp-block-builder` - Block building primitives
- `sp-blockchain` - Blockchain database primitives
- `sp-keystore` - Key management
- `sp-session` - Session management
- `sp-timestamp` - Timestamp type
- `sp-transaction-pool` - Transaction pool types
- `sp-weights` - Weight system
- `sp-version` - Version information

#### Client (Node) Crates
- `sc-cli` - Command-line interface
- `sc-client-api` - Client abstraction
- `sc-consensus` - Core consensus engine
- `sc-consensus-aura` - Aura implementation
- `sc-consensus-grandpa` - GRANDPA implementation
- `sc-executor` - WASM executor
- `sc-network` - Networking layer, P2P
- `sc-network-gossip` - Gossip protocol
- `sc-rpc` - RPC server
- `sc-rpc-api` - RPC method definitions
- `sc-service` - Service configuration (with RocksDB)
- `sc-telemetry` - Telemetry system
- `sc-transaction-pool` - Transaction pool implementation
- `sc-keystore` - Keystore management
- `sc-basic-authorship` - Block authoring

#### RPC System
- `jsonrpsee` - v0.22.5 JSON-RPC implementation
- `substrate-frame-rpc-system` - Frame system RPC
- `pallet-transaction-payment-rpc` - Fee RPC
- `fp-rpc` - Frontier RPC extensions

### Virtual Machines

#### EVM Integration (Frontier)
**Files:** `crates/evm-integration/Cargo.toml`, `runtime/Cargo.toml`

- `pallet-evm` - EVM execution pallet (Frontier branch `polkadot-v1.1.0`)
- `pallet-ethereum` - Ethereum pallet
- `pallet-base-fee` - Base fee mechanism
- `pallet-evm-precompile-simple` - Basic precompiles
- `pallet-evm-precompile-modexp` - Modular exponentiation
- `pallet-evm-precompile-sha3fips` - SHA3 FIPS
- `fp-evm` - EVM types
- `fp-self-contained` - Self-contained transactions
- `fp-rpc` - Frontier RPC
- `fc-rpc` - RPC client
- `fc-rpc-core` - RPC core types
- `fc-db` - RPC database layer
- `fc-mapping-sync` - Block/transaction mapping
- `fc-storage` - Storage traits
- `evm` - SputnikVM (no-std EVM interpreter from `rust-blockchain/evm` rev `b7b82c7e1fc57b7449d6dfa6826600de37cc1e65`)
- `ethereum` - Ethereum types (v0.14)
- `ethereum-types` - Core Ethereum types

#### SVM Integration (Solana)
**Files:** `crates/svm-integration/Cargo.toml`

- `solana_rbpf` - v0.8 - Solana BPF runtime (std-only for execution)
- `solana-program` - v3.0.0 - Solana program library (dev/test)
- `solana-program-test` - v3.0.0 - Test harness
- `solana-sdk` - v3.0.0 - SDK utilities (dev/test)

### Cryptography & Serialization

- `parity-scale-codec` - v3.6.5 - SCALE encoding/decoding
- `scale-info` - v2.11.1 - Type metadata
- `curve25519-dalek` - v3.2.1 - Elliptic curve cryptography
- `sp-core-hashing` - v9.0 - Hashing primitives
- `blake2` - v0.10 - BLAKE2 hash
- `hex` - v0.4.3 - Hex encoding/decoding
- `serde` - v1.0.195 - Serialization framework
- `serde_json` - v1.0.111 - JSON support

### Utilities & Standard Library Compatibility

- `uuid` - v1.8.0 - UUID generation
- `tracing` - v0.1.37 - Structured logging/observability
- `anyhow` - v1.0 - Error handling
- `tokio` - v1.0 - Async runtime (full features)
- `futures` - v0.3.30 - Async utilities
- `log` - v0.4.20 - Logging facade
- `once_cell` - v1.19.0 - Lazy static cells
- `thiserror` - v1.0.51 - Error derive
- `clap` - v4.4.18 - CLI argument parsing

### Consensus & Networking

- `proc-macro2` - v1.0.103 - Procedural macro support
- `base64ct` - v1.6.0 (pinned) - Base64 encoding (avoids edition2024 requirement)
- `ahash` - v0.8.11 - Fast hashing (getrandom 0.2 compatible)
- `tempfile` - v3.8.1 - Temporary file handling
- `hex-literal` - v0.4.1 - Compile-time hex literals

---

## X3 Chain Pallets

**Location:** `pallets/`

Core pallets providing X3-specific blockchain features:

### Kernel & Core
- `pallet-x3-kernel` - Core execution kernel
- `pallet-x3-atomic-kernel` - Atomic operation kernel for cross-VM coordination
- `pallet-x3-sequencer` - Sequencer pallet
- `pallet-x3-da` - Data availability layer

### Virtual Machine & Execution
- `pallet-svm-runtime` - Solana Virtual Machine runtime integration
- `pallet-x3-verifier` - Proof verification
- `pallet-fraud-proofs` - Fraud proof handling
- `pallet-private-execution` - Private transaction execution

### Trading & Economics
- `pallet-atomic-trade-engine` - Atomic swap orchestration
- `pallet-x3-settlement-engine` - Trade settlement
- `pallet-treasury` - Treasury management
- `pallet-x3-coin` - Native currency
- `pallet-x3-domain-registry` - Domain/name registry
- `pallet-x3-governance` - Governance
- `pallet-x3-invariants` - Constitutional invariants enforcement

### Advanced Features
- `pallet-agent-accounts` - Agent account management
- `pallet-agent-memory` - Agent state memory
- `pallet-evolution-core` - Evolution/upgrade mechanisms
- `pallet-swarm` - Swarm coordination
- `pallet-depin-marketplace` - DePIN marketplace
- `pallet-x3-jury-anchor` - Jury anchoring for dispute resolution
- `pallet-meme-overlord` - Meme integration (novelty feature)

---

## X3 Crates (Rust Libraries)

**Location:** `crates/`

### VM & Compilation
- `x3-lexer` - Tokenization for X3 language
- `x3-parser` - Syntax parsing
- `x3-ast` - Abstract syntax tree
- `x3-hir` - High-level intermediate representation
- `x3-mir` - Mid-level intermediate representation
- `x3-semantics` - Semantic analysis
- `x3-typeck` - Type checking
- `x3-opt` - Optimization passes
- `x3-backend` - Code generation
- `x3-vm` - Virtual machine runtime
- `x3-compiler` - Integrated compiler
- `x3-proof` - Proof generation

### Infrastructure & Integration
- `x3-evm-integration` - EVM bridge implementation
- `x3-svm-integration` - SVM bridge implementation
- `x3-cross-vm-bridge` - Cross-VM coordination
- `x3-cross-vm-coordinator` - Orchestration and coordination
- `x3-bridge-adapters` - Adapter implementations
- `x3-external-chains` - Multi-chain support

### Core Services
- `x3-gateway` - REST/GraphQL API gateway (Axum + Async GraphQL)
- `x3-indexer` - Blockchain indexer (Subxt + PostgreSQL)
- `x3-rpc` - RPC server extensions
- `x3-cli` - Command-line interface
- `x3-sdk` - Rust SDK for client interaction
- `x3-lsp` - Language server protocol support
- `x3-wallet` - Wallet management
- `x3-wallet-cli` - CLI wallet tool

### Financial & Specialized
- `x3-dex` - Decentralized exchange
- `x3-swap-router` - Swap routing
- `x3-flashloan` - Flash loan implementation
- `x3-slash` - Slashing/penalty system
- `x3-fees` - Fee calculation
- `x3-intent` - Intent-based execution
- `x3-staking-analytics` - Staking metrics
- `x3-economics` - Economic models

### Performance & Consensus
- `x3-turbine` - High-performance block propagation
- `flash-finality` - Flash finality mechanism
- `poh-generator` - Proof of History generator
- `x3-launch-validator` - Validator initialization
- `x3-constitution` - Constitutional rules engine

### Advanced Execution
- `x3-evolution` - Evolutionary improvements
- `parallel-proposer` - Parallel block proposal
- `private-mempool` - Private transaction pool
- `confidential-gpu` - GPU-accelerated confidential compute
- `contention-predictor` - Transaction contention prediction
- `atomic-swap-orchestrator` - Atomic swap coordination

### Utilities
- `x3-common` - Common utilities
- `x3-agent` - Agent framework
- `x3-court` - Judicial/court system
- `x3-marketplace` - Generic marketplace
- `x3-stdlib` - Standard library for X3 lang
- `x3-oracle` - Oracle integration
- `x3-bot` - Trading bot
- `tps-tracker` - Throughput tracking
- `x3-sidecar` - Sidecar service
- `invariant-macros` - Macro utilities

### Experimental/Research
- `quantum-swarm` - Quantum-resistant primitives
- `quantum-crypto` - Quantum cryptography
- `dream-mining` - Speculative mining
- `voice-to-x3` - Voice interface
- `apotheosis-tx` - Advanced transaction types
- `chronos-flash` - Time-based flash mechanisms
- `x3-dns-server` - DNS service
- `gpu-swarm` - GPU computation network
- `x3-gpu-validator-swarm` - GPU-assisted validation

---

## Runtime Configuration

**Location:** `runtime/Cargo.toml`, `src/lib.rs`

### Binary Features
- WASM library (`crate-type = ["rlib", "cdylib"]`)
- Buildscript: `build.rs` for runtime configuration
- Metadata docs target: `x86_64-unknown-linux-gnu`

### Runtime Features (gated compilation)
- `std` - Standard library support
- `runtime-benchmarks` - Enable benchmarking
- `try-runtime` - Testing runtime migrations
- `frontier` - EVM integration
- `native-real-vm-adapters` - Real VM adapter implementations

### Build Profile Settings
```toml
[profile.release]
codegen-units = <N>  # Reduced to avoid LLVM crashes
```

---

## Node Configuration

**Location:** `node/Cargo.toml`, `src/main.rs`

### Binary
- Binary name: `x3-chain-node`
- Default run: `x3-chain-node`
- Supports multiple runtime features

### Database Backend
- RocksDB (default storage via `sc-service`)
- Configurable pruning mode
- Archive mode for public nodes

### RPC Configuration
- JSON-RPC 2.0 via jsonrpsee
- HTTP endpoint: `9933` (dev/testnet), configurable (production)
- WebSocket endpoint: `9944` (dev/testnet), configurable (production)
- Frontier EVM RPC methods (when `frontier` feature enabled)

### Consensus Configuration
- **Proposer:** Aura (Authority Round)
- **Finality:** GRANDPA (Grandpa)
- **Block Time:** 200ms (configurable)

### Network & P2P
- Libp2p with Yamux
- Gossip protocol for transaction distribution
- Configured via chain-spec JSON files

---

## Frontend & SDK Stack

**Location:** `packages/`, `apps/`

### TypeScript/JavaScript SDKs

#### Core Packages
- `@x3-chain/ts-sdk` - TypeScript SDK
  - Dependencies: Polkadot.js v14.0.1, Solana Web3.js v1.87.0, Ethers v6.8.0, Axios v1.6.0
  - Runtime: Node.js 18+
  
#### Specialized Packages
- `atomic-swap-sdk` - Atomic swap operations
- `blockchain-adapter` - Multi-chain abstraction
- `blockchain-connector` - Blockchain connection manager
- `polkawallet-plugin` - PolkaWallet integration
- `polkawallet-bridge-adapter` - Bridge operations
- `x3-marketplace-sdk` - Marketplace API

### Python SDK
- `py-sdk` - Python bindings

### Frontend Applications
- `x3-desktop` - Desktop application (Tauri-based)
- `x3-intelligence` - Intelligent dashboard
- `wallet` - Web wallet
- `explorer` - Block explorer
- `dex` - Decentralized exchange UI
- `dashboard` - Analytics dashboard
- `analyzer` - Transaction analyzer
- `validators` - Validator management
- `llm-endpoint-checker` - Gateway monitoring GUI (Fyne + Go)

### API & Analytics
- `analytics-service` - Analytics backend (Actix-web + PostgreSQL)
  - Dependencies: Tokio, Tokio-PostgreSQL, Prometheus, Tracing

---

## Database & Indexing

### PostgreSQL
- **Version:** 15+ (Alpine images)
- **Client:** `sqlx` (async Rust driver) or `tokio-postgres` (async)
- **Connection Pool:** `deadpool-postgres` (v0.12+)

### Database Services
- `x3-indexer` - Substrate block indexer
  - Indexes blockchain events into PostgreSQL
  - Subxt client for Substrate RPC
  - REST metrics exposed via Axum
  
- `analytics-service` - Off-chain analytics
  - User behavior tracking
  - Financial metrics aggregation

### Migration System
- **Tool:** Alembic (Python)
- **Configuration:** `alembic.ini`
- **Scripts:** `alembic/versions/`
- **Database:** PostgreSQL

---

## Monitoring & Observability

### Metrics Collection
- **Prometheus:** v2.x
- **Configuration:** `prometheus.yml`
- **Retention:** 30 days (default, configurable)

### Metrics Sources
- Node metrics (EVM execution, consensus)
- RPC request metrics
- P2P network metrics
- Pallet-specific custom metrics

### Visualization
- **Grafana:** Latest image
- **Dashboards:**
  - `grafana-llm-dashboard.json` - LLM service metrics
  - Custom dashboards via `grafana-dashboards.yml`
- **Datasources:** Prometheus
- **Plugins:** Grafana Piechart panel

### LLM Integration
- **Ollama:** Local LLM inference (port 11434)
- **LLM Router:** Custom service routing queries
  - Supports Ollama local + OpenRouter cloud fallback
  - Configuration via `llm-config.json`
- **Telemetry:** Prometheus metrics from LLM service

---

## Build & Deployment

### Build System
- **Cargo** - Rust workspace manager
- **Workspace resolver:** v2 (faster, better UX)
- **Compiler:** Rust 1.85-slim (Docker base)

### Build Targets
- `wasm32-unknown-unknown` - WASM runtime compilation
- `x86_64-unknown-linux-gnu` - Native Linux binary

### Docker & Containerization
**Files:**
- `Dockerfile` - Multistage build for `x3-chain-node`
- `docker-compose.yml` - Basic orchestration
- `docker-compose.monitoring.yml` - Monitoring stack
- `docker-compose.prod.yml` - Production cluster
- `docker-compose.staging.yml` - Staging cluster

**Base Images:**
- Builder: `docker.io/library/rust:1.85-slim`
- Runtime: `docker.io/library/ubuntu:20.04`
- Services: Postgres 15-alpine, Prometheus, Grafana, Ollama (latest)

### Kubernetes
- **Configuration:** `k8s-deployment.yaml`
- **Helm charts:** `deployment/helm/`

### Package Management
### Workspace Dependency Management
- **Root package.json:** Monorepo orchestration
- **Workspace packages:** TypeScript/JavaScript packages managed via npm workspaces
- **Testing:** Jest, Vitest, Cypress, Storybook

---

## Configuration Management

### Environment Configuration
**Files:**
- `.env.example` - Template with all variables
- `.env` - Runtime (git-ignored)
- `.env.apps.template` - App-specific variables
- `.env.production` - Production overrides
- `CONFIG.md` - Configuration reference guide

### Supported Networks
- Arbitrum (mainnet + Goerli testnet)
- Base
- ZkSync Era
- Optimism
- Polygon
- Linea

### RPC Provider Configuration
**Alchemy Endpoints:**
- Arbitrum Mainnet: `https://arb-mainnet.g.alchemy.com/v2/`
- Arbitrum Goerli: `https://arb-goerli.g.alchemy.com/v2/`

**DRPC Endpoints:**
- Base: `https://lb.drpc.org/base/`
- Arbitrum: `https://lb.drpc.org/arbitrum/`
- ZkSync Era: `https://lb.drpc.org/zksync-era/`

**Ankr Endpoints:**
- Base: `https://rpc.ankr.com/base/`
- Arbitrum: `https://rpc.ankr.com/arbitrum/`
- ZkSync Era: `https://rpc.ankr.com/zksync_era/`

---

## Smart Contracts & Solidity

**Location:** `contracts/`

### EVM Smart Contracts
- `@openzeppelin/` - OpenZeppelin contracts (Solidity 5.6.1)
- `core/` - Core protocol contracts
- `token/` - Token standards
- `dex/` - DEX contracts
- `lending/` - Lending protocol
- `staking/` - Staking mechanisms
- `treasury/` - Treasury contracts
- `evm-hello/` - Example contracts
- `ai-swarm/` - AI swarm contracts
- `launchpad/` - Token launch contracts

### Build Tools
- Forge/Foundry (implied from remappings.txt)
- **Configuration:** `remappings.txt` - Contract import mappings

---

## Testing & Validation

### Test Frameworks
- **Rust:** Cargo test framework (built-in)
- **TypeScript/JavaScript:** Jest, Vitest, Cypress
- **Python:** Pytest (assumed)

### Test Organization
- **Unit tests:** Inline in source files (Rust)
- **Integration tests:** `integration-tests/`, `tests/`
- **E2E tests:** Cypress tests, bash scripts
- **Scripts:** `run_e2e_tests.sh`, `validate-test-framework.sh`

### Benchmarking
- **Rust:** Frame benchmarking macros
- **Command:** `cargo benchmark` or frame-benchmarking-cli
- **Reports:** `.benchmarks/` directory

---

## Development Tools & Infrastructure

### IDE & LSP
- **X3 LSP:** `x3-lsp` crate
- **Language:** X3 (custom blockchain language)
- **Editor Support:** Vim, VS Code (via LSP)

### CLI Tools
- `x3-cli` - Main CLI tool
  - Compiler invocation
  - Deployment
  - Transaction signing
  - Account management
  - REPL for testing

### Version Control
- **SCM:** Git
- **Hooks:** `.githooks/`, `.pre-commit-config.yaml`
- **Git modules:** `.gitmodules` (submodule management)

### Deployment & Configuration
- **Location:** `deployment/`
- **Chain specs:** `deployment/chain-specs/`
  - Development spec
  - Local testnet spec
  - Staging spec
  - Production spec
- **Systemd units:** `deployment/systemd/`
- **Database init:** `deployment/sql/`
- **Kubernetes:** `deployment/kubernetes/` + `deployment/helm/`
- **Ansible:** `deployment/inventory.yaml` (infrastructure provisioning)

---

## Key Configuration Files Summary

| File | Purpose | Format |
|------|---------|--------|
| `Cargo.toml` | Workspace manifest | TOML |
| `Cargo.lock` | Dependency lock | Lock |
| `package.json` | npm/Node workspace | JSON |
| `Dockerfile` | Container build | Dockerfile |
| `docker-compose*.yml` | Service orchestration | YAML |
| `k8s-deployment.yaml` | Kubernetes config | YAML |
| `.env.example` | Environment template | dotenv |
| `prometheus.yml` | Metrics configuration | YAML |
| `grafana-dashboards.yml` | Grafana provisioning | YAML |
| `alembic.ini` | DB migration config | INI |
| `jest.config.cjs` | Jest test config | JavaScript |
| `remappings.txt` | Solidity import paths | Text |
| `swarm-config.toml` | Swarm configuration | TOML |
| `CONFIG.md` | Node configuration guide | Markdown |

---

## Tech Stack Matrix

| Category | Technology | Version | Use Case |
|----------|-----------|---------|----------|
| **Blockchain** | Substrate | 948fbd2 | Core runtime |
| **Consensus** | Aura + GRANDPA | - | Block production & finality |
| **EVM** | Frontier | polkadot-v1.1.0 | Ethereum compatibility |
| **SVM** | Solana RBPF | 0.8 | Solana program execution |
| **Language (Runtime)** | Rust | 2021 | Core blockchain |
| **Language (SDK)** | TypeScript | 5.0+ | Client libraries |
| **Database** | PostgreSQL | 15+ | Off-chain indexing |
| **Monitoring** | Prometheus + Grafana | latest | Observability |
| **RPC** | jsonrpsee | 0.22.5 | JSON-RPC 2.0 |
| **Async** | Tokio | 1.0+ | Async runtime |
| **Container** | Docker + Compose | latest | Deployment |
| **Orchestration** | Kubernetes | latest | Cluster mgmt |
| **LLM** | Ollama | latest | Local inference |

---

## Dependencies Lock & Management

- **Workspace Dependencies:** Centralized in `Cargo.toml` `[workspace.dependencies]` section
- **Patches Applied:** Substrate rev `948fbd2` patched across workspace
- **Overrides:** npm overrides in `package.json` for security (elliptic, axios, ws)
- **Constraint Strategy:** WASM32-compatible versions enforced (getrandom 0.2, specific base64ct version)

---

## Future Extensibility Points

Based on current architecture:

1. **Additional VMs:** Infrastructure ready for additional virtual machines beyond EVM/SVM
2. **Custom Precompiles:** EVM precompile system extensible via Frontier
3. **Pallet Composability:** New pallets can be added to runtime via workspace members
4. **Bridge Adapters:** Cross-chain bridge support expandable via `x3-bridge-adapters`
5. **Consensus Mechanisms:** GRANDPA + Aura can be complemented with alternative protocols
6. **Storage Layer:** PostgreSQL indexing can be extended with specialized indices

