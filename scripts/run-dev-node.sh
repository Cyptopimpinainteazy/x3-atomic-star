#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="${X3_NODE_BIN:-$ROOT/target/release/x3-chain-node}"
if [[ ! -x "$BINARY" ]]; then
  echo "Real node executable missing: build with cargo build --locked --release -p x3-chain-node" >&2
  exit 1
fi
exec "$BINARY" --chain dev --tmp "$@"
