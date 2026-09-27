#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# scripts/drills/wallet_recovery_live.sh
#
# The wallet pallet's biometric registration and social recovery, on a live three-validator chain.
#
# `X3-ECO-001` and `X3-ECO-004` were P0/core rows whose entire proof was the pallet's own mock —
# and, before `d85f79c85`, the recovery test there had to seed a `GuardianAccount` straight into
# storage because no extrinsic created one, and `register_biometric` validated nothing. Both paths
# are real now; neither had run on a chain. This drives them through the real extrinsics:
#
#   Alice      -> register_biometric(1, template, pin)      (and a bad type, which must be refused)
#   Alice      -> register_recovery_guardians([Bob, Charlie], threshold 2, delay 3)
#   Bob        -> initiate_recovery(Alice, new_owner)
#   Charlie    -> approve_recovery(Alice)
#   Bob        -> approve_recovery(Alice)                    (threshold met)
#   Bob        -> finalize_recovery(Alice)                    (refused: the delay has not elapsed)
#   Charlie    -> cancel_recovery(Alice)                      (refused: not the recovery owner)
#   (three blocks later)
#   Bob        -> finalize_recovery(Alice)                    (accepted)
#   driver     -> the *stored* recovery owner must now be the new owner
#
# The verdict is the last assertion, not the event: an event alone was the bug this flow replaced.
#
# Usage: bash scripts/drills/wallet_recovery_live.sh
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
NODE_BIN="${X3_NODE_BIN:-$ROOT_DIR/target/release/x3-chain-node}"
NODE_MODULES="${X3_ORDERING_NODE_MODULES:-$ROOT_DIR/packages/blockchain-connector/node_modules}"
DRIVER="$ROOT_DIR/scripts/drills/wallet_recovery_live_driver.cjs"
TIMEOUT="${X3_WALLET_TIMEOUT:-300}"

pass() { printf '[PASS] %s\n' "$1"; }
fail() { printf '[FAIL] %s %s\n' "$1" "${2:-}"; OVERALL="FAIL"; }
info() { printf '[wallet] %s\n' "$*"; }

OVERALL="PASS"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/x3-wallet.XXXXXX")"
NODE_PIDS=()

cleanup() {
    for pid in "${NODE_PIDS[@]:-}"; do
        kill "$pid" >/dev/null 2>&1 || true
    done
    sleep 1
    for pid in "${NODE_PIDS[@]:-}"; do
        kill -9 "$pid" >/dev/null 2>&1 || true
    done
    rm -rf "$WORK_DIR" >/dev/null 2>&1 || true
}
trap cleanup EXIT

rpc() {
    curl -s -m 10 -H 'Content-Type: application/json' \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"$2\",\"params\":${3:-[]},\"id\":1}" \
        "http://127.0.0.1:$1"
}

finalized_number() {
    local hash
    hash="$(rpc "$1" chain_getFinalizedHead | jq -r '.result // empty')"
    [ -n "$hash" ] || return 1
    rpc "$1" chain_getHeader "[\"$hash\"]" | jq -r '.result.number // empty' | xargs -r printf '%d\n'
}

wait_for_rpc() {
    local deadline=$(( $(date +%s) + TIMEOUT ))
    while [[ "$(date +%s)" -lt "$deadline" ]]; do
        if rpc "$2" chain_getHeader | grep -q '"number"'; then
            info "$1 answered RPC on :$2"
            return 0
        fi
        sleep 1
    done
    return 1
}

if [[ ! -x "$NODE_BIN" ]]; then
    fail "node_binary" "(no executable at $NODE_BIN — build it first)"
elif [[ ! -d "$NODE_MODULES/@polkadot/api" ]]; then
    fail "client_library" "(no @polkadot/api under $NODE_MODULES)"
elif [[ ! -f "$DRIVER" ]]; then
    fail "driver_present" "($DRIVER missing)"
else
    pass "prerequisites"
fi
if [[ "$OVERALL" == "FAIL" ]]; then
    echo "wallet_recovery_live: FAIL"
    exit 1
fi

FROZEN_NODE="$WORK_DIR/x3-chain-node"
cp "$NODE_BIN" "$FROZEN_NODE"
chmod +x "$FROZEN_NODE"
info "node binary $(sha256sum "$FROZEN_NODE" | awk '{print $1}')"

