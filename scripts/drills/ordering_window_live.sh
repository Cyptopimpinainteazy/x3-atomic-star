#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# scripts/drills/ordering_window_live.sh
#
# The commit-reveal ordering window, on a live three-validator chain.
#
# Why this exists: `X3-MEV-006` and `X3-MEV-008` carried "no live-node run" as their remaining
# blocker for two rounds, and the reason was not scheduling. `open_ordering_window` and
# `commit_ordering` were gated on private execution *and* the confidential-validator quorum, and
# this runtime sets `AttestationVerifier = RefuseAllAttestations` on purpose (there is no vendor
# trust root), so the quorum can never be met and the window could not be opened on a live chain at
# all. Since `c917917d6` the lane has its own switch, and this gate turns it on the way an operator
# would — a council motion through the governance pallet — then drives the whole lifecycle:
#
#   council motion  -> privateExecution.set_ordering_windows_enabled(true)
#   Alice           -> open_ordering_window(now+1, now+8)
#   Alice, Bob      -> commit_ordering(window, commitment_hash(sender, plaintext, nonce), bond)
#   Alice           -> reveal_ordering with the WRONG plaintext  (must be refused, by name)
#   Alice, Bob      -> reveal_ordering with the right plaintext
#   (two blocks past close, so the beacon block's hash exists)
#   Alice           -> settle_ordering_window(window)
#   driver          -> recompute the canonical order from the stored beacon and require equality
#
# The driver does not decide the verdict on its own: it prints the chain's settled order and the
# order it recomputed, and exits non-zero unless they are identical, so a settle that ordered by
# arrival instead of by key would fail here even though every call succeeded.
#
# Usage: bash scripts/drills/ordering_window_live.sh
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
NODE_BIN="${X3_NODE_BIN:-$ROOT_DIR/target/release/x3-chain-node}"
NODE_MODULES="${X3_ORDERING_NODE_MODULES:-$ROOT_DIR/packages/blockchain-connector/node_modules}"
DRIVER="$ROOT_DIR/scripts/drills/ordering_window_live_driver.cjs"
TIMEOUT="${X3_ORDERING_TIMEOUT:-300}"

pass() { printf '[PASS] %s\n' "$1"; }
fail() { printf '[FAIL] %s %s\n' "$1" "${2:-}"; OVERALL="FAIL"; }
info() { printf '[ordering] %s\n' "$*"; }

OVERALL="PASS"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/x3-ordering.XXXXXX")"
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

rpc() {  # rpc <port> <method> [params]
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

wait_for_rpc() {  # wait_for_rpc <name> <port>
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
    echo "ordering_window_live: FAIL"
    exit 1
fi

# One frozen copy of the node, so every validator of the set is the same binary and therefore the
# same genesis (two builds of one tree differ).
FROZEN_NODE="$WORK_DIR/x3-chain-node"
cp "$NODE_BIN" "$FROZEN_NODE"
chmod +x "$FROZEN_NODE"
info "node binary $(sha256sum "$FROZEN_NODE" | awk '{print $1}')"

# Free ports, probed rather than guessed: a second chain on the same ports answers the first
# chain's RPC and wedges instead of failing.
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

start_validator() {  # start_validator <name> <seed> <rpc> <p2p> <prom> [extra...]
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
    echo "ordering_window_live: FAIL"
    exit 1
fi

# The set has to be finalizing before a window is worth opening: every block of the window is a
# block the settle depends on.
DEADLINE=$(( $(date +%s) + TIMEOUT ))
FINALIZED=0
while [[ "$(date +%s)" -lt "$DEADLINE" ]]; do
    FINALIZED="$(finalized_number "$A_RPC" || echo 0)"
    PEERS="$(rpc "$A_RPC" system_health | jq -r '.result.peers // 0')"
    if [[ "$FINALIZED" -ge 2 && "$PEERS" -ge 2 ]]; then
        break
    fi
    sleep 1
done
if [[ "$FINALIZED" -ge 2 && "${PEERS:-0}" -ge 2 ]]; then
    pass "chain_finalizing"
    info "finalized #$FINALIZED with $PEERS peers"
else
    fail "chain_finalizing" "(finalized #$FINALIZED, peers ${PEERS:-0})"
    echo "ordering_window_live: FAIL"
    exit 1
fi

EVIDENCE_DIR="$ROOT_DIR/.ai/runlogs/ordering-window-live-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$EVIDENCE_DIR"
info "evidence: $EVIDENCE_DIR"

if env NODE_PATH="$NODE_MODULES" \
    X3_WS_URL="ws://127.0.0.1:$A_RPC" \
    X3_OUT_JSON="$EVIDENCE_DIR/ordering_window.json" \
    node "$DRIVER" >"$EVIDENCE_DIR/driver.log" 2>&1; then
    pass "ordering_window_lifecycle"
    tail -5 "$EVIDENCE_DIR/driver.log" | sed 's/^/[ordering] /'
else
    fail "ordering_window_lifecycle" "(see $EVIDENCE_DIR/driver.log)"
    tail -20 "$EVIDENCE_DIR/driver.log" | sed 's/^/[ordering] /'
fi

# The verdict is the driver's, and the driver's verdict is the comparison it printed.
if [[ "$OVERALL" == "PASS" ]]; then
    echo "ordering_window_live: PASS"
    exit 0
fi
echo "ordering_window_live: FAIL"
exit 1
