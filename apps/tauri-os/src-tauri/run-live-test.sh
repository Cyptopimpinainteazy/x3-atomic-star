#!/usr/bin/env bash
# Boot a real `x3-chain-node --dev` and run the console's live test against it.
#
# This is the layer `scripts/local-ci.sh` runs in the live set: the unit +
# integration suite proves the console refuses to invent answers, and this
# proves it reads a real chain. It is the only place the console's
# `system_health` / `chain_getFinalizedHead` / `system_name` reads are exercised
# against jsonrpsee rather than against a canned answer.
#
# The live test lives behind the `live-node` feature, so a plain `cargo test`
# does not compile it at all: there is nothing to forget to un-skip, and no skip
# attribute for the test-cheat ratchet to count.
#
# Every port is pinned to a free one: the node's defaults (9944 RPC, 30333 p2p,
# 9615 Prometheus) collide with anything else on the box.
#
# Usage: bash apps/tauri-os/src-tauri/run-live-test.sh
# Env:   X3_NODE_BIN       the node binary to boot (built when absent)
#        X3_OS_LIVE_RPC_PORT / X3_OS_LIVE_P2P_PORT / X3_OS_LIVE_PROMETHEUS_PORT
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR_ROOT:-$ROOT/target}"
RPC_PORT="${X3_OS_LIVE_RPC_PORT:-$(( 21000 + RANDOM % 8000 ))}"
P2P_PORT="${X3_OS_LIVE_P2P_PORT:-$(( 31000 + RANDOM % 8000 ))}"
PROMETHEUS_PORT="${X3_OS_LIVE_PROMETHEUS_PORT:-$(( 41000 + RANDOM % 8000 ))}"
RPC_URL="http://127.0.0.1:$RPC_PORT"
NODE_LOG="$(mktemp)"
NODE_PID=""

cleanup() {
  if [ -n "$NODE_PID" ] && kill -0 "$NODE_PID" 2>/dev/null; then
    kill "$NODE_PID" 2>/dev/null || true
    for _ in $(seq 1 20); do
      kill -0 "$NODE_PID" 2>/dev/null || break
      sleep 0.5
    done
    kill -9 "$NODE_PID" 2>/dev/null || true
  fi
  rm -f "$NODE_LOG"
}
trap cleanup EXIT

info() { printf '[tauri-os-live] %s\n' "$*"; }
fail() { printf '[tauri-os-live] FAIL: %s\n' "$*" >&2; tail -25 "$NODE_LOG" >&2 || true; exit 1; }

rpc() {
  curl -s -m 5 -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"method\":\"$1\",\"params\":${2:-[]},\"id\":1}" \
    "$RPC_URL"
}

# ── the binary ───────────────────────────────────────────────────────────────
NODE_BIN="${X3_NODE_BIN:-}"
if [ -z "$NODE_BIN" ]; then
  for candidate in "$TARGET_DIR/release/x3-chain-node" "$ROOT/target/release/x3-chain-node"; do
    [ -x "$candidate" ] && NODE_BIN="$candidate" && break
  done
fi
if [ -z "$NODE_BIN" ]; then
  info "no node binary found; building it (cargo build --release -p x3-chain-node)"
  ( cd "$ROOT" && cargo build --release -p x3-chain-node ) || fail "could not build the node"
  NODE_BIN="$ROOT/target/release/x3-chain-node"
fi
[ -x "$NODE_BIN" ] || fail "node binary is not executable: $NODE_BIN"
info "node: $NODE_BIN"

# ── boot ─────────────────────────────────────────────────────────────────────
info "booting a dev chain on $RPC_URL (p2p $P2P_PORT, prometheus $PROMETHEUS_PORT)"
"$NODE_BIN" --dev --tmp \
  --rpc-port "$RPC_PORT" --port "$P2P_PORT" --prometheus-port "$PROMETHEUS_PORT" \
  >"$NODE_LOG" 2>&1 &
NODE_PID=$!

info "waiting for the RPC endpoint"
ready=0
for _ in $(seq 1 90); do
  if rpc chain_getHeader | grep -q '"number"'; then ready=1; break; fi
  kill -0 "$NODE_PID" 2>/dev/null || fail "the node exited before it answered RPC"
  sleep 1
done
[ "$ready" = 1 ] || fail "the node did not answer chain_getHeader within 90s"

info "waiting for finality past genesis"
finalized=0
for _ in $(seq 1 60); do
  hash="$(rpc chain_getFinalizedHead | sed -n 's/.*"result":"\(0x[0-9a-f]*\)".*/\1/p' | head -1)"
  if [ -n "$hash" ]; then
    number_hex="$(rpc chain_getHeader "[\"$hash\"]" | sed -n 's/.*"number":"\(0x[0-9a-f]*\)".*/\1/p' | head -1)"
    if [ -n "$number_hex" ]; then
      finalized=$((number_hex))
      [ "$finalized" -ge 1 ] && break
    fi
  fi
  kill -0 "$NODE_PID" 2>/dev/null || fail "the node exited before finalizing"
  sleep 1
done
[ "$finalized" -ge 1 ] || fail "nothing was finalized within 60s"
info "finalized at $finalized"

# ── the console's own reads, against jsonrpsee ───────────────────────────────
info "running the live console test"
X3_OS_RPC_URL="$RPC_URL" bash "$SCRIPT_DIR/run-tests.sh" \
  --features live-node --test operator_console -- --nocapture live_operator_console_reads_a_running_node

info "PASS — the console read a live chain (finalized $finalized) at $RPC_URL"
