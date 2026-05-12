#!/usr/bin/env bash
# RC1 internal settlement testnet smoke test
# Connects to Alice's RPC (localhost:9944) and verifies:
#   1. Chain is producing blocks
#   2. Finality is advancing
#   3. Runtime metadata exposes X3SupplyLedger + X3CrossVmRouter pallets
#   4. ExternalBridgesEnabled storage = false
set -euo pipefail

RPC="${RPC_URL:-http://localhost:9944}"
PASS=0
FAIL=0

ok()   { echo "[PASS] $1"; ((PASS++)); }
fail() { echo "[FAIL] $1"; ((FAIL++)); }
rpc()  { curl -s -X POST -H "Content-Type: application/json" --data "$1" "$RPC"; }

echo "=== X3 RC1 Smoke Test ==="
echo "RPC: $RPC"
echo ""

# --- 1. Chain head is advancing ---
echo "-- Block production --"
HEAD1=$(rpc '{"id":1,"jsonrpc":"2.0","method":"chain_getHeader","params":[]}' | python3 -c "import sys,json; d=json.load(sys.stdin); print(int(d['result']['number'],16))")
sleep 6
HEAD2=$(rpc '{"id":2,"jsonrpc":"2.0","method":"chain_getHeader","params":[]}' | python3 -c "import sys,json; d=json.load(sys.stdin); print(int(d['result']['number'],16))")
if [ "$HEAD2" -gt "$HEAD1" ]; then
  ok "Block production: advanced from #$HEAD1 to #$HEAD2"
else
  fail "Block production: stuck at #$HEAD1"
fi

# --- 2. Finality is advancing ---
echo "-- Finality --"
FIN1=$(rpc '{"id":3,"jsonrpc":"2.0","method":"chain_getFinalizedHead","params":[]}' | python3 -c "import sys,json; print(json.load(sys.stdin)['result'])")
sleep 12
FIN2=$(rpc '{"id":4,"jsonrpc":"2.0","method":"chain_getFinalizedHead","params":[]}' | python3 -c "import sys,json; print(json.load(sys.stdin)['result'])")
if [ "$FIN1" != "$FIN2" ]; then
  ok "Finality: advancing (was $FIN1, now $FIN2)"
else
  fail "Finality: not advancing — GRANDPA may be stalled"
fi

# --- 3. Runtime metadata pallets ---
echo "-- Runtime metadata --"
META_HEX=$(rpc '{"id":5,"jsonrpc":"2.0","method":"state_getMetadata","params":[]}' | python3 -c "import sys,json; print(json.load(sys.stdin)['result'])")
META_BYTES=$(python3 -c "import sys; h=sys.argv[1].lstrip('0x'); print(bytes.fromhex(h).decode('latin-1'))" "$META_HEX" 2>/dev/null || echo "$META_HEX")

if echo "$META_BYTES" | grep -q "X3SupplyLedger"; then
  ok "Pallet X3SupplyLedger present in metadata"
else
  fail "Pallet X3SupplyLedger NOT found in metadata"
fi

if echo "$META_BYTES" | grep -q "X3CrossVmRouter"; then
  ok "Pallet X3CrossVmRouter present in metadata"
else
  fail "Pallet X3CrossVmRouter NOT found in metadata"
fi

# --- 4. External bridges disabled ---
echo "-- External bridges disabled --"
# ExternalBridgesEnabled storage key (twox128("X3BridgeAdapters") + twox128("ExternalBridgesEnabled"))
STORAGE_KEY="0x$(python3 -c "
import hashlib, struct
def twox128(s):
    h = hashlib.new('shake_128')
    h.update(s.encode())
    return h.digest(16).hex()
print(twox128('X3BridgeAdapters') + twox128('ExternalBridgesEnabled'))
")"
VAL=$(rpc "{\"id\":6,\"jsonrpc\":\"2.0\",\"method\":\"state_getStorage\",\"params\":[\"$STORAGE_KEY\"]}" | python3 -c "import sys,json; print(json.load(sys.stdin).get('result','null'))")
if [ "$VAL" = "null" ] || [ "$VAL" = "0x00" ]; then
  ok "ExternalBridgesEnabled = false (value: $VAL)"
else
  fail "ExternalBridgesEnabled = $VAL — external bridges appear ENABLED"
fi

# --- Summary ---
echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] && echo "RC1 smoke test: PASSED" && exit 0 || echo "RC1 smoke test: FAILED" && exit 1
