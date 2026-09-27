#!/usr/bin/env bash
# testnet-ceremony-drill.sh — record a launch, then verify it, then prove the
# verifier can fail.
#
# What a published testnet needs is not only that it starts: it needs a record of
# *what* was launched (the spec, the binary, the genesis hash, the authorities, the
# runtime version, the bootnode identities) that anyone can check a running network
# against. This drill produces that record from a real launch and then verifies it —
# and, because a verifier nobody has seen fail is not evidence, it tampers with a copy
# of the manifest and requires the verifier to reject it.
#
# Usage:
#   scripts/testnet/testnet-ceremony-drill.sh [--count N] [--keep]
# Env: NODE_BIN, BASE_DIR (default /tmp/x3-testnet-ceremony)
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COUNT="${COUNT:-4}"
BASE_DIR="${BASE_DIR:-/tmp/x3-testnet-ceremony}"
LOG_DIR="$BASE_DIR/logs"
MANIFEST="$BASE_DIR/ceremony.json"
KEEP="${KEEP:-0}"

# Same binary discovery as the launcher.
NODE_BIN="${NODE_BIN:-}"
if [[ -z "$NODE_BIN" ]]; then
  for candidate in \
    "${CARGO_TARGET_DIR:-$ROOT_DIR/target}/release/x3-chain-node" \
    "${CARGO_TARGET_DIR:-$ROOT_DIR/target}/debug/x3-chain-node" \
    "$ROOT_DIR/target/release/x3-chain-node" \
    "$ROOT_DIR/target/debug/x3-chain-node"; do
    [[ -x "$candidate" ]] && NODE_BIN="$candidate" && break
  done
fi
[[ -n "$NODE_BIN" && -x "$NODE_BIN" ]] || {
  echo "node binary not found; build it with cargo build -p x3-chain-node" >&2
  exit 1
}

RPC_BASE="${RPC_BASE:-9944}"
PORTS="$(seq -s, "$RPC_BASE" $((RPC_BASE + COUNT - 1)))"
SPEC="${CHAIN_SPEC:-$ROOT_DIR/deployment/chain-specs/fresh/generated/x3-testnet-plain.json}"

info() { printf '[ceremony-drill] %s\n' "$*"; }
fail() { printf '[ceremony-drill] FAIL: %s\n' "$*" >&2; exit 1; }
pass() { printf '[ceremony-drill] PASS: %s\n' "$*"; }

cleanup() {
  [[ "$KEEP" == "1" ]] && { info "KEEP=1 — leaving the network under $BASE_DIR"; return; }
  local f
  for f in "$BASE_DIR"/pids/node-*.pid; do
    [[ -f "$f" ]] || continue
    kill -9 "$(cat "$f")" 2>/dev/null || true
  done
  pkill -f -- "--base-path $BASE_DIR/node-" 2>/dev/null || true
}
trap cleanup EXIT

rm -rf "$BASE_DIR"
mkdir -p "$LOG_DIR"

info "building a ${COUNT}-authority Live spec"
X3_NODE_BIN="$NODE_BIN" python3 "$ROOT_DIR/scripts/testnet/build-x3-testnet-spec.py" "$COUNT" \
  >"$BASE_DIR.spec.log" 2>&1 || { tail -20 "$BASE_DIR.spec.log" >&2; fail "spec build failed"; }
[[ -f "$SPEC" ]] || fail "expected $SPEC after the build"
info "spec: $SPEC"

info "launching ${COUNT} validators through x3_testnet_up.sh"
COUNT="$COUNT" NODE_BIN="$NODE_BIN" CHAIN_SPEC="$SPEC" BASE_DIR="$BASE_DIR" LOG_DIR="$LOG_DIR" \
  SKIP_BUILD=1 bash "$ROOT_DIR/scripts/testnet/x3_testnet_up.sh" --skip-build \
  >"$BASE_DIR.launch.log" 2>&1 || { tail -20 "$BASE_DIR.launch.log" >&2; fail "launch failed"; }

