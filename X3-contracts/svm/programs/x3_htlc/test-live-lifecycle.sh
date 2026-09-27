#!/usr/bin/env bash
# Real solana-test-validator lifecycle gate for the X3 `x3_htlc` SPL-token
# program (`X3-contracts/svm/programs/x3_htlc`).
#
# This proves, against a genuine local `solana-test-validator` running the
# compiled SBF program at the address its `declare_id!` fixes, that the escrow
# enforces the whole HTLC lifecycle end to end with real SPL tokens:
#
#   1. the program account is deployed and executable on-chain
#   2. a real SPL mint + token accounts are created against the on-chain token
#      program (no mock token accounting anywhere in this gate)
#   3. `create_htlc` locks tokens into the escrow vault PDA
#   4. the escrow account on-chain binds initiator, recipient, mint, amount,
#      hashlock, timelock and status (decoded by raw byte offset)
#   5. claim with a WRONG preimage is rejected and moves no tokens
#   6. claim by a party that is not the recorded recipient is rejected
#   7. claim with the CORRECT preimage succeeds, records the preimage on-chain
#      and releases exactly `amount` to the recipient
#   8. a second claim (double-claim) is rejected and the vault stays empty
#   9. refund after a successful claim is rejected
#  10. refund before the timelock expires is rejected
#  11. refund by a party that is not the recorded initiator is rejected
#  12. rejected creates (amount 0, timelock too short, timelock too long) are
#      rejected with the specific program error and leave no escrow behind
#  13. re-creating an identical lock is rejected (no escrow overwrite)
#  14. token supply is conserved across every path: initiator + recipient +
#      vault balances always sum to the minted supply
#
# The lock-claim path runs on real wall-clock time; the escrow's 1-hour
# minimum timelock means the *post-expiry refund* cannot be reached on a live
# validator. That branch is covered separately by the `solana-program-test`
# clock-warp suite in `tests-live/`, which executes the same compiled SBF
# program.
#
# Every on-chain assertion is made through raw JSON-RPC (`getAccountInfo`) and
# manual byte-offset decoding of the `Htlc` and SPL token-account layouts,
# independent of the broadcaster's own serialization, so a bug in the client
# cannot mask a bug in the on-chain program (or vice versa).
#
# Usage: bash X3-contracts/svm/programs/x3_htlc/test-live-lifecycle.sh
# Requires: solana, solana-keygen, solana-test-validator, spl-token and
# cargo-build-sbf on PATH.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
# `x3_htlc` is a member of the nested `X3-contracts/svm` workspace, so
# `cargo build-sbf` (run from the member directory) writes its artifact into the
# workspace root's `target/deploy`.
SVM_WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
RPC_PORT="${X3_HTLC_RPC_PORT:-18511}"
# Every validator needs its own faucet port too: the default (9900) is held by
# whichever live gate started first, and a faucet bind failure aborts startup
# while still leaving the RPC port bound.
FAUCET_PORT="${X3_HTLC_FAUCET_PORT:-$((RPC_PORT + 1))}"
RPC_URL="http://127.0.0.1:${RPC_PORT}"
WORKDIR="$(mktemp -d /tmp/x3-htlc-live.XXXXXX)"
VALIDATOR_PID=""

# Program id fixed by `declare_id!` in src/lib.rs. Deploying under any other
# address would prove nothing, so the gate pins the declared id instead of
# inventing a fresh one.
PROGRAM_ID="X3HTLC1111111111111111111111111111111111111"

# `Htlc` is Borsh: 8-byte discriminator + fields in declaration order.
#   0   .. 8    discriminator
#   8   .. 40   initiator      (Pubkey)
#   40  .. 72   recipient      (Pubkey)
#   72  .. 104  token_mint     (Pubkey)
#   104 .. 112  amount         (u64 LE)
#   112 .. 144  hashlock       ([u8; 32])
#   144 .. 152  timelock       (i64 LE)
#   152         status         (enum tag: 0 Pending 1 Funded 2 Claimed 3 Refunded 4 Expired)
#   153 .. 185  preimage       ([u8; 32])
#   185 .. 193  created_at     (i64 LE)
#   193         bump
# The account is allocated with `space = HTLC_SIZE` = 194 bytes of fields plus
# 64 bytes of forward-compatibility padding = 258 bytes.
EXPECTED_HTLC_ACCOUNT_LEN=258
STATUS_FUNDED=1
STATUS_CLAIMED=2

pass=0
fail=0

cleanup() {
  if [ -n "$VALIDATOR_PID" ] && kill -0 "$VALIDATOR_PID" 2>/dev/null; then
    kill "$VALIDATOR_PID" 2>/dev/null || true
    wait "$VALIDATOR_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

for tool in solana solana-keygen solana-test-validator spl-token curl python3; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "Error: $tool not installed"
    exit 1
  }
done
command -v cargo-build-sbf >/dev/null 2>&1 || {
  echo "Error: cargo-build-sbf not installed (Solana toolchain)"
  exit 1
}

