#!/usr/bin/env bash
# Local source/build/unit gates only; never launches nodes or signs transactions.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
python3 -m unittest discover -s scripts/tests -p test_check_docs.py
python3 scripts/check-docs.py
python3 scripts/verify-core-snapshot.py
cargo metadata --locked --no-deps --format-version 1 > /dev/null
cargo test --locked -p x3bc-core -p x3-state-snapshot -p x3-genesis-builder
cargo test --locked -p x3-compiler --test schema_validator
cargo check --locked -p x3-chain-node