# Wait for every validator to finalize a few blocks, so the manifest records a live
# network rather than a booting one.
deadline=$(( $(date +%s) + 300 ))
while :; do
  done_all=1
  for port in $(seq "$RPC_BASE" $((RPC_BASE + COUNT - 1))); do
    head=$(curl -s -m 5 -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"chain_getFinalizedHead","params":[]}' \
      "http://127.0.0.1:$port" | python3 -c "import json,sys;print(json.load(sys.stdin).get('result',''))" 2>/dev/null || true)
    [[ -n "$head" ]] || { done_all=0; break; }
    num=$(curl -s -m 5 -H 'Content-Type: application/json' \
      -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"chain_getHeader\",\"params\":[\"$head\"]}" \
      "http://127.0.0.1:$port" | python3 -c "import json,sys;h=(json.load(sys.stdin).get('result') or {});print(int(h.get('number','0x0'),16) if h else 0)" 2>/dev/null || echo 0)
    [[ "${num:-0}" -ge 5 ]] || { done_all=0; break; }
  done
  [[ "$done_all" == "1" ]] && break
  [[ "$(date +%s)" -lt "$deadline" ]] || fail "the network did not finalize 5 blocks on every validator"
  sleep 3
done
pass "${COUNT} validators finalizing"

info "recording the ceremony manifest"
python3 "$ROOT_DIR/scripts/testnet/testnet-ceremony.py" record "$SPEC" \
  --node-bin "$NODE_BIN" --rpc "$PORTS" --out "$MANIFEST" || fail "could not record the manifest"
[[ -s "$MANIFEST" ]] || fail "manifest not written"
pass "manifest: $MANIFEST"

# A record that only exists on the machine that produced it is a file, not a
# ceremony. Each validator signs the manifest with the ed25519 key its GRANDPA
# authority *is*, so the agreement is checkable from the manifest alone.
info "signing the manifest with every validator's GRANDPA key"
python3 "$ROOT_DIR/scripts/testnet/testnet-ceremony.py" sign "$MANIFEST" \
  | sed 's/^/[ceremony-drill]   /' || fail "could not sign the manifest"
SIGNED_THRESHOLD="$(python3 - "$MANIFEST" <<'PY'
import json, sys
attestations = json.load(open(sys.argv[1]))["attestations"]
print(f"{attestations['obtained']}/{attestations['authority_set_size']} (threshold {attestations['required']})")
PY
)"
pass "signed by $SIGNED_THRESHOLD authorities"

info "verifying the running network against the signed manifest"
if ! python3 "$ROOT_DIR/scripts/testnet/testnet-ceremony.py" verify "$MANIFEST" \
    --rpc "$PORTS" --node-bin "$NODE_BIN" --min-finalized 5 | sed 's/^/[ceremony-drill]   /'; then
  fail "the network does not match its own manifest"
fi
pass "every check passes against the launched network, attestations included"

# Negative control: a verifier that has never failed proves nothing.
TAMPERED="$BASE_DIR/ceremony-tampered.json"
python3 - "$MANIFEST" "$TAMPERED" <<'PY'
import json, sys
manifest = json.load(open(sys.argv[1]))
genesis = manifest["genesis"]["hash"]
manifest["genesis"]["hash"] = ("0x" + "00" * 32) if genesis != "0x" + "00" * 32 else ("0x" + "11" * 32)
json.dump(manifest, open(sys.argv[2], "w"), indent=2)
PY
if python3 "$ROOT_DIR/scripts/testnet/testnet-ceremony.py" verify "$TAMPERED" \
    --rpc "$PORTS" --node-bin "$NODE_BIN" --min-finalized 5 >"$BASE_DIR/tampered-verify.log" 2>&1; then
  fail "a manifest with a wrong genesis hash was accepted"
fi
grep -q "genesis hash" "$BASE_DIR/tampered-verify.log" \
  || fail "the tampered manifest was rejected, but not for the genesis hash"
# The signature covers the manifest, so the same edit also breaks the attestation:
# one tamper, two independent refusals.
grep -q "verifies over the manifest" "$BASE_DIR/tampered-verify.log" \
  || fail "the tampered manifest was rejected for the hash but the signature check did not notice"
pass "a tampered manifest is rejected (wrong genesis hash and a signature that no longer covers it)"

# The attestation layer's own refusals, each on a copy of the good manifest.
info "negative controls: the attestation checks must fail when they should"
CONTROLS="$BASE_DIR/controls"
mkdir -p "$CONTROLS"

unsigned_manifest() { # unsigned_manifest <out>
  python3 - "$MANIFEST" "$1" <<'PY'
import json, sys
manifest = json.load(open(sys.argv[1]))
manifest.pop("attestations", None)
json.dump(manifest, open(sys.argv[2], "w"), indent=2)
PY
}