# Persist a proof artifact when the repository layout is available. The gate is
# still runnable from a bare checkout, so this is best-effort.
RUNLOG_DIR="$REPO_ROOT/.ai/runlogs/x3-htlc-live-$(date -u +%Y%m%dT%H%M%SZ)"
if mkdir -p "$RUNLOG_DIR" 2>/dev/null; then
  exec > >(tee -a "$RUNLOG_DIR/report.txt") 2>&1
  echo "runlog: $RUNLOG_DIR"
fi

echo "workdir: $WORKDIR"
echo "rpc:     $RPC_URL (port $RPC_PORT)"

echo "=== build the SBF program ==="
# `cargo build-sbf` honors an ambient CARGO_TARGET_DIR (the local CI sets one to
# share a warm root target), which relocates the artifact this script looks
# for. Unset it so the asserted path below is the one the toolchain uses.
( cd "$SCRIPT_DIR" && env -u CARGO_TARGET_DIR cargo build-sbf 2>&1 | tail -3 )
PROGRAM_SO="$SVM_WORKSPACE_ROOT/target/deploy/x3_htlc.so"
[ -f "$PROGRAM_SO" ] || PROGRAM_SO="$SCRIPT_DIR/target/deploy/x3_htlc.so"
[ -f "$PROGRAM_SO" ] || {
  echo "Error: $PROGRAM_SO missing after cargo build-sbf"
  exit 1
}
echo "program so: $PROGRAM_SO ($(stat -c%s "$PROGRAM_SO") bytes)"

echo "=== build the broadcaster ==="
( cd "$SCRIPT_DIR/client" && env -u CARGO_TARGET_DIR cargo build --release --bin x3-htlc-broadcast 2>&1 | tail -3 )
BIN="$SCRIPT_DIR/client/target/release/x3-htlc-broadcast"
[ -x "$BIN" ] || {
  echo "Error: $BIN missing after build"
  exit 1
}

echo "=== generating keypairs ==="
solana-keygen new --no-bip39-passphrase -s -o "$WORKDIR/initiator.json" >/dev/null
solana-keygen new --no-bip39-passphrase -s -o "$WORKDIR/recipient.json" >/dev/null
solana-keygen new --no-bip39-passphrase -s -o "$WORKDIR/stranger.json" >/dev/null
INITIATOR_PK="$(solana-keygen pubkey "$WORKDIR/initiator.json")"
RECIPIENT_PK="$(solana-keygen pubkey "$WORKDIR/recipient.json")"
STRANGER_PK="$(solana-keygen pubkey "$WORKDIR/stranger.json")"
echo "initiator=$INITIATOR_PK recipient=$RECIPIENT_PK stranger=$STRANGER_PK"

echo "=== starting solana-test-validator ==="
solana-test-validator --reset --quiet \
  --rpc-port "$RPC_PORT" \
  --faucet-port "$FAUCET_PORT" \
  --ledger "$WORKDIR/ledger" \
  --bpf-program "$PROGRAM_ID" "$PROGRAM_SO" \
  > "$WORKDIR/validator.log" 2>&1 &
VALIDATOR_PID=$!

for i in $(seq 1 90); do
  if solana cluster-version --url "$RPC_URL" >/dev/null 2>&1; then
    break
  fi
  sleep 1
  if [ "$i" -eq 90 ]; then
    echo "Error: validator did not become ready"
    cat "$WORKDIR/validator.log"
    exit 1
  fi
done
echo "validator ready: $(solana cluster-version --url "$RPC_URL")"

# A working-directory CLI config keeps `spl-token` from needing (or writing) a
# global ~/.config/solana file.
CFG="$WORKDIR/solana-cli.yml"
solana config set --config "$CFG" --url "$RPC_URL" --keypair "$WORKDIR/initiator.json" \
  --commitment confirmed >/dev/null

solana airdrop 20 --url "$RPC_URL" --keypair "$WORKDIR/initiator.json" --commitment finalized >/dev/null
solana airdrop 5 --url "$RPC_URL" --keypair "$WORKDIR/recipient.json" --commitment finalized >/dev/null
solana airdrop 5 --url "$RPC_URL" --keypair "$WORKDIR/stranger.json" --commitment finalized >/dev/null

# Airdrops must be FINALIZED before transactions debit these accounts, since the
# broadcaster's RPC client builds transactions at finalized commitment.
for who in initiator recipient stranger; do
  for _ in $(seq 1 60); do
    bal="$(solana balance --url "$RPC_URL" --keypair "$WORKDIR/$who.json" --commitment finalized 2>/dev/null | grep -oP '^\d+(\.\d+)?' || echo 0)"
    awk -v b="$bal" 'BEGIN{exit !(b>0)}' && break
    sleep 1
  done
done

json_rpc() {
  curl -s "$RPC_URL" -X POST -H 'Content-Type: application/json' -d "$1"
}

