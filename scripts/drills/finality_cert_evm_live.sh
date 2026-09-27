#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# scripts/drills/finality_cert_evm_live.sh
#
# The EVM finality-certificate producer, on a live anvil chain.
#
# `X3-XCHAIN-005` made `FinalityCertificate` a checked shape: it cannot invent depth for an anchor,
# but nothing bound its `block_hash` to the chain named in `chain`. This drill runs the producer
# added for that gap against real chain data:
#
#   1. deploy AtlasHTLC (this repository's contract) with forge — one real transaction;
#   2. let it reach the depth the config requires (12 confirmations) and build a certificate from
#      the node's receipt + block + tip, then settle on it;
#   3. rewind the chain with evm_snapshot/evm_revert so the anchor is one block deep again, and
#      present a certificate built from that *real* rewound fork to a *fresh process* that reloaded
#      the accepted tip from the store — it must be refused as CertificateRewindsAcceptedAnchor;
#   4. point the producer at chain id 5 while the node answers 31337 — the receipt must be refused
#      for the chain, not graded against the depth rule.
#
# The verdict is the last assertion in the driver, not the certificate's own say-so.
#
# Usage: bash scripts/drills/finality_cert_evm_live.sh
# Requires: foundry (anvil, forge, cast), node, python3. Run from anywhere.
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DRIVER="$ROOT_DIR/scripts/drills/finality_cert_evm_live_driver.cjs"
CONTRACTS_DIR="$ROOT_DIR/X3-contracts/evm"
CHAIN_ID="${X3_FINALITY_CERT_CHAIN_ID:-31337}"
TIMEOUT="${X3_FINALITY_CERT_TIMEOUT:-300}"

# Foundry is installed outside the default PATH on this host.
if ! command -v anvil >/dev/null 2>&1 && [[ -x "$HOME/.foundry/bin/anvil" ]]; then
    export PATH="$HOME/.foundry/bin:$PATH"
fi

pass() { printf '[PASS] %s\n' "$1"; }
fail() { printf '[FAIL] %s %s\n' "$1" "${2:-}"; OVERALL="FAIL"; }
info() { printf '[finality-cert] %s\n' "$*"; }

OVERALL="PASS"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/x3-finality-cert.XXXXXX")"
ANVIL_PID=""

cleanup() {
    if [[ -n "$ANVIL_PID" ]] && kill -0 "$ANVIL_PID" 2>/dev/null; then
        kill "$ANVIL_PID" >/dev/null 2>&1 || true
        wait "$ANVIL_PID" 2>/dev/null || true
    fi
    rm -rf "$WORK_DIR" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Anvil's deterministic dev account 0 (public test mnemonic; ephemeral chain only).
SENDER_KEY="ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"

for tool in anvil forge cast node python3; do
    command -v "$tool" >/dev/null 2>&1 || fail "prerequisite_$tool" "($tool not on PATH)"
done
[[ -f "$DRIVER" ]] || fail "driver_present" "($DRIVER missing)"
[[ -f "$CONTRACTS_DIR/contracts/AtlasHTLC.sol" ]] || fail "contract_source" "(AtlasHTLC.sol missing)"
if [[ "$OVERALL" == "FAIL" ]]; then
    echo "finality_cert_evm_live: FAIL"
    exit 1
fi

info "building x3-finality-cert"
if (cd "$ROOT_DIR" && cargo build -p x3-atomic-swap --features std --bin x3-finality-cert) >"$WORK_DIR/build.log" 2>&1; then
    pass "build_binary"
else
    fail "build_binary" "(see $WORK_DIR/build.log)"
    tail -20 "$WORK_DIR/build.log" | sed 's/^/[finality-cert] /'
    echo "finality_cert_evm_live: FAIL"
    exit 1
fi
BIN="${CARGO_TARGET_DIR:-$ROOT_DIR/target}/debug/x3-finality-cert"
if [[ -x "$BIN" ]]; then
    pass "binary_present"
    info "binary $(sha256sum "$BIN" | awk '{print $1}')"
else
    fail "binary_present" "(no executable at $BIN)"
    echo "finality_cert_evm_live: FAIL"
    exit 1
fi

PORT="$(python3 - <<'PY'
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
PY
)"
RPC="http://127.0.0.1:$PORT"

info "starting anvil on $RPC (chain id $CHAIN_ID)"
anvil --port "$PORT" --chain-id "$CHAIN_ID" --silent >"$WORK_DIR/anvil.log" 2>&1 &
ANVIL_PID=$!
READY=0
DEADLINE=$(( $(date +%s) + TIMEOUT ))
while [[ "$(date +%s)" -lt "$DEADLINE" ]]; do
    if cast chain-id --rpc-url "$RPC" >/dev/null 2>&1; then
        READY=1
        break
    fi
    sleep 0.5
done
if [[ "$READY" == "1" ]]; then
    pass "anvil_ready"
else
    fail "anvil_ready" "(see $WORK_DIR/anvil.log)"
    echo "finality_cert_evm_live: FAIL"
    exit 1
fi

info "deploying AtlasHTLC"
if DEPLOY_JSON="$( (cd "$CONTRACTS_DIR" && forge create contracts/AtlasHTLC.sol:AtlasHTLC \
        --rpc-url "$RPC" --private-key "$SENDER_KEY" --broadcast --json) 2>"$WORK_DIR/deploy.err" )"; then
    DEPLOYED_TO="$(DEPLOY_JSON="$DEPLOY_JSON" python3 -c 'import json,os; print(json.loads(os.environ["DEPLOY_JSON"])["deployedTo"])')"
    ANCHOR_TX="$(DEPLOY_JSON="$DEPLOY_JSON" python3 -c 'import json,os; print(json.loads(os.environ["DEPLOY_JSON"])["transactionHash"])')"
    pass "deploy_contract"
    info "AtlasHTLC at $DEPLOYED_TO (anchor tx $ANCHOR_TX)"
else
    fail "deploy_contract" "(see $WORK_DIR/deploy.err)"
    tail -20 "$WORK_DIR/deploy.err" | sed 's/^/[finality-cert] /'
    echo "finality_cert_evm_live: FAIL"
    exit 1
fi

EVIDENCE_DIR="$ROOT_DIR/.ai/runlogs/finality-cert-live-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$EVIDENCE_DIR"
info "evidence: $EVIDENCE_DIR"

if env \
    X3_RPC_URL="$RPC" \
    X3_CHAIN_ID="$CHAIN_ID" \
    X3_ANCHOR_TX="$ANCHOR_TX" \
    X3_CONTRACT="$DEPLOYED_TO" \
    X3_FINALITY_CERT_BIN="$BIN" \
    X3_WORK_DIR="$WORK_DIR" \
    X3_EVIDENCE_DIR="$EVIDENCE_DIR" \
    node "$DRIVER" >"$EVIDENCE_DIR/driver.log" 2>&1; then
    pass "drill"
    tail -12 "$EVIDENCE_DIR/driver.log" | sed 's/^/[finality-cert] /'
else
    fail "drill" "(see $EVIDENCE_DIR/driver.log)"
    tail -25 "$EVIDENCE_DIR/driver.log" | sed 's/^/[finality-cert] /'
fi

if [[ "$OVERALL" == "PASS" ]]; then
    echo "finality_cert_evm_live: PASS"
    exit 0
fi
echo "finality_cert_evm_live: FAIL"
exit 1