expect_refusal() { # expect_refusal <label> <manifest> <grep-pattern> [extra args...]
  local label="$1" candidate="$2" pattern="$3"
  shift 3
  local log="$CONTROLS/$(basename "$candidate").log"
  if python3 "$ROOT_DIR/scripts/testnet/testnet-ceremony.py" verify "$candidate" \
      --rpc "$PORTS" --node-bin "$NODE_BIN" --min-finalized 5 "$@" >"$log" 2>&1; then
    fail "$label was accepted"
  fi
  grep -q "$pattern" "$log" \
    || { sed 's/^/[ceremony-drill]   /' "$log" >&2; fail "$label was rejected, but not for: $pattern"; }
  pass "$label is rejected ($pattern)"
}

# 1. An unsigned record is not a ceremony.
unsigned_manifest "$CONTROLS/unsigned.json"
expect_refusal "an unsigned manifest" "$CONTROLS/unsigned.json" "carries operator attestations"
# ... and the escape hatch is real and named, so an operator who means it can still check
# a boot-strapping network's technical claims.
if ! python3 "$ROOT_DIR/scripts/testnet/testnet-ceremony.py" verify "$CONTROLS/unsigned.json" \
    --rpc "$PORTS" --node-bin "$NODE_BIN" --min-finalized 5 --allow-unsigned \
    >"$CONTROLS/unsigned-allowed.log" 2>&1; then
  fail "--allow-unsigned did not accept an unsigned manifest"
fi
grep -q "skip  operator attestations" "$CONTROLS/unsigned-allowed.log" \
  || fail "--allow-unsigned accepted the manifest without saying it skipped the signatures"
pass "--allow-unsigned accepts it and says so"

# 2. Fewer signatures than the threshold.
python3 - "$MANIFEST" "$CONTROLS/below-threshold.json" <<'PY'
import json, sys
manifest = json.load(open(sys.argv[1]))
attestations = manifest["attestations"]
# Leave one short of the bar the manifest declares.
keep = max(0, attestations["required"] - 1)
attestations["authorities"] = attestations["authorities"][:keep]
attestations["obtained"] = keep
json.dump(manifest, open(sys.argv[2], "w"), indent=2)
PY
expect_refusal "a manifest below its own threshold" "$CONTROLS/below-threshold.json" \
  "reached the threshold"

# 3. A signature from a key that is not an authority, even though it is a valid
#    ed25519 signature over the very same bytes.
python3 - "$MANIFEST" "$CONTROLS/foreign-signer.json" \
    "$ROOT_DIR/scripts/testnet/testnet-ceremony.py" <<'PY'
import importlib.util, json, sys
spec = importlib.util.spec_from_file_location("ceremony", sys.argv[3])
ceremony = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ceremony)
manifest = json.load(open(sys.argv[1]))
foreign_seed = bytes(range(64, 96))
signature = ceremony.ed25519_sign(foreign_seed, ceremony.canonical_manifest_bytes(manifest))
manifest["attestations"]["authorities"][0] = {
    "index": 0,
    "public_key": "0x" + ceremony.ed25519_pubkey_from_seed(foreign_seed).hex(),
    "signature": "0x" + signature.hex(),
}
json.dump(manifest, open(sys.argv[2], "w"), indent=2)
PY
expect_refusal "a signature from a key outside the authority set" "$CONTROLS/foreign-signer.json" \
  "is an authority key"

# 4. One byte of one signature flipped.
python3 - "$MANIFEST" "$CONTROLS/flipped-signature.json" <<'PY'
import json, sys
manifest = json.load(open(sys.argv[1]))
entry = manifest["attestations"]["authorities"][0]
raw = bytearray.fromhex(entry["signature"].removeprefix("0x"))
raw[-1] ^= 0x01
entry["signature"] = "0x" + raw.hex()
json.dump(manifest, open(sys.argv[2], "w"), indent=2)
PY
expect_refusal "a flipped signature byte" "$CONTROLS/flipped-signature.json" \
  "verifies over the manifest"