# All reads use `finalized`. The broadcaster's `RpcClient` preflights at its
# configured commitment (finalized by default), so the bank the program
# simulates against is the rooted bank: any account created by a merely
# *confirmed* transaction — every `spl-token` CLI call below — is invisible to
# the program until it roots. Reading at finalized keeps this gate's assertions
# and the program's own view on the same bank.
account_len() {
  json_rpc "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"$1\",{\"encoding\":\"base64\",\"commitment\":\"finalized\"}]}" \
    | python3 -c "
import base64, json, sys
r = json.load(sys.stdin)['result']['value']
print('MISSING' if r is None else len(base64.b64decode(r['data'][0])))
"
}

account_owner() {
  json_rpc "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"$1\",{\"encoding\":\"base64\",\"commitment\":\"finalized\"}]}" \
    | python3 -c "
import json, sys
r = json.load(sys.stdin)['result']['value']
print('MISSING' if r is None else r['owner'])
"
}

# wait_for_account <pubkey> <description> — poll until the account is visible in
# the finalized bank (see the note above `account_len`).
wait_for_account() {
  local pk="$1" desc="$2"
  for _ in $(seq 1 90); do
    if [ "$(account_len "$pk")" != "MISSING" ]; then
      return 0
    fi
    sleep 1
  done
  echo "Error: $desc ($pk) never became visible in the finalized bank"
  return 1
}

# Raw byte-offset decode of the on-chain Htlc account (layout table above),
# independent of the broadcaster's serialization.
decode_field() {
  local pda="$1" field="$2"
  json_rpc "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"$pda\",{\"encoding\":\"base64\",\"commitment\":\"finalized\"}]}" \
    | python3 -c "
import base64, json, sys
FIELDS = {
    'initiator': (8, 40),
    'recipient': (40, 72),
    'token_mint': (72, 104),
    'amount': (104, 112),
    'hashlock': (112, 144),
    'timelock': (144, 152),
    'status': (152, 153),
    'preimage': (153, 185),
    'created_at': (185, 193),
    'bump': (193, 194),
}
ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
value = json.load(sys.stdin)['result']['value']
if value is None:
    print('MISSING')
    raise SystemExit(0)
data = base64.b64decode(value['data'][0])
field = sys.argv[1]
start, end = FIELDS[field]
raw = data[start:end]
if field in ('amount', 'timelock', 'created_at', 'bump', 'status'):
    print(int.from_bytes(raw, 'little'))
elif field in ('hashlock', 'preimage'):
    print(raw.hex())
else:
    n = int.from_bytes(raw, 'big')
    out = ''
    while n:
        n, rem = divmod(n, 58)
        out = ALPHABET[rem] + out
    for byte in raw:
        if byte == 0:
            out = '1' + out
        else:
            break
    print(out)
" "$field"
}

# Raw SPL values decoded from account data rather than read back through the
# CLI, so a client-side accounting bug cannot pass this gate.
#
# SPL Mint layout:          mint_authority(4+32) supply(u64 LE @36) decimals(u8 @44)
# SPL Token Account layout: mint(32) owner(32) amount(u64 LE @64)
mint_decimals() {
  json_rpc "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"$1\",{\"encoding\":\"base64\",\"commitment\":\"finalized\"}]}" \
    | python3 -c "
import base64, json, sys
r = json.load(sys.stdin)['result']['value']
print('MISSING' if r is None else base64.b64decode(r['data'][0])[44])
"
}

mint_supply() {
  json_rpc "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"$1\",{\"encoding\":\"base64\",\"commitment\":\"finalized\"}]}" \
    | python3 -c "
import base64, json, sys
r = json.load(sys.stdin)['result']['value']
print('MISSING' if r is None else int.from_bytes(base64.b64decode(r['data'][0])[36:44], 'little'))
"
}

token_amount() {
  json_rpc "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"$1\",{\"encoding\":\"base64\",\"commitment\":\"finalized\"}]}" \
    | python3 -c "
import base64, json, sys
r = json.load(sys.stdin)['result']['value']
print('MISSING' if r is None else int.from_bytes(base64.b64decode(r['data'][0])[64:72], 'little'))
"
}

zero_if_missing() {
  if [ "$1" = "MISSING" ]; then echo 0; else echo "$1"; fi
}

# wait_for_token_amount <token-account> <expected> <description>
wait_for_token_amount() {
  local pk="$1" expected="$2" desc="$3"
  local observed="MISSING"
  for _ in $(seq 1 90); do
    observed="$(zero_if_missing "$(token_amount "$pk")")"
    if [ "$observed" = "$expected" ]; then
      return 0
    fi
    sleep 1
  done
  echo "Error: $desc ($pk) never reached $expected base units in the finalized bank (last observed: $observed)"
  return 1
}

# initiator ATA + recipient ATA + escrow vault must always equal the supply.
conservation() {
  local vault="$1"
  echo "$(( $(zero_if_missing "$(token_amount "$INITIATOR_ATA")") \
          + $(zero_if_missing "$(token_amount "$RECIPIENT_ATA")") \
          + $(zero_if_missing "$(token_amount "$vault")") ))"
}

