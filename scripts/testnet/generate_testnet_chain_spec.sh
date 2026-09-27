#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# generate_testnet_chain_spec.sh — build a loadable testnet chain spec.
#
# Both RC gates (`mainnet_rc_gate.sh`, `testnet_rc_gate.sh`) have hard-coded this
# path since they were written, and nothing was ever created at it. With
# `set -euo pipefail`, a call to a missing script exits 127, so the mainnet gate
# died before reaching a single real check and the testnet gate — which ran every
# check as `... || true` — never noticed.
#
# The generator itself is `build-x3-testnet-spec.py`: it derives each validator's
# keys with the node's own CLI, refuses the published development seeds, writes a
# *plain* spec (the form the node loads for a Live chain), and feeds the artifact
# back through `build-spec --chain <file>` to prove it loads. This wrapper only
# resolves it and passes arguments through, so there stays exactly one
# implementation of genesis generation.
#
# Usage: scripts/testnet/generate_testnet_chain_spec.sh [validator-count] [--raw]
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GENERATOR="$ROOT_DIR/scripts/testnet/build-x3-testnet-spec.py"

if [ ! -f "$GENERATOR" ]; then
  echo "FAILED: the testnet chain spec generator is not at $GENERATOR" >&2
  echo "  this wrapper exists so a missing generator fails with a name, not with 127" >&2
  exit 1
fi

exec python3 "$GENERATOR" "$@"
