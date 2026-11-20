# Verification and evidence

Use each check for the claim it actually establishes.

| Check | Establishes | Does not establish |
|---|---|---|
| `python3 scripts/check-docs.py` | Maintained local documentation links resolve | Historical claims are true |
| `python3 scripts/verify-core-snapshot.py` | Listed imported bytes and expected workspace membership agree | Independent audit, runtime equivalence or full Git-history integrity |
| `cargo metadata --locked` | Manifests and pinned dependencies resolve | Compilation or execution |
| `cargo check --locked -p x3-chain-node` | Node type-check and embedded runtime compilation | Node startup, finality, recovery or settlement |
| `cargo test --locked -p x3bc-core` | Canonical bytecode unit tests | Full chain integration |
| `cargo test --locked -p x3-compiler --test schema_validator` | Compiler schema tests | Every language or runtime path |

Supporting proof, genesis, snapshot, coordinator and simulation modules are retained for their own tests. Nested workspaces use their own manifest and lockfile; resolve their complete dependencies before claiming they pass. Never turn missing tooling or an unavailable RPC into a successful result.

The initial extraction evidence is in [verification.json](../../audit-artifacts/migration/verification.json). It applies to the original candidate snapshot, not automatically to later changes. New organization/support-tool evidence is recorded separately. The latest [release status](../release/STATUS.md) names the remaining unverified integration work.

An allowed control, duplicate/replay, wrong secret, refund/timeout and restart/failure cases are relevant to atomic paths. A simulator is useful failure evidence but does not replace actual-chain execution. Historical tests and README counts are not fresh verification.

## Supporting-tool scope

- `proof-forge/` provides the `x3-proof` CLI. Its feature matrix under `proof/features/` describes the broader original ecosystem; evidence that is absent remains unverified.
- `proving/` is a legacy RPC capability probe. Its step labels do not establish real bundle execution, rollback or settlement. See [experimental utilities](EXPERIMENTAL.md).
- `crates/x3-state-snapshot/` and `crates/x3-genesis-builder/` supply local snapshot/hash/regenesis verification. Live chain-root matching and restore require their separate integration procedures.
- `crates/cross-vm-coordinator/`, `crates/x3-sim/` and `crates/x3-mc/` retain separate Cargo workspaces and lockfiles. Their default tests differ from opt-in distributed/Valkey and real-chain tests.
- Launch/operator utilities remain under `crates/` and `tools/launchops/`; their external artifact, telemetry and policy inputs must be present before an operational claim. The legacy health daemon is an experimental EVM-monitoring prototype, not a trusted X3 health/quorum gate.
- `formal-proofs/` retains Coq, K and TLA+ specification sources. Copying those files does not execute their respective proof/model-checking tools.

`FEATURE_REGISTRY.toml` and `TESTNET_FEATURE_FLAGS.toml` are preserved compatibility inputs for the legacy full-ecosystem readiness tools. Their scores and flags do not certify this core. In particular, `x3-readiness testnet-report` can derive a GO decision from stored declarations; that output is advisory and is not an executed release gate. Absent companion applications must not be treated as tested here.