check() {
  local desc="$1" expect_fail="$2"
  shift 2
  if "$@" >"$WORKDIR/last.out" 2>&1; then
    if [ "$expect_fail" = "1" ]; then
      echo "FAIL: $desc (expected rejection, but succeeded)"
      cat "$WORKDIR/last.out"
      fail=$((fail + 1))
    else
      echo "PASS: $desc"
      pass=$((pass + 1))
    fi
  else
    if [ "$expect_fail" = "1" ]; then
      echo "PASS: $desc (correctly rejected)"
      pass=$((pass + 1))
    else
      echo "FAIL: $desc"
      cat "$WORKDIR/last.out"
      fail=$((fail + 1))
    fi
  fi
}

check_reject() {
  # check_reject <description> <expected program-error regex> <command...>
  local desc="$1" expected="$2"
  shift 2
  if "$@" >"$WORKDIR/last.out" 2>&1; then
    echo "FAIL: $desc (expected rejection, but succeeded)"
    cat "$WORKDIR/last.out"
    fail=$((fail + 1))
  elif grep -Eq "$expected" "$WORKDIR/last.out"; then
    echo "PASS: $desc (rejected with the expected program error)"
    pass=$((pass + 1))
  else
    echo "FAIL: $desc (rejected with an unexpected error)"
    cat "$WORKDIR/last.out"
    fail=$((fail + 1))
  fi
}

assert_eq() {
  # assert_eq <description> <expected> <actual>
  if [ "$2" = "$3" ]; then
    echo "PASS: $1"
    pass=$((pass + 1))
  else
    echo "FAIL: $1 (expected '$2', got '$3')"
    fail=$((fail + 1))
  fi
}

# ─────────────────────────── deployment evidence ────────────────────────────
executable=""
for i in $(seq 1 30); do
  executable="$(json_rpc "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"$PROGRAM_ID\",{\"encoding\":\"base64\"}]}" \
    | python3 -c "import json,sys; r=json.load(sys.stdin)['result']['value']; print(r['executable'] if r else 'False')")"
  [ "$executable" = "True" ] && break
  sleep 1
done
if [ "$executable" != "True" ]; then
  echo "FAIL: $PROGRAM_ID is not an executable on-chain program"
  cat "$WORKDIR/validator.log"
  exit 1
fi
echo "PASS: x3_htlc is deployed and executable on-chain at $PROGRAM_ID"
pass=$((pass + 1))

# ─────────────────────────── real SPL token setup ───────────────────────────
echo "=== creating SPL mint and token accounts ==="
spl-token --config "$CFG" create-token > "$WORKDIR/mint.out" 2>&1 || {
  echo "Error: spl-token create-token failed"
  cat "$WORKDIR/mint.out"
  exit 1
}
MINT="$(grep -oP 'Creating token \K[1-9A-HJ-NP-Za-km-z]+' "$WORKDIR/mint.out" | head -1)"
[ -n "$MINT" ] || {
  echo "Error: could not parse the mint address"
  cat "$WORKDIR/mint.out"
  exit 1
}
echo "mint=$MINT"
wait_for_account "$MINT" "SPL mint"
assert_eq "SPL mint is owned by the on-chain token program" \
  "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA" "$(account_owner "$MINT")"

FEE_ARGS=(--config "$CFG" --fee-payer "$WORKDIR/initiator.json")
spl-token "${FEE_ARGS[@]}" create-account "$MINT" --owner "$INITIATOR_PK" | tail -2
spl-token "${FEE_ARGS[@]}" create-account "$MINT" --owner "$RECIPIENT_PK" | tail -2
spl-token "${FEE_ARGS[@]}" create-account "$MINT" --owner "$STRANGER_PK" | tail -2

# Resolve the token accounts through the node (not the CLI's own derivation) so
# the addresses this gate funds are provably the ones the node holds.
find_ata() {
  local owner="$1"
  json_rpc "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getTokenAccountsByOwner\",\"params\":[\"$owner\",{\"mint\":\"$MINT\"},{\"encoding\":\"base64\",\"commitment\":\"finalized\"}]}" \
    | python3 -c "
import json, sys
r = json.load(sys.stdin)
if 'result' not in r:
    raise SystemExit(f'RPC error: {r}')
values = r['result']['value']
print(values[0]['pubkey'] if values else 'MISSING')
"
}

INITIATOR_ATA="MISSING"
RECIPIENT_ATA="MISSING"
STRANGER_ATA="MISSING"
for _ in $(seq 1 30); do
  INITIATOR_ATA="$(find_ata "$INITIATOR_PK")"
  RECIPIENT_ATA="$(find_ata "$RECIPIENT_PK")"
  STRANGER_ATA="$(find_ata "$STRANGER_PK")"
  if [ "$INITIATOR_ATA" != "MISSING" ] && [ "$RECIPIENT_ATA" != "MISSING" ] \
     && [ "$STRANGER_ATA" != "MISSING" ]; then
    break
  fi
  sleep 1
