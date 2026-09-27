#!/usr/bin/env bash
# Boot (and stop) a three-validator `local3` network for a gate that needs a chain which actually
# produces **and finalizes** blocks.
#
# Why this exists: `scripts/start-x3-chain.sh` runs a single node with `--chain dev` and no session
# keys. It answers RPC and never advances — measured 2026-09-27, the rc6 sequence's cross-chain smoke
# started it, connected, and then failed with `Error: block height/finality did not advance` while
# the node log filled with `Failed to trigger bootstrap: No known peers`. A gate that needs finality
# has to boot a network whose genesis has the authorities it runs. `local3` is that spec: three
# validators, `--alice`/`--bob`/`--charlie` for session keys, the same shape the passing live gates
# (`scripts/local-network-smoke.sh`, `scripts/mainnet/boot_local3.sh`) use.
#
# Usage:
#   source scripts/mainnet/local3_lib.sh
#   start_local3                     # idempotent: reuses a node already finalizing on 9944
#   stop_local3                      # stops only what start_local3 started
#
# Requires `chain-specs/x3-local3-raw.json` (committed; regenerate with
# `REGENERATE_CHAIN_SPEC=1 bash scripts/mainnet/boot_local3.sh`).

LOCAL3_ROOT="${LOCAL3_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
LOCAL3_BINARY="${X3_NODE_BIN:-$LOCAL3_ROOT/target/release/x3-chain-node}"
# A *raw* spec embeds the runtime WASM at the moment it was generated, so a committed one pins the
# gate to whatever the runtime was then. Measured 2026-09-27: `chain-specs/x3-local3-raw.json` (Sep 25)
# handed the rc2 smoke spec_version 11 while the tree's runtime was 20 — a gate that reports on code it
# is not running. Unless a caller points `X3_LOCAL3_RAW_SPEC` at a spec deliberately, the spec is
# rebuilt from the current binary into the temp work directory.
LOCAL3_RAW_SPEC="${X3_LOCAL3_RAW_SPEC:-}"
LOCAL3_WORK_DIR="${X3_LOCAL3_WORK_DIR:-}"
LOCAL3_LOG_DIR="${X3_LOCAL3_LOG_DIR:-}"
LOCAL3_PIDS=()
LOCAL3_STARTED=0

local3_rpc() {  # local3_rpc <method> [params-json]
  curl -sS -m 3 -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":${2:-[]}}" \
    "http://127.0.0.1:9944"
}

local3_up() { local3_rpc system_health >/dev/null 2>&1; }

