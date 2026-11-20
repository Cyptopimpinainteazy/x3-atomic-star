# Development

Use the toolchain pinned in [rust-toolchain.toml](../../rust-toolchain.toml). Native prerequisites include protobuf, clang/LLVM, libclang, OpenSSL development headers and pkg-config. Cargo may download pinned upstream dependencies during setup.

From the repository root, `bash scripts/check-core.sh` runs the maintained local gates without launching nodes or signing transactions. Individual commands are:

```bash
python3 scripts/check-docs.py
python3 scripts/verify-core-snapshot.py
cargo metadata --locked --no-deps --format-version 1
cargo check --locked -p x3-chain-node
cargo test --locked -p x3bc-core
cargo test --locked -p x3-compiler --test schema_validator
```

The node check also exercises its embedded runtime build. It does not produce a release executable or prove a running chain. Scope Rust fmt/clippy checks to code you change; imported source has historical formatting and warning debt. Do not rewrite unrelated consensus code to tidy the tree.

Use [the verification guide](../verification/README.md) for supporting-tool tests. See [operations](../operations/README.md) before starting any node. No wallet, API credential or funded account is needed for the local source/documentation checks.