done
if [ "$INITIATOR_ATA" = "MISSING" ] || [ "$RECIPIENT_ATA" = "MISSING" ] \
   || [ "$STRANGER_ATA" = "MISSING" ]; then
  echo "Error: token accounts never became visible on-chain ($INITIATOR_ATA / $RECIPIENT_ATA / $STRANGER_ATA)"
  exit 1
fi
echo "initiator_ata=$INITIATOR_ATA recipient_ata=$RECIPIENT_ATA stranger_ata=$STRANGER_ATA"

# Denominations come from the mint's own on-chain `decimals`, so the lock amount
# asserted below is an exact base-unit value regardless of the CLI's default.
MINT_DECIMALS="$(mint_decimals "$MINT")"
case "$MINT_DECIMALS" in
  MISSING|"") echo "Error: could not read the on-chain mint decimals"; exit 1 ;;
  [0-9]|[0-9][0-9]) ;;
  *) echo "Error: implausible on-chain mint decimals '$MINT_DECIMALS'"; exit 1 ;;
esac
WHOLE_TOKENS=1000
MINTED_AMOUNT=$(( WHOLE_TOKENS * 10 ** MINT_DECIMALS ))
LOCK_AMOUNT=$(( (WHOLE_TOKENS / 2) * 10 ** MINT_DECIMALS ))
echo "mint decimals=$MINT_DECIMALS minted=$MINTED_AMOUNT lock=$LOCK_AMOUNT (base units)"

# The fee payer is the initiator, so a plain `mint` credits the initiator's
# token account for the mint.
spl-token "${FEE_ARGS[@]}" mint "$MINT" "$WHOLE_TOKENS" | tail -2
# The CLI confirms at `confirmed`; the program preflights against the rooted
# bank, so the minted supply has to root before the first lock is submitted.
wait_for_token_amount "$INITIATOR_ATA" "$MINTED_AMOUNT" "initiator minted balance"
assert_eq "initiator token account holds the minted supply before locking" \
  "$MINTED_AMOUNT" "$(token_amount "$INITIATOR_ATA")"
assert_eq "on-chain mint supply equals the minted amount" \
  "$MINTED_AMOUNT" "$(mint_supply "$MINT")"

# ───────────────────────── scenario 1: lock → claim ─────────────────────────
python3 - "$WORKDIR" <<'PY'
import hashlib, os, sys

work = sys.argv[1]
preimage = os.urandom(32)
open(f"{work}/preimage.hex", "w").write(preimage.hex())
open(f"{work}/hashlock.hex", "w").write(hashlib.sha256(preimage).digest().hex())
PY
HASHLOCK="$(cat "$WORKDIR/hashlock.hex")"
PREIMAGE="$(cat "$WORKDIR/preimage.hex")"
TIMELOCK=$(( $(date +%s) + 7200 ))

ADDRS="$("$BIN" --program-id "$PROGRAM_ID" addresses \
  --initiator "$INITIATOR_PK" --recipient "$RECIPIENT_PK" --hashlock "$HASHLOCK")"
HTLC_PDA="$(grep -oP 'htlc=\K\S+' <<<"$ADDRS")"
VAULT_PDA="$(grep -oP 'vault=\K\S+' <<<"$ADDRS")"
[ -n "$HTLC_PDA" ] && [ -n "$VAULT_PDA" ] || {
  echo "Error: PDA derivation failed: $ADDRS"
  exit 1
}
echo "escrow=$HTLC_PDA vault=$VAULT_PDA"

assert_eq "escrow account does not exist before the lock" "MISSING" "$(account_len "$HTLC_PDA")"

check "lock (happy path, real SPL transfer into the escrow vault)" 0 \
  "$BIN" --rpc "$RPC_URL" --program-id "$PROGRAM_ID" --payer-keypair "$WORKDIR/initiator.json" \
  create --recipient "$RECIPIENT_PK" --mint "$MINT" --initiator-token-account "$INITIATOR_ATA" \
  --hashlock "$HASHLOCK" --timelock "$TIMELOCK" --amount "$LOCK_AMOUNT"

wait_for_account "$HTLC_PDA" "escrow account"
assert_eq "on-chain escrow account has the allocated HTLC_SIZE" \
  "$EXPECTED_HTLC_ACCOUNT_LEN" "$(account_len "$HTLC_PDA")"
