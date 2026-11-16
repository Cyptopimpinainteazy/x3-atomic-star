# X3 Atomic Star — blockchain core candidate

A local extraction of the X3 blockchain node, runtime, required pallets and shared Rust crates. XXXSTAR remains active and authoritative while this candidate is verified. Nothing here changes or decommissions it.

The source snapshot is `xxxstar@69234c1897d9e80424cb0a652d48cd4ee6511deb`. Extraction retains the node/runtime dependency closure, local patches, chain specifications and required language/compiler fixtures. Unrelated applications and services, local chain databases, bundled toolchains and historical build dumps remain outside this source tree.

## Scope

The root workspace retains 120 explicitly listed members, down from 189. Cargo can include additional local dependency packages automatically. Consensus, execution, custody, settlement and GPU fallback source is copied unchanged. Root workspace membership and lockfile pruning are packaging changes that still require build and execution verification.

This is a candidate, not a release or a verified running network. Original status and security limits remain in [LAUNCH_SCOPE.md](LAUNCH_SCOPE.md), [CURRENT_MAINNET_STATUS.md](CURRENT_MAINNET_STATUS.md) and [SECURITY.md](SECURITY.md).

## Verify

Use the pinned Rust toolchain and install the upstream node prerequisites, including protobuf, clang and LLVM.

```bash
python3 scripts/verify-core-snapshot.py
cargo metadata --locked --no-deps --format-version 1
cargo check --locked -p x3-chain-node
cargo test --locked -p x3bc-core
```

Build the actual node and start a disposable development chain only after the gates pass:

```bash
cargo build --locked --release -p x3-chain-node
bash scripts/run-dev-node.sh
```

The launcher requires the real node binary and never substitutes a mock RPC server. Development keys must never secure a funded or public production network.

## Provenance and ownership

[audit-artifacts/migration/provenance.json](audit-artifacts/migration/provenance.json) identifies the source and local-only migration status. The file manifest checks copied bytes; it is not a security audit or proof of runtime equivalence. XXXSTAR branch history and unmerged work are preserved in their original repository, not merged indiscriminately into this candidate.

No repository visibility, hosted default branch, deployed node, key, contract or payment is changed by this extraction.
