# Technology Stack

**Project:** X3 Chain
**Researched:** 2026-03-15

## Recommended Stack

### Core Framework
| Technology | Version / Pin | Purpose | Why |
|------------|--------------|---------|-----|
| Rust | 2024+ (nightly for some tooling) | Primary language for node, runtime, pallets, and tooling | High performance, strong type safety, existing Substrate ecosystem |
| Substrate (paritytech/substrate@948fbd2) | 948fbd2 (pinned) | Blockchain runtime framework, consensus, networking, storage | Proven L1 stack; enables modular pallets and deterministic execution |
| Frontier (pallet-evm) | Upstream Frontier integrated in runtime | EVM compatibility for Ethereum tooling and contracts | Enables fast onboarding for Solidity ecosystem |
| solana-rbpf | Latest release (in repo via crates) | Solana-style BPF VM execution (SVM) | Adds Solana-program compatibility in same runtime |

### Runtime / Node
| Technology | Purpose | Why |
|------------|---------|-----|
| Substrate Runtime (`runtime/`) | Defines on-chain logic, pallets, and VM adapters | Central on-chain execution model |
| Node (`node/`) | Networking, RPC, consensus client, runtime host | Hosts the chain; provides JSON-RPC surface |
| `pallets/x3-kernel` | Atomic cross-VM orchestration, account locking, comits | Differentiator: atomic multi-VM transactions |

### Infrastructure & Tooling
| Technology | Purpose | Why |
|------------|---------|-----|
| GitHub Actions | CI gating (x3-audit) | Enforces completion checklist + build/test gating |
| `cargo-deny` | Dependency security checks | Blocks known vulnerabilities and abandoned crates |
| `cargo-audit` (likely) | Supply chain auditing | Standard for Rust security |
| `cargo fmt/clippy` | Formatting and linting | Maintain code quality and Rust idioms |
| `scripts/x3_audit.sh` | Self-auditing runner | Enforces repo structure, build, tests, unsafe usage |

### Developer SDKs & Tooling
| Technology | Purpose | When to Use |
|------------|---------|-------------|
| TypeScript SDK (`packages/ts-sdk`) | Build DApps, wallets, and frontends against X3 runtime | Use for web apps, CLI tooling, and SDK distribution |
| Python SDK | Scripting, automation, tooling for node interaction | Useful for integration tests and scripting |
| `x3-wallet`, `x3-cli` | Developer utilities for accounts + transactions | Onboarding and debugging |

### Supporting Libraries
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `serde` | 1.0.x | (De)serialization for JSON/RPC and SCALE | Everywhere data structs are encoded/decoded |
| `tokio` | 1.x | Async runtime in node and tooling | Required for jsonrpsee / async RPC handling |
| `tracing` | 0.1.x | Structured logging | For observability in node and tools |

## Alternatives Considered
| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Base chain framework | Substrate | Cosmos SDK / Tendermint | Substrate already provides WASM runtime + pallets + EVM support; switching would require rewriting core architecture |
| EVM support | Frontier | OpenEthereum / EVMBin | Frontier is designed to plug into Substrate runtime and has existing pallet support |
| SVM support | solana-rbpf | Solana runtime / Anchor | The goal is compatibility with Solana BPF programs without full Solana runtime; solana-rbpf is lightweight and embeddable |

## Sources
- `docs/ARCHITECTURE.md` (primary architecture source)
- `Cargo.toml` (workspace member list & dependencies)
- `docs/X3_INDEX.md` (audit and enforcement docs)
- `docs/X3_SYSTEMS.md` (operational enforcement model)
