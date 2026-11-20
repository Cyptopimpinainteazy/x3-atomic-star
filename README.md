# X3 Atomic Star

A focused blockchain development repository: node, runtime, on-chain modules, shared crates, proof/failure-testing tools and operator utilities. XXXSTAR remains active as the companion project workspace.

Start with the [documentation index](docs/README.md), [architecture](docs/architecture/README.md) and [current release status](docs/release/STATUS.md). The code layout keeps existing Substrate/Cargo paths stable; apps and general AI services stay outside this core.

## Build and check

Use the pinned [Rust toolchain](rust-toolchain.toml) and [native prerequisites](docs/development/README.md). From the repository root:

```bash
bash scripts/check-core.sh
```

For executable builds and isolated testnets, follow [local operations](docs/operations/README.md). Checks and their limits are explained in [verification](docs/verification/README.md). This is a development candidate, not a verified production network or a deployment approval.

## Source and evidence

The source import is pinned to `xxxstar@69234c1897d9e80424cb0a652d48cd4ee6511deb`. [Migration provenance](audit-artifacts/migration/provenance.json) and [recorded extraction checks](audit-artifacts/migration/verification.json) preserve that original observation. Follow-up changes have their own evidence.

Earlier status documents are kept in [the historical archive](docs/archive/README.md), separate from current instructions. [Security policy](SECURITY.md) and [invariants](INVARIANTS.md) remain relevant. No cleanup changes XXXSTAR, running nodes, contracts, keys or funds.