assert_eq "on-chain escrow is owned by the x3_htlc program" "$PROGRAM_ID" "$(account_owner "$HTLC_PDA")"
assert_eq "on-chain status == Funded(1) after the lock" "$STATUS_FUNDED" "$(decode_field "$HTLC_PDA" status)"
assert_eq "on-chain amount == the locked amount" "$LOCK_AMOUNT" "$(decode_field "$HTLC_PDA" amount)"
assert_eq "on-chain timelock == the requested timelock" "$TIMELOCK" "$(decode_field "$HTLC_PDA" timelock)"
assert_eq "on-chain hashlock == sha256(preimage)" "$HASHLOCK" "$(decode_field "$HTLC_PDA" hashlock)"
assert_eq "on-chain initiator == the signing initiator" "$INITIATOR_PK" "$(decode_field "$HTLC_PDA" initiator)"
assert_eq "on-chain recipient == the requested recipient" "$RECIPIENT_PK" "$(decode_field "$HTLC_PDA" recipient)"
assert_eq "on-chain token_mint == the SPL mint" "$MINT" "$(decode_field "$HTLC_PDA" token_mint)"
assert_eq "on-chain preimage is zeroed until claimed" \
  "0000000000000000000000000000000000000000000000000000000000000000" "$(decode_field "$HTLC_PDA" preimage)"
assert_eq "escrow vault holds exactly the locked amount" \
  "$LOCK_AMOUNT" "$(token_amount "$VAULT_PDA")"
assert_eq "initiator balance decreased by exactly the locked amount" \
  "$((MINTED_AMOUNT - LOCK_AMOUNT))" "$(token_amount "$INITIATOR_ATA")"
assert_eq "supply is conserved after the lock" "$MINTED_AMOUNT" "$(conservation "$VAULT_PDA")"

check_reject "claim with a WRONG preimage" \
  "0x1773|InvalidPreimage|does not match hashlock" \
  "$BIN" --rpc "$RPC_URL" --program-id "$PROGRAM_ID" --payer-keypair "$WORKDIR/recipient.json" \
  claim --escrow "$HTLC_PDA" --recipient-token-account "$RECIPIENT_ATA" \
  --preimage "$(python3 -c 'import os; print(os.urandom(32).hex())')"
assert_eq "rejected wrong-preimage claim left the escrow Funded" "$STATUS_FUNDED" "$(decode_field "$HTLC_PDA" status)"
assert_eq "rejected wrong-preimage claim moved no tokens" "$LOCK_AMOUNT" "$(token_amount "$VAULT_PDA")"

check_reject "claim by a party that is not the recorded recipient" \
  "0x1777|NotRecipient|Only the recipient can claim" \
  "$BIN" --rpc "$RPC_URL" --program-id "$PROGRAM_ID" --payer-keypair "$WORKDIR/stranger.json" \
  claim --escrow "$HTLC_PDA" --recipient-token-account "$STRANGER_ATA" --preimage "$PREIMAGE"
assert_eq "rejected unauthorized claim left the escrow Funded" "$STATUS_FUNDED" "$(decode_field "$HTLC_PDA" status)"
assert_eq "rejected unauthorized claim moved no tokens" "$LOCK_AMOUNT" "$(token_amount "$VAULT_PDA")"

RECIPIENT_BEFORE="$(zero_if_missing "$(token_amount "$RECIPIENT_ATA")")"
check "claim with the CORRECT preimage" 0 \
  "$BIN" --rpc "$RPC_URL" --program-id "$PROGRAM_ID" --payer-keypair "$WORKDIR/recipient.json" \
  claim --escrow "$HTLC_PDA" --recipient-token-account "$RECIPIENT_ATA" --preimage "$PREIMAGE"

assert_eq "on-chain status == Claimed(2) after the claim" "$STATUS_CLAIMED" "$(decode_field "$HTLC_PDA" status)"
assert_eq "on-chain preimage records the revealed secret" "$PREIMAGE" "$(decode_field "$HTLC_PDA" preimage)"
assert_eq "escrow vault was drained by the claim" "0" "$(token_amount "$VAULT_PDA")"
assert_eq "recipient received exactly the locked amount" \
  "$((RECIPIENT_BEFORE + LOCK_AMOUNT))" "$(token_amount "$RECIPIENT_ATA")"
assert_eq "supply is conserved after the claim" "$MINTED_AMOUNT" "$(conservation "$VAULT_PDA")"

check_reject "double-claim on a Claimed escrow" \
  "0x1774|HtlcNotClaimable|not in claimable state" \
  "$BIN" --rpc "$RPC_URL" --program-id "$PROGRAM_ID" --payer-keypair "$WORKDIR/recipient.json" \
  claim --escrow "$HTLC_PDA" --recipient-token-account "$RECIPIENT_ATA" --preimage "$PREIMAGE"
assert_eq "double-claim moved nothing" "$MINTED_AMOUNT" "$(conservation "$VAULT_PDA")"
assert_eq "double-claim left the escrow Claimed" "$STATUS_CLAIMED" "$(decode_field "$HTLC_PDA" status)"

check_reject "refund after a successful claim" \
  "0x1775|HtlcNotRefundable|not in refundable state" \
  "$BIN" --rpc "$RPC_URL" --program-id "$PROGRAM_ID" --payer-keypair "$WORKDIR/initiator.json" \
  refund --escrow "$HTLC_PDA" --initiator-token-account "$INITIATOR_ATA"
assert_eq "post-claim refund attempt changed no balances" "$MINTED_AMOUNT" "$(conservation "$VAULT_PDA")"

