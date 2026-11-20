# Architecture and repository layout

Keep tightly coupled blockchain implementation together; organize its supporting tools by purpose without renaming crate paths.

| Location | Responsibility |
|---|---|
| `node/` | Node process, consensus wiring, networking and RPC integration |
| `runtime/` | Runtime construction, feature gates and inline runtime modules |
| `pallets/` | Required state transitions and on-chain policies |
| `crates/` | Shared execution, consensus, accounting, verification and operator modules |
| `patches/` | Pinned local dependency fixes |
| `x3-lang/` | Compiler/VM dependencies and fixtures required by this workspace |
| `proof-forge/` | Proof and verification tooling; evidence and policy inputs remain required |
| `proving/`, `crates/x3-chain-health-daemon/` | Retained experimental utilities, outside trusted release/health gates |
| `tools/launchops/` | Operator tooling |
| `scripts/` | Explicit local checks and isolated launch commands |
| `chain-specs/`, `deployment/chain-specs/` | Specification files and generated local testnet inputs |
| `docs/` | Maintained guides plus clearly separated historical material |
| `audit-artifacts/` | Dated execution evidence and source provenance |

The node/runtime dependency closure alone does not define a complete blockchain development repository. Snapshot/recovery tools, proof harnesses, fault simulators and release checks belong alongside it even when the node does not link them.

The companion private XXXSTAR checkout holds IDEs, desktop/front-end apps, general AI services, app-store projects and client SDK projects. XXXSTAR stays active. They are not silently imported through this cleanup. External contracts/adapters can remain companion workspaces with explicit integration tests.

The `pallets/fraud-proofs` stub is not the actual fraud-proof implementation; the runtime implementation lives under `runtime/src/fraud_proofs/`, which is retained. Do not copy duplicate stubs merely to make a directory list look complete.

Consensus correctness, CPU/GPU agreement, authority and atomic settlement semantics must survive any future extraction. Nothing in this organization enables a new runtime feature or authorizes deployment.
