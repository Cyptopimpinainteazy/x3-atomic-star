# X3 Atomic Star core

Start with [README.md](README.md). This candidate keeps XXXSTAR's blockchain implementation together and narrows the source distribution.

- `node/`: node service, RPC integration and chain loading.
- `runtime/` and `pallets/`: runtime and required on-chain modules.
- `crates/`: required execution, consensus, verification and shared types.
- `patches/`: pinned local dependency fixes retained from XXXSTAR.
- `x3-lang/`: only nested compiler/VM dependencies and required fixtures.
- `chain-specs/` and `deployment/chain-specs/`: upstream network specification files; inclusion does not authorize a deployment.
- `audit-artifacts/migration/`: source identity and extraction verification.

XXXSTAR stays active. Applications, AI services and other satellite repositories are not replaced by this candidate.