read -r A_RPC A_P2P A_PROM B_RPC B_P2P B_PROM C_RPC C_P2P C_PROM < <(
    python3 - <<'PY'
import socket

def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port

ports = set()
while len(ports) < 9:
    ports.add(free_port())
print(" ".join(str(port) for port in sorted(ports)))
PY
)

BASE_DIR="$WORK_DIR/chain"
LOG_DIR="$WORK_DIR/logs"
mkdir -p "$BASE_DIR" "$LOG_DIR"

start_validator() {
    local name="$1" seed="$2" rpc_port="$3" p2p_port="$4" prom_port="$5"; shift 5
    "$FROZEN_NODE" --chain local3 --base-path "$BASE_DIR/$name" \
        --rpc-port "$rpc_port" --port "$p2p_port" --prometheus-port "$prom_port" \
        --rpc-methods unsafe --no-mdns \
        --node-key "$(printf '%064x' "$seed")" "$@" >"$LOG_DIR/$name.log" 2>&1 &
    local pid=$!
    NODE_PIDS+=("$pid")
    disown "$pid" 2>/dev/null || true
}

info "starting three validators (rpc $A_RPC/$B_RPC/$C_RPC)"
start_validator alice 1 "$A_RPC" "$A_P2P" "$A_PROM" --alice
wait_for_rpc alice "$A_RPC" || fail "alice_rpc" "(see $LOG_DIR/alice.log)"
ALICE_PEER_ID="$(rpc "$A_RPC" system_localPeerId | jq -r '.result // empty')"
ALICE_BOOTNODE="/ip4/127.0.0.1/tcp/$A_P2P/p2p/$ALICE_PEER_ID"

start_validator bob 2 "$B_RPC" "$B_P2P" "$B_PROM" --bob --bootnodes "$ALICE_BOOTNODE"
wait_for_rpc bob "$B_RPC" || fail "bob_rpc" "(see $LOG_DIR/bob.log)"
BOB_PEER_ID="$(rpc "$B_RPC" system_localPeerId | jq -r '.result // empty')"
BOB_BOOTNODE="/ip4/127.0.0.1/tcp/$B_P2P/p2p/$BOB_PEER_ID"

start_validator charlie 3 "$C_RPC" "$C_P2P" "$C_PROM" --charlie \
    --bootnodes "$ALICE_BOOTNODE" "$BOB_BOOTNODE"
wait_for_rpc charlie "$C_RPC" || fail "charlie_rpc" "(see $LOG_DIR/charlie.log)"

if [[ "$OVERALL" == "FAIL" ]]; then
    echo "wallet_recovery_live: FAIL"
    exit 1
fi

DEADLINE=$(( $(date +%s) + TIMEOUT ))
FINALIZED=0
PEERS=0
while [[ "$(date +%s)" -lt "$DEADLINE" ]]; do
    FINALIZED="$(finalized_number "$A_RPC" || echo 0)"
    PEERS="$(rpc "$A_RPC" system_health | jq -r '.result.peers // 0')"
    if [[ "$FINALIZED" -ge 2 && "$PEERS" -ge 2 ]]; then
        break
    fi
    sleep 1
done
if [[ "$FINALIZED" -ge 2 && "$PEERS" -ge 2 ]]; then
    pass "chain_finalizing"
    info "finalized #$FINALIZED with $PEERS peers"
else
    fail "chain_finalizing" "(finalized #$FINALIZED, peers $PEERS)"
    echo "wallet_recovery_live: FAIL"
    exit 1
fi

EVIDENCE_DIR="$ROOT_DIR/.ai/runlogs/wallet-recovery-live-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$EVIDENCE_DIR"
info "evidence: $EVIDENCE_DIR"

if env NODE_PATH="$NODE_MODULES" \
    X3_WS_URL="ws://127.0.0.1:$A_RPC" \
    X3_OUT_JSON="$EVIDENCE_DIR/wallet_recovery.json" \
    node "$DRIVER" >"$EVIDENCE_DIR/driver.log" 2>&1; then
    pass "wallet_recovery_lifecycle"
    tail -6 "$EVIDENCE_DIR/driver.log" | sed 's/^/[wallet] /'
else
    fail "wallet_recovery_lifecycle" "(see $EVIDENCE_DIR/driver.log)"
    tail -20 "$EVIDENCE_DIR/driver.log" | sed 's/^/[wallet] /'
fi

if [[ "$OVERALL" == "PASS" ]]; then
    echo "wallet_recovery_live: PASS"
    exit 0
fi
echo "wallet_recovery_live: FAIL"
exit 1