# 5. A manifest that declares a lower bar for itself than the authority set implies.
python3 - "$MANIFEST" "$CONTROLS/lowered-threshold.json" <<'PY'
import json, sys
manifest = json.load(open(sys.argv[1]))
manifest["attestations"]["required"] = 1
json.dump(manifest, open(sys.argv[2], "w"), indent=2)
PY
expect_refusal "a manifest that lowers its own threshold" "$CONTROLS/lowered-threshold.json" \
  "workspace rule"

# 6. The same key signing twice, with the manifest's count unchanged.
python3 - "$MANIFEST" "$CONTROLS/duplicate-signer.json" <<'PY'
import json, sys
manifest = json.load(open(sys.argv[1]))
entries = manifest["attestations"]["authorities"]
entries[1] = dict(entries[0], index=entries[1]["index"])
json.dump(manifest, open(sys.argv[2], "w"), indent=2)
PY
expect_refusal "the same authority signing twice" "$CONTROLS/duplicate-signer.json" \
  "distinct signer"

# 7. The ceremony shape itself: `attest` adds one operator's signature to a
#    manifest others have already signed, and the result still verifies.
KEYS_DIR="${KEYS_DIR:-$(dirname "$SPEC")/validator-keys}"
LAST_SEED="$(awk -F= '/^grandpa=/{print $2}' "$KEYS_DIR/validator-$COUNT.suri" | tr -d '[:space:]')"
[[ -n "$LAST_SEED" ]] || fail "could not read validator-$COUNT's seed from $KEYS_DIR"
python3 - "$MANIFEST" "$CONTROLS/partial.json" "$COUNT" <<'PY'
import json, sys
manifest = json.load(open(sys.argv[1]))
entries = manifest["attestations"]["authorities"]
manifest["attestations"]["authorities"] = entries[:-1]
manifest["attestations"]["obtained"] = len(entries) - 1
json.dump(manifest, open(sys.argv[2], "w"), indent=2)
print(f"  left {len(entries) - 1} of {sys.argv[3]} signatures on the copy")
PY
info "attesting the last authority onto a copy that is one signature short"
python3 "$ROOT_DIR/scripts/testnet/testnet-ceremony.py" attest "$CONTROLS/partial.json" \
  --key "$LAST_SEED" | sed 's/^/[ceremony-drill]   /' \
  || fail "attest refused to add a missing authority's signature"
if ! python3 "$ROOT_DIR/scripts/testnet/testnet-ceremony.py" verify "$CONTROLS/partial.json" \
    --rpc "$PORTS" --node-bin "$NODE_BIN" --min-finalized 5 >"$CONTROLS/attested.log" 2>&1; then
  sed 's/^/[ceremony-drill]   /' "$CONTROLS/attested.log" >&2
  fail "the manifest assembled by attest does not verify"
fi
pass "attest assembles the ceremony one operator at a time, and the result verifies"

# 8. Attesting onto a manifest that has been edited since it was signed.
python3 - "$CONTROLS/partial.json" "$CONTROLS/stale.json" <<'PY'
import json, sys
manifest = json.load(open(sys.argv[1]))
entries = manifest["attestations"]["authorities"]
manifest["attestations"]["authorities"] = entries[:-1]
manifest["attestations"]["obtained"] = len(entries) - 1
manifest["chain"] = "a chain nobody signed for"
json.dump(manifest, open(sys.argv[2], "w"), indent=2)
PY
if python3 "$ROOT_DIR/scripts/testnet/testnet-ceremony.py" attest "$CONTROLS/stale.json" \
    --key "$LAST_SEED" >"$CONTROLS/stale.log" 2>&1; then
  fail "attest added a signature to a manifest whose existing signatures are stale"
fi
grep -q "no longer verifies" "$CONTROLS/stale.log" \
  || { sed 's/^/[ceremony-drill]   /' "$CONTROLS/stale.log" >&2; fail "stale attestation was refused without saying why"; }
pass "attest refuses a manifest edited since it was signed (stale signatures)"

cleanup
# `pgrep` exits 1 when nothing matches; under `set -o pipefail` that would abort the
# script before it can report the success it is checking for.
left=$( { pgrep -f -- "--base-path $BASE_DIR/node-" 2>/dev/null || true; } | wc -l | tr -d '[:space:]')
[[ "$left" == "0" ]] || fail "cleanup left ${left} validator process(es) running"
pass "no validator processes left"

printf '\n[ceremony-drill] ALL PHASES PASSED: recorded, verified, and proven able to fail.\n'