# ──────────────────── scenario 2: refund-path rejections ────────────────────
python3 - "$WORKDIR" <<'PY'
import hashlib, os, sys

work = sys.argv[1]
preimage = os.urandom(32)
open(f"{work}/preimage2.hex", "w").write(preimage.hex())
open(f"{work}/hashlock2.hex", "w").write(hashlib.sha256(preimage).digest().hex())
PY
HASHLOCK2="$(cat "$WORKDIR/hashlock2.hex")"
ADDRS2="$("$BIN" --program-id "$PROGRAM_ID" addresses \
  --initiator "$INITIATOR_PK" --recipient "$RECIPIENT_PK" --hashlock "$HASHLOCK2")"
HTLC_PDA2="$(grep -oP 'htlc=\K\S+' <<<"$ADDRS2")"
VAULT_PDA2="$(grep -oP 'vault=\K\S+' <<<"$ADDRS2")"

check "lock #2 (long timelock, for the refund-path rejections)" 0 \
  "$BIN" --rpc "$RPC_URL" --program-id "$PROGRAM_ID" --payer-keypair "$WORKDIR/initiator.json" \
  create --recipient "$RECIPIENT_PK" --mint "$MINT" --initiator-token-account "$INITIATOR_ATA" \
  --hashlock "$HASHLOCK2" --timelock "$(( $(date +%s) + 7200 ))" --amount "$LOCK_AMOUNT"

check_reject "refund BEFORE the timelock expires" \
  "0x1776|TimelockNotExpired|has not expired" \
  "$BIN" --rpc "$RPC_URL" --program-id "$PROGRAM_ID" --payer-keypair "$WORKDIR/initiator.json" \
  refund --escrow "$HTLC_PDA2" --initiator-token-account "$INITIATOR_ATA"
assert_eq "rejected early refund left the escrow Funded" "$STATUS_FUNDED" "$(decode_field "$HTLC_PDA2" status)"
assert_eq "rejected early refund kept the tokens in the vault" "$LOCK_AMOUNT" "$(token_amount "$VAULT_PDA2")"

check_reject "refund by a party that is not the recorded initiator" \
  "0x1778|NotInitiator|Only the initiator can refund" \
  "$BIN" --rpc "$RPC_URL" --program-id "$PROGRAM_ID" --payer-keypair "$WORKDIR/recipient.json" \
  refund --escrow "$HTLC_PDA2" --initiator-token-account "$RECIPIENT_ATA"
assert_eq "rejected unauthorized refund moved no tokens" "$LOCK_AMOUNT" "$(token_amount "$VAULT_PDA2")"
assert_eq "rejected unauthorized refund left the escrow Funded" "$STATUS_FUNDED" "$(decode_field "$HTLC_PDA2" status)"

# ───────────────────── scenario 3: rejected lock shapes ─────────────────────
python3 - "$WORKDIR" <<'PY'
import hashlib, os, sys

work = sys.argv[1]
open(f"{work}/hashlock3.hex", "w").write(hashlib.sha256(os.urandom(32)).digest().hex())
PY
HASHLOCK3="$(cat "$WORKDIR/hashlock3.hex")"

check_reject "lock with amount = 0" \
  "0x1772|InvalidAmount|Amount must be greater than 0" \
  "$BIN" --rpc "$RPC_URL" --program-id "$PROGRAM_ID" --payer-keypair "$WORKDIR/initiator.json" \
  create --recipient "$RECIPIENT_PK" --mint "$MINT" --initiator-token-account "$INITIATOR_ATA" \
  --hashlock "$HASHLOCK3" --timelock "$(( $(date +%s) + 7200 ))" --amount 0

check_reject "lock with a timelock shorter than the 1-hour minimum" \
  "0x1770|TimelockTooShort|at least 1 hour" \
  "$BIN" --rpc "$RPC_URL" --program-id "$PROGRAM_ID" --payer-keypair "$WORKDIR/initiator.json" \
  create --recipient "$RECIPIENT_PK" --mint "$MINT" --initiator-token-account "$INITIATOR_ATA" \
  --hashlock "$HASHLOCK3" --timelock "$(( $(date +%s) + 60 ))" --amount "$LOCK_AMOUNT"

check_reject "lock with a timelock longer than the 7-day maximum" \
  "0x1771|TimelockTooLong|cannot exceed 7 days" \
  "$BIN" --rpc "$RPC_URL" --program-id "$PROGRAM_ID" --payer-keypair "$WORKDIR/initiator.json" \
  create --recipient "$RECIPIENT_PK" --mint "$MINT" --initiator-token-account "$INITIATOR_ATA" \
  --hashlock "$HASHLOCK3" --timelock "$(( $(date +%s) + 8 * 24 * 3600 ))" --amount "$LOCK_AMOUNT"

ADDRS3="$("$BIN" --program-id "$PROGRAM_ID" addresses \
  --initiator "$INITIATOR_PK" --recipient "$RECIPIENT_PK" --hashlock "$HASHLOCK3")"
