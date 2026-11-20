# Core verification and release gates

Use [verification](docs/verification/README.md) and [current release status](docs/release/STATUS.md) for the scope of each gate.

Local source/development gates:

```bash
python3 -m unittest discover -s scripts/tests -p 'test_check_docs.py'
python3 scripts/check-docs.py
python3 scripts/verify-core-snapshot.py
cargo metadata --locked --no-deps --format-version 1
cargo test --locked -p x3bc-core
cargo test --locked -p x3-compiler --test schema_validator
cargo check --locked -p x3-chain-node
```

Restored proof, snapshot/genesis and coordinator/simulator tools have their own focused tests and prerequisites. Their source presence is not a completed release gate. The CI workflow must identify which checks it actually executes.

A release additionally requires intended-feature builds, actual isolated-network finality/restart/recovery, applicable atomic success/failure tests, security review and source-bound artifacts. None is replaced by an old readiness score, a simulation, a documentation link check or a passing compiler check.

No command above deploys or approves a mainnet transaction. Broader XXXSTAR ecosystem readiness tools can reference applications deliberately absent from this repository; they must report those prerequisites as unavailable rather than silently pass a reduced scope.