local3_finalized_height() {  # prints a decimal height, or nothing
  local hash number
  hash="$(local3_rpc chain_getFinalizedHead | sed -n 's/.*"result":"\([^"]*\)".*/\1/p')"
  [ -n "$hash" ] || return 1
  number="$(local3_rpc chain_getHeader "[\"$hash\"]" | sed -n 's/.*"number":"\([^"]*\)".*/\1/p')"
  [ -n "$number" ] || return 1
  printf '%d' "$(( number ))"
}

local3_node_key() { printf '%064x' "$1"; }  # deterministic libp2p key per validator

# `build-spec` prints a banner before the JSON; take everything from the first `{`, the same way
# `boot_local3.sh`'s `capture_build_spec_json` does.
local3_generate_raw_spec() {
  local out="$1"
  "$LOCAL3_BINARY" build-spec --chain local3 --disable-default-bootnode --raw \
    | awk 'BEGIN{emit=0} /^[[:space:]]*\{/ {emit=1} emit {print}' > "$out"
  [ -s "$out" ] || return 1
  python3 -c 'import json,sys; json.load(open(sys.argv[1]))' "$out" >/dev/null 2>&1
}

local3_peer_id() {  # the base58 peer id the node advertises, read from its own RPC
  local attempt=0
  while [ "$attempt" -lt 60 ]; do
    local id
    id="$(local3_rpc system_localPeerId | sed -n 's/.*"result":"\([^"]*\)".*/\1/p')"
    if [ -n "$id" ]; then
      printf '%s' "$id"
      return 0
    fi
    sleep 1
    attempt=$((attempt + 1))
  done
  return 1
}

local3_start_one() {  # <name> <key-seed> <rpc-port> <p2p-port> <prom-port> [extra node args...]
  local name="$1" seed="$2" rpc_port="$3" p2p_port="$4" prom_port="$5"
  shift 5
  "$LOCAL3_BINARY" \
    --chain "$LOCAL3_RAW_SPEC" \
    --base-path "$LOCAL3_WORK_DIR/$name" \
    --node-key "$(local3_node_key "$seed")" \
    --rpc-port "$rpc_port" \
    --port "$p2p_port" \
    --prometheus-port "$prom_port" \
    --rpc-cors all \
    --rpc-methods unsafe \
    --no-mdns \
    "$@" >"$LOCAL3_LOG_DIR/$name.log" 2>&1 &
  LOCAL3_PIDS+=("$!")
}

# Start the network and wait until a *finalized* height has advanced, which is the property the
# callers actually need. Returns non-zero (and stops what it started) if it never gets there.
start_local3() {
  local timeout="${1:-120}"

  if local3_up; then
    local height
    height="$(local3_finalized_height || true)"
    if [ -n "$height" ] && [ "$height" -gt 0 ]; then
      echo "[local3] reusing the node finalizing at height $height on 127.0.0.1:9944"
      return 0
    fi
    echo "[local3] a node answers on 9944 but has finalized nothing; refusing to reuse it" >&2
    return 1
  fi

  [ -x "$LOCAL3_BINARY" ] || { echo "[local3] missing node binary: $LOCAL3_BINARY" >&2; return 1; }

  if [ -z "$LOCAL3_WORK_DIR" ]; then
    LOCAL3_WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/x3-local3.XXXXXX")"
  fi
  if [ -z "$LOCAL3_LOG_DIR" ]; then
    LOCAL3_LOG_DIR="${LOCAL3_ROOT}/reports/rc2/local3"
  fi
  mkdir -p "$LOCAL3_WORK_DIR" "$LOCAL3_LOG_DIR"

  if [ -z "$LOCAL3_RAW_SPEC" ]; then
    LOCAL3_RAW_SPEC="$LOCAL3_WORK_DIR/local3-raw.json"
    echo "[local3] building a raw spec from the current binary (so the gate runs the tree's runtime)"
    if ! local3_generate_raw_spec "$LOCAL3_RAW_SPEC"; then
      echo "[local3] could not build a local3 raw spec from $LOCAL3_BINARY" >&2
      return 1
    fi
  fi
  [ -s "$LOCAL3_RAW_SPEC" ] || { echo "[local3] missing chain spec: $LOCAL3_RAW_SPEC" >&2; return 1; }

  echo "[local3] booting three validators (base $LOCAL3_WORK_DIR, logs $LOCAL3_LOG_DIR)"
  LOCAL3_STARTED=1
  local3_start_one alice 1 9944 30333 9615 --alice --validator

  # The bootnode multiaddr needs Alice's *peer id* — the base58 multihash she advertises — not the
  # hex seed of her libp2p key. Passing the seed produced
  # `multiaddr parsing error: Invalid base string` for Bob and Charlie, and Alice then sat alone with
  # `Failed to trigger bootstrap: No known peers`, so nothing finalized (measured 2026-09-27).
  local alice_peer_id
  if ! alice_peer_id="$(local3_peer_id)"; then
    echo "[local3] Alice never answered system_localPeerId; see $LOCAL3_LOG_DIR/alice.log" >&2
    stop_local3
    return 1
  fi
  echo "[local3] alice peer id $alice_peer_id"
  local bootnode="/ip4/127.0.0.1/tcp/30333/p2p/$alice_peer_id"
  local3_start_one bob 2 9945 30334 9616 --bob --validator --bootnodes "$bootnode"
  local3_start_one charlie 3 9946 30335 9617 --charlie --validator --bootnodes "$bootnode"

  local waited=0 height=""
  while [ "$waited" -lt "$timeout" ]; do
    height="$(local3_finalized_height || true)"
    if [ -n "$height" ] && [ "$height" -gt 0 ]; then
      echo "[local3] finalized height $height after ${waited}s"
      return 0
    fi
    sleep 1
    waited=$((waited + 1))
  done

  echo "[local3] no finalized height after ${timeout}s; see $LOCAL3_LOG_DIR/*.log" >&2
  stop_local3
  return 1
}

stop_local3() {
  if [ "$LOCAL3_STARTED" != "1" ]; then
    return 0
  fi
  local pid
  for pid in "${LOCAL3_PIDS[@]:-}"; do
    [ -n "$pid" ] || continue
    kill "$pid" >/dev/null 2>&1 || true
  done
  wait "${LOCAL3_PIDS[@]:-}" 2>/dev/null || true
  LOCAL3_PIDS=()
  LOCAL3_STARTED=0
  echo "[local3] stopped"
}
