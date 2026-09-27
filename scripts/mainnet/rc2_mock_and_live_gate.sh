#!/usr/bin/env bash
set -euo pipefail

# Derived, not hardcoded. This said `/home/lojak/Desktop/X3_ATOMIC_STAR`, which does not exist on
# this box — and because the script `mkdir -p`s into `$ROOT` before `cd`, the failure was silent in
# the worst way: it *created* that empty directory, cd'd into it, and then failed with
# `manifest path tests/e2e/Cargo.toml does not exist`, which reads like a missing crate rather than
# a wrong root. The stray `/home/lojak/Desktop/X3_ATOMIC_STAR/reports/rc2` was this script.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RPC_HOST="127.0.0.1"
RPC_PORT="9944"
RPC_URL="http://${RPC_HOST}:${RPC_PORT}"
LIVE_LOG="${ROOT}/reports/rc2/live_node_for_e2e.log"

mkdir -p "${ROOT}/reports/rc2"

# The live lane needs a chain that produces *and finalizes* blocks. `scripts/start-x3-chain.sh` starts
# one `--chain dev` node with no session keys, which answers RPC and never advances, so this used to
# `wait_for_node` successfully and then have the live suite fail on a static chain. `local3_lib.sh`
# boots the three-validator spec the passing live gates use and waits for a finalized height.
X3_LOCAL3_LOG_DIR="${ROOT}/reports/rc2/local3"
source "${ROOT}/scripts/mainnet/local3_lib.sh"
trap stop_local3 EXIT

cd "${ROOT}"

echo "[rc2_mock_and_live_gate] Running mock/internal suite"
cargo test --manifest-path tests/e2e/Cargo.toml --test mainnet_rc1 -- --nocapture

echo "[rc2_mock_and_live_gate] Ensuring live node and running strict live suite"
start_local3 150
# `live_internal_mainnet_e2e` declares `required-features = ["real-chain"]`; without the flag cargo
# refuses the target outright: "target `live_internal_mainnet_e2e` in package `e2e_tests` requires
# the features: `real-chain`". That is what this gate was reporting as a failure.
X3_E2E_REQUIRE_NODE=1 cargo test --manifest-path tests/e2e/Cargo.toml --features real-chain \
  --test live_internal_mainnet_e2e -- --nocapture

echo "[rc2_mock_and_live_gate] PASS: mock suite and live suite both passed"