HTLC_PDA3="$(grep -oP 'htlc=\K\S+' <<<"$ADDRS3")"
assert_eq "rejected lock shapes created no escrow account" "MISSING" "$(account_len "$HTLC_PDA3")"
assert_eq "rejected lock shapes moved no tokens" \
  "$LOCK_AMOUNT" "$(token_amount "$VAULT_PDA2")"

check_reject "re-locking an existing escrow identity" \
  "already in use|already exists|AccountAlreadyInUse|0xbc2" \
  "$BIN" --rpc "$RPC_URL" --program-id "$PROGRAM_ID" --payer-keypair "$WORKDIR/initiator.json" \
  create --recipient "$RECIPIENT_PK" --mint "$MINT" --initiator-token-account "$INITIATOR_ATA" \
  --hashlock "$HASHLOCK2" --timelock "$(( $(date +%s) + 7200 ))" --amount "$LOCK_AMOUNT"
assert_eq "rejected re-lock left the original escrow untouched" "$STATUS_FUNDED" "$(decode_field "$HTLC_PDA2" status)"
assert_eq "rejected re-lock moved no extra tokens" "$LOCK_AMOUNT" "$(token_amount "$VAULT_PDA2")"

# ───────────────── scenario 4: restart recovery on the same ledger ──────────
# An escrow that only exists in a process's memory is not an escrow. Restart
# the validator on the same ledger and require the funded escrow, its vault
# balance and its timelock rule to come back from durable state.
echo "=== restarting the validator on the same ledger ==="
kill "$VALIDATOR_PID" 2>/dev/null || true
wait "$VALIDATOR_PID" 2>/dev/null || true
VALIDATOR_PID=""

solana-test-validator --quiet \
  --rpc-port "$RPC_PORT" \
  --faucet-port "$FAUCET_PORT" \
  --ledger "$WORKDIR/ledger" \
  > "$WORKDIR/validator-restart.log" 2>&1 &
VALIDATOR_PID=$!

restarted=0
for i in $(seq 1 90); do
  if solana cluster-version --url "$RPC_URL" >/dev/null 2>&1; then
    restarted=1
    break
  fi
  sleep 1
done
if [ "$restarted" != "1" ]; then
  echo "FAIL: the validator did not come back up on the same ledger"
  cat "$WORKDIR/validator-restart.log"
  fail=$((fail + 1))
else
  echo "PASS: the validator restarted on the same ledger"
  pass=$((pass + 1))
fi

wait_for_account "$HTLC_PDA2" "escrow after restart"
assert_eq "the funded escrow survives a validator restart" "$STATUS_FUNDED" "$(decode_field "$HTLC_PDA2" status)"
assert_eq "the escrow amount survives a validator restart" "$LOCK_AMOUNT" "$(decode_field "$HTLC_PDA2" amount)"
assert_eq "the escrow hashlock survives a validator restart" "$HASHLOCK2" "$(decode_field "$HTLC_PDA2" hashlock)"
assert_eq "the vault balance survives a validator restart" "$LOCK_AMOUNT" "$(token_amount "$VAULT_PDA2")"
assert_eq "the claimed escrow is still Claimed after a restart" "$STATUS_CLAIMED" "$(decode_field "$HTLC_PDA" status)"

check_reject "refund before the timelock is still refused after a restart" \
  "0x1776|TimelockNotExpired|has not expired" \
  "$BIN" --rpc "$RPC_URL" --program-id "$PROGRAM_ID" --payer-keypair "$WORKDIR/initiator.json" \
  refund --escrow "$HTLC_PDA2" --initiator-token-account "$INITIATOR_ATA"
assert_eq "the post-restart rejection moved no tokens" "$LOCK_AMOUNT" "$(token_amount "$VAULT_PDA2")"

check_reject "double-claim on the settled escrow is still refused after a restart" \
  "0x1774|HtlcNotClaimable|not in claimable state" \
  "$BIN" --rpc "$RPC_URL" --program-id "$PROGRAM_ID" --payer-keypair "$WORKDIR/recipient.json" \
  claim --escrow "$HTLC_PDA" --recipient-token-account "$RECIPIENT_ATA" --preimage "$PREIMAGE"

assert_eq "supply is conserved across every path" "$MINTED_AMOUNT" \
  "$(( $(zero_if_missing "$(token_amount "$INITIATOR_ATA")") \
      + $(zero_if_missing "$(token_amount "$RECIPIENT_ATA")") \
      + $(zero_if_missing "$(token_amount "$VAULT_PDA2")") ))"

echo
echo "=== Results: $pass passed, $fail failed ==="
echo "escrow(claimed)=$HTLC_PDA"
echo "escrow(funded)=$HTLC_PDA2"
if [ "$fail" -ne 0 ]; then
  echo "program log tail:"
  tail -20 "$WORKDIR/validator.log" || true
  exit 1
fi
rm -rf "$WORKDIR"
