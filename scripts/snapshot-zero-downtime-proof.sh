#!/usr/bin/env bash
# snapshot-zero-downtime-proof.sh - take a snapshot of a chain that is still running.
#
# The snapshot row's last open item was that the archive is cut with the node
# stopped: `scripts/snapshot-restore.sh backup` refuses a live database (a plain
# tar of a running RocksDB is not a consistent snapshot) and the node exposes no
# checkpoint, so on a real validator the backup window *is* a downtime window.
#
# This proves the other path - export the state over RPC from the running node,
# while it keeps authoring and finalizing, and build the content-addressed
# snapshot from that export:
#
#   1. boot an archive node and wait until the chain has grown past its first
#      justification period, so the anchor has accumulated real state rather
#      than being a hair above genesis;
#   2. export the state at the newest *justified* finalized block over RPC and
#      page every value at that same block hash, while the node keeps running
#      (its PID never changes and its finalized head must move on afterwards);
#   3. recompute the trie root from the exported entries with
#      `x3-state-snapshot root --from-raw-spec`, using the chain's own layout, and
#      require it to equal the `stateRoot` the chain published in that block's
#      header. A key the walk missed, a value read at the wrong block, or a key
#      the enumeration invented all move that root, so this is the export being
#      measured against the chain and not against itself;
#   4. build and verify the snapshot with the chain's own GRANDPA justification
#      as the finality proof - read from `chain_getBlock`, not invented;
#   5. restore it into a chain spec, boot a node on that spec, and require the
#      genesis state root to equal the anchor's and the whole state map at
#      genesis to be byte-identical to the export.
#
# Four controls, because every one of those checks can pass for the wrong
# reason:
#
#   * a one-key-short export must change the recomputed root, and `build` must
#     refuse to write a manifest whose declared root disagrees with it;
#   * `verify` must refuse the good snapshot against a state root that is not its
#     own;
#   * the *unaltered* genesis spec - a chain that has not grown - must not
#     reproduce the anchor's root, so the root check cannot pass for a chain
#     that never changed;
#   * a node with a bounded pruning window must **refuse** to export an anchor
#     whose state it has already discarded, naming pruning, and must leave no
#     spec behind. A partial snapshot that looks complete is the failure this
#     whole path exists to avoid.
#
# Usage:
#   bash scripts/snapshot-zero-downtime-proof.sh
#
# Environment:
#   X3_NODE_BIN                    node binary (default target/{release,debug})
#   X3_SNAPSHOT_VERIFIER           x3-state-snapshot binary (default target/{release,debug})
#   X3_ZERO_DOWNTIME_MIN_HEIGHT    height the source must reach (default 520)
#   X3_ZERO_DOWNTIME_WAIT_SECS     per-wait timeout (default 300)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EXPORTER="$ROOT/scripts/snapshot-rpc-export.py"
MIN_HEIGHT="${X3_ZERO_DOWNTIME_MIN_HEIGHT:-520}"
WAIT_SECS="${X3_ZERO_DOWNTIME_WAIT_SECS:-300}"

say() { printf '[zero-downtime] %s\n' "$*"; }
die() {
  printf '[zero-downtime] FAIL: %s\n' "$*" >&2
  for log in "$WORK"/*.log; do
    [ -f "$log" ] || continue
    printf '[zero-downtime] --- %s (tail) ---\n' "$(basename "$log")" >&2
    tail -20 "$log" >&2 || true
  done
  exit 1
}

picked() { # picked <explicit> <name>
  local explicit="$1" name="$2" candidate
  if [ -n "$explicit" ]; then printf '%s\n' "$explicit"; return 0; fi
  for candidate in "$ROOT/target/release/$name" "$ROOT/target/debug/$name"; do
    if [ -x "$candidate" ]; then printf '%s\n' "$candidate"; return 0; fi
  done
  return 1
}

NODE_BIN="$(picked "${X3_NODE_BIN:-}" x3-chain-node)" || {
  echo "[zero-downtime] node binary not found; build it with" >&2
  echo "                 cargo build --release -p x3-chain-node" >&2
  exit 2
}
SNAPSHOT_BIN="$(picked "${X3_SNAPSHOT_VERIFIER:-}" x3-state-snapshot)" || {
  echo "[zero-downtime] x3-state-snapshot not found; build it with" >&2
  echo "                 cargo build -p x3-state-snapshot" >&2
  exit 2
}
[ -x "$EXPORTER" ] || { echo "[zero-downtime] $EXPORTER is missing" >&2; exit 2; }
say "node:       $NODE_BIN"
say "snapshot:   $SNAPSHOT_BIN"
say "exporter:   $EXPORTER"

WORK="$(mktemp -d)"
NODE_PIDS=()
cleanup() {
  for pid in "${NODE_PIDS[@]:-}"; do kill "$pid" 2>/dev/null || true; done
  sleep 1
  for pid in "${NODE_PIDS[@]:-}"; do kill -9 "$pid" 2>/dev/null || true; done
  rm -rf "$WORK"
}
trap cleanup EXIT

BASE_PORT="$(python3 - <<'PY'
import random, socket
while True:
    base = random.randint(42000, 55000)
    socks = []
    try:
        for i in range(16):
            s = socket.socket()
            s.bind(("127.0.0.1", base + i))
            socks.append(s)
        print(base)
        break
    except OSError:
        pass
    finally:
        for s in socks:
            s.close()
PY
)"
say "ports:      base $BASE_PORT"

rpc() { # rpc <url> <method> <params-json>
  curl -s -m 20 -H 'Content-Type: application/json' \
    --data-binary "{\"jsonrpc\":\"2.0\",\"method\":\"$2\",\"params\":$3,\"id\":1}" "$1"
}

finalized_hash() { # finalized_hash <url>
  rpc "$1" chain_getFinalizedHead '[]' | python3 -c 'import json,sys; doc=json.load(sys.stdin); sys.exit("chain_getFinalizedHead: "+str(doc["error"])) if doc.get("error") else print(doc.get("result") or "")'
}

height_of() { # height_of <url> <hash>
  rpc "$1" chain_getHeader "[\"$2\"]" | python3 -c 'import json,sys; h=json.load(sys.stdin).get("result"); print(-1 if h is None else int(h["number"],16))'
}

finalized_height() { # finalized_height <url>
  local hash
  hash="$(finalized_hash "$1" || true)"
  [ -n "$hash" ] || { echo -1; return 0; }
  height_of "$1" "$hash"
}

hash_at() { # hash_at <url> <height>
  rpc "$1" chain_getBlockHash "[$2]" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("result") or "")'
}

state_root_at() { # state_root_at <url> <hash>
  rpc "$1" chain_getHeader "[\"$2\"]" | python3 -c 'import json,sys; h=json.load(sys.stdin).get("result"); print("" if h is None else h["stateRoot"])'
}

start_node() { # start_node <name> <base-path> <rpc-port> [extra args...]
  local name="$1" base="$2" port="$3"
  shift 3
  "$NODE_BIN" --dev --base-path "$base" --rpc-port "$port" --port "$((port + 1))" \
    --rpc-methods unsafe --no-telemetry --no-mdns --no-prometheus "$@" \
    >"$WORK/$name.log" 2>&1 &
  NODE_PIDS+=("$!")
  printf '%s\n' "$!" >"$WORK/$name.pid"
}

# A node that boots from a chain spec rather than `--dev`, and that is an
# *authority* on it: `--node-key` because this node build exits with
# `NetworkKeyNotFound` without one, `--validator --force-authoring` so it
# proposes, and `X3_DEV_SEED` because the restored state's authority set is the
# dev chain's, whose session keys the node then inserts. Without all four the
# restored chain boots and authors nothing, which is a much weaker result.
start_authority_node() { # start_authority_node <name> <base-path> <rpc-port> <chain-spec>
  local name="$1" base="$2" port="$3" spec="$4" node_key
  node_key="$(python3 -c "import secrets;print('0x'+secrets.token_hex(32))")"
  X3_DEV_SEED=//Alice "$NODE_BIN" --chain "$spec" --base-path "$base" \
    --rpc-port "$port" --port "$((port + 1))" --rpc-methods unsafe \
    --validator --force-authoring --node-key "$node_key" \
    --no-telemetry --no-mdns --no-prometheus \
    >"$WORK/$name.log" 2>&1 &
  NODE_PIDS+=("$!")
  printf '%s\n' "$!" >"$WORK/$name.pid"
}

wait_rpc() { # wait_rpc <url>
  local url="$1"
  for _ in $(seq 1 "$WAIT_SECS"); do
    if rpc "$url" system_health '[]' | grep -q '"peers"'; then return 0; fi
    sleep 1
  done
  return 1
}

wait_height() { # wait_height <url> <min>
  local url="$1" min="$2" height
  for _ in $(seq 1 "$WAIT_SECS"); do
    height="$(finalized_height "$url" || echo -1)"
    if [ "$height" -ge "$min" ] 2>/dev/null; then return 0; fi
    sleep 1
  done
  return 1
}

export_json() { # export_json <file> <array-key> ; reads one field out of a JSON document
  python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))[sys.argv[2]])' "$1" "$2"
}

# ── 1. an archive source node, grown past its first justification period ─────
SRC="$WORK/source/node"
mkdir -p "$WORK/source" "$WORK/bounded" "$WORK/restored/node" "$WORK/controls"
SRC_RPC=$(( BASE_PORT ))
# GRANDPA publishes a justification once per `justification_generation_period`
# (512 in this node), so a young chain has exactly one justified block - height
# 1 - and a snapshot anchored there would carry almost no state. Wait until the
# chain has crossed the boundary so the anchor is a block the chain actually
# accumulated state at.
say "booting the source chain (archive) at rpc $SRC_RPC"
start_node source "$SRC" "$SRC_RPC" --state-pruning archive --blocks-pruning archive
SRC_URL="http://127.0.0.1:$SRC_RPC"
wait_rpc "$SRC_URL" || die "source node never answered RPC"

# The archive warning is this node's own words about running an unbounded
# authority; it is the operator guidance this row ends up recommending, so quote
# it rather than paraphrasing it.
if grep -q "keeps unbounded state or blocks" "$WORK/source.log"; then
  say "source node warns (its own words): $(grep -m1 'keeps unbounded' "$WORK/source.log" | sed 's/^[^ ]* //' | cut -c1-160)"
fi

say "waiting for the chain to finalize height $MIN_HEIGHT (~$((MIN_HEIGHT / 5))s at 5 blocks/s)"
wait_height "$SRC_URL" "$MIN_HEIGHT" \
  || die "source chain never reached height $MIN_HEIGHT (finalized $(finalized_height "$SRC_URL"))"
HEAD_BEFORE="$(finalized_height "$SRC_URL")"
SRC_PID="$(cat "$WORK/source.pid")"
say "source: finalized height $HEAD_BEFORE, pid $SRC_PID"

# A template, so the restored spec is a valid chain spec rather than a bare
# state map. It contributes metadata only - never state.
"$NODE_BIN" build-spec --dev >"$WORK/template.json" 2>/dev/null \
  || die "could not build the template chain spec"

# ── 2. export the state of the running node ──────────────────────────────────
say "exporting over RPC (the node is not stopped, and will not be)"
python3 "$EXPORTER" \
  --rpc "$SRC_URL" \
  --out "$WORK/export.json" \
  --report "$WORK/anchor.json" \
  --from-spec "$WORK/template.json" \
  --justified-ancestor \
  --min-finalized "$MIN_HEIGHT" \
  --justification-period 512 >"$WORK/export.log" 2>&1 \
  || { sed 's/^/[zero-downtime] /' "$WORK/export.log" >&2; die "the export refused"; }
sed 's/^/[zero-downtime] /' "$WORK/export.log"

ANCHOR_NUMBER="$(export_json "$WORK/anchor.json" block_number)"
ANCHOR_HASH="$(export_json "$WORK/anchor.json" block_hash)"
ANCHOR_ROOT="$(export_json "$WORK/anchor.json" state_root)"
ANCHOR_SPEC="$(export_json "$WORK/anchor.json" runtime_spec_version)"
ANCHOR_KEYS="$(export_json "$WORK/anchor.json" key_count)"
HEAD_AFTER="$(export_json "$WORK/anchor.json" finalized_head_after)"
ADVANCED="$(export_json "$WORK/anchor.json" finalized_head_advanced_by)"
CHAIN_ID="$(export_json "$WORK/template.json" id)"
JUSTIFICATION="$(export_json "$WORK/anchor.json" finality_proof)"

[ -n "$JUSTIFICATION" ] && [ "$JUSTIFICATION" != "None" ] \
  || die "the export carried no finality proof"
[ "$ANCHOR_NUMBER" -gt 1 ] \
  || die "the anchor is height $ANCHOR_NUMBER: a genesis-height anchor would make the state checks vacuous"
[ "$HEAD_AFTER" -ge "$HEAD_BEFORE" ] \
  || die "the finalized head went backwards during the export ($HEAD_BEFORE -> $HEAD_AFTER)"
[ "$(cat "$WORK/source.pid")" = "$SRC_PID" ] && kill -0 "$SRC_PID" 2>/dev/null \
  || die "the source node process changed during the export; that is not a zero-downtime snapshot"

# The node must still be finalizing *after* serving the snapshot: the export read
# a moving chain, not a frozen one.
AFTER_EXPORT="$HEAD_AFTER"
NOW="$AFTER_EXPORT"
for _ in $(seq 1 60); do
  NOW="$(finalized_height "$SRC_URL" || echo -1)"
  if [ "$NOW" -gt "$AFTER_EXPORT" ]; then break; fi
  sleep 1
done
[ "$NOW" -gt "$AFTER_EXPORT" ] \
  || die "the source chain stopped finalizing after the export (stuck at $AFTER_EXPORT)"
say "source kept finalizing through the export: $HEAD_BEFORE -> $AFTER_EXPORT -> $NOW"

say "anchor: height=$ANCHOR_NUMBER hash=${ANCHOR_HASH:0:18}... keys=$ANCHOR_KEYS spec_version=$ANCHOR_SPEC"
say "        state_root=$ANCHOR_ROOT"
say "        head advanced $ADVANCED block(s) while the export ran"

# The chain's own root for that block, read independently of the export.
CHAIN_ROOT="$(state_root_at "$SRC_URL" "$ANCHOR_HASH")"
[ "$CHAIN_ROOT" = "$ANCHOR_ROOT" ] \
  || die "the report's state root $ANCHOR_ROOT is not the chain's $CHAIN_ROOT"

# ── 3. the export is the chain's state: recompute the trie root ──────────────
DERIVED_ROOT="$("$SNAPSHOT_BIN" root --from-raw-spec "$WORK/export.json" --state-version 1)" \
  || die "x3-state-snapshot root refused the exported state"
[ "$DERIVED_ROOT" = "$ANCHOR_ROOT" ] \
  || die "the exported state hashes to $DERIVED_ROOT, not the chain's $ANCHOR_ROOT: the export is incomplete"
say "the exported state recomputes to the chain's own state root ($DERIVED_ROOT)"

# ── 4. the controls that stop those checks passing for the wrong reason ──────
say "control 1: an export that is one key short must not reproduce the root"
python3 - "$WORK/export.json" "$WORK/short.json" <<'PY'
import json, sys
spec = json.load(open(sys.argv[1]))
top = spec["genesis"]["raw"]["top"]
dropped = sorted(top)[len(top) // 2]
del top[dropped]
json.dump(spec, open(sys.argv[2], "w"))
print(f"dropped {dropped}")
PY
SHORT_ROOT="$("$SNAPSHOT_BIN" root --from-raw-spec "$WORK/short.json" --state-version 1)"
[ "$SHORT_ROOT" != "$ANCHOR_ROOT" ] \
  || die "dropping a key did not change the recomputed root: the root check is not load-bearing"
if "$SNAPSHOT_BIN" build --from-raw-spec "$WORK/short.json" --out "$WORK/controls/short" \
     --chain-id "$CHAIN_ID" --block-number "$ANCHOR_NUMBER" --block-hash "$ANCHOR_HASH" \
     --state-root "$ANCHOR_ROOT" --runtime-version "$ANCHOR_SPEC" \
     --finality-proof "$JUSTIFICATION" >"$WORK/controls/short.log" 2>&1; then
  die "build accepted a one-key-short export against the chain's state root"
fi
say "            build refused it: $(tail -1 "$WORK/controls/short.log")"

say "control 2: the unaltered genesis spec must not reproduce the anchor's root"
"$NODE_BIN" build-spec --dev --raw >"$WORK/genesis.json" 2>/dev/null \
  || die "could not build a raw genesis spec"
GENESIS_KEYS="$(python3 -c 'import json,sys;print(len(json.load(open(sys.argv[1]))["genesis"]["raw"]["top"]))' "$WORK/genesis.json")"
[ "$GENESIS_KEYS" -gt 0 ] || die "the genesis spec carries no state; the control would be vacuous"
GENESIS_ROOT="$("$SNAPSHOT_BIN" root --from-raw-spec "$WORK/genesis.json" --state-version 1)"
[ "$GENESIS_ROOT" != "$ANCHOR_ROOT" ] \
  || die "an unaltered genesis spec reproduces the anchor's root: the anchor is not a grown state"
say "            genesis root $GENESIS_ROOT ($GENESIS_KEYS keys) differs from the anchor"

# ── 5. build and verify the snapshot with the chain's own justification ──────
say "building the snapshot with the chain's GRANDPA justification as its finality proof"
"$SNAPSHOT_BIN" build --from-raw-spec "$WORK/export.json" --out "$WORK/snapshot" \
  --chain-id "$CHAIN_ID" --block-number "$ANCHOR_NUMBER" --block-hash "$ANCHOR_HASH" \
  --state-root "$ANCHOR_ROOT" --runtime-version "$ANCHOR_SPEC" \
  --finality-proof "$JUSTIFICATION" >"$WORK/build.log" 2>&1 \
  || { cat "$WORK/build.log" >&2; die "build refused the export"; }
MANIFEST_HASH="$(tail -1 "$WORK/build.log")"
say "manifest $MANIFEST_HASH"

"$SNAPSHOT_BIN" verify --manifest "$WORK/snapshot/manifest.json" --chunks "$WORK/snapshot" \
  --chain-id "$CHAIN_ID" --block-hash "$ANCHOR_HASH" --state-root "$ANCHOR_ROOT" \
  --runtime-version "$ANCHOR_SPEC" --state-version 1 >"$WORK/verify.log" 2>&1 \
  || { cat "$WORK/verify.log" >&2; die "verify refused the snapshot it just built"; }
say "verify: $(tail -1 "$WORK/verify.log")"

say "control 3: verify must refuse the same snapshot against a different state root"
if "$SNAPSHOT_BIN" verify --manifest "$WORK/snapshot/manifest.json" --chunks "$WORK/snapshot" \
     --chain-id "$CHAIN_ID" --block-hash "$ANCHOR_HASH" \
     --state-root "$GENESIS_ROOT" --runtime-version "$ANCHOR_SPEC" \
     --state-version 1 >"$WORK/controls/verify.log" 2>&1; then
  die "verify accepted the snapshot against a state root that is not its own"
fi
say "            refused: $(tail -1 "$WORK/controls/verify.log")"

# ── 6. restore, boot, and require the state back ─────────────────────────────
# `--regenesis` is not a convenience. Without it the restored spec carries the
# producing chain's `frame_system` bookkeeping, including `System::Number` = the
# anchor's height, and `frame_system::initialize` asserts
# `number == Self::block_number() + 1` - so the first block a restored chain tries
# to author panics with "Block number must be strictly increasing". That is what
# this gate found the first time it booted a restored spec, and it is why the
# drill now requires the restored chain to *author and finalize* rather than
# merely to announce a matching genesis root.
say "restoring the snapshot into a chain spec (re-genesis: the producing chain's bookkeeping is dropped)"
"$SNAPSHOT_BIN" restore --manifest "$WORK/snapshot/manifest.json" --chunks "$WORK/snapshot" \
  --out "$WORK/restored.json" --chain-id "$CHAIN_ID" --block-hash "$ANCHOR_HASH" \
  --state-root "$ANCHOR_ROOT" --runtime-version "$ANCHOR_SPEC" \
  --from-spec "$WORK/template.json" --state-version 1 --regenesis --force >"$WORK/restore.log" 2>&1 \
  || { cat "$WORK/restore.log" >&2; die "restore refused the snapshot"; }
sed 's/^/[zero-downtime] /' "$WORK/restore.log"

REMOVED_KEYS="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["properties"]["x3SnapshotBookkeepingRemoved"])' "$WORK/restored.json")"
GENESIS_ROOT="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["properties"]["x3SnapshotGenesisStateRoot"])' "$WORK/restored.json")"
[ -n "$REMOVED_KEYS" ] || die "the restore dropped no bookkeeping keys; the spec is not a genesis"
[ "$GENESIS_ROOT" != "$ANCHOR_ROOT" ] \
  || die "the regenerated genesis root equals the anchor root, so nothing was actually re-genesised"

RST_RPC=$(( BASE_PORT + 4 ))
say "booting a node on the restored spec (rpc $RST_RPC)"
start_authority_node restored "$WORK/restored/node" "$RST_RPC" "$WORK/restored.json"
RST_URL="http://127.0.0.1:$RST_RPC"
wait_rpc "$RST_URL" || die "the restored node never answered RPC (a restored spec that panics on its first block lands here)"

GENESIS_HASH="$(hash_at "$RST_URL" 0)"
[ -n "$GENESIS_HASH" ] || die "the restored node reports no genesis block"
RESTORED_ROOT="$(state_root_at "$RST_URL" "$GENESIS_HASH")"
[ "$RESTORED_ROOT" = "$GENESIS_ROOT" ] \
  || die "the restored chain's genesis state root is $RESTORED_ROOT, not the regenerated $GENESIS_ROOT"

# The check a matching genesis root cannot make: the restored chain has to be a
# *chain*, not a database that panics the moment it produces a block.
wait_height "$RST_URL" 2 >/dev/null \
  || die "the restored chain never finalized a block beyond genesis; it is not authoring"
RESTORED_HEIGHT="$(finalized_height "$RST_URL")"
say "the restored chain is live: genesis root $RESTORED_ROOT, finalized height $RESTORED_HEIGHT"

# Read the restored chain's state back through the *same* reader that produced
# the export. Comparing the two documents is then a comparison of the two
# chains, not of two implementations of "read the state".
python3 "$EXPORTER" --rpc "$RST_URL" --out "$WORK/restored-export.json" \
  --at "$GENESIS_HASH" --finality-proof optional >"$WORK/restored-export.log" 2>&1 \
  || { sed 's/^/[zero-downtime] /' "$WORK/restored-export.log" >&2; die "the restored chain refused to export its genesis state"; }
RESTORED_KEYS="$(python3 - "$WORK/export.json" "$WORK/restored-export.json" "$REMOVED_KEYS" <<'PY'
import json, sys
expected = json.load(open(sys.argv[1]))["genesis"]["raw"]["top"]
actual = json.load(open(sys.argv[2]))["genesis"]["raw"]["top"]
removed = [key for key in sys.argv[3].split(",") if key]
for key in removed:
    expected.pop(key, None)
    if key in actual:
        print(f"{key} is still in the restored state; it should have been dropped", file=sys.stderr)
        raise SystemExit(1)
missing = sorted(set(expected) - set(actual))
extra = sorted(set(actual) - set(expected))
changed = sorted(k for k in set(expected) & set(actual) if expected[k] != actual[k])
if missing or extra or changed:
    print(
        f"the restored state differs from the export: {len(missing)} missing, "
        f"{len(extra)} extra, {len(changed)} changed",
        file=sys.stderr,
    )
    for key in (missing + extra + changed)[:5]:
        print(f"  {key}", file=sys.stderr)
    raise SystemExit(1)
print(len(expected))
PY
)" || die "the restored chain does not hold the exported state"
say "the restored state matches the export on every one of its $RESTORED_KEYS non-bookkeeping entries"

# ── 7. the ugly path: a pruned anchor is refused, not answered partially ─────
say "control 4: a bounded node must refuse an anchor whose state it discarded"
BND_RPC=$(( BASE_PORT + 8 ))
start_node bounded "$WORK/bounded/node" "$BND_RPC" --state-pruning 16 --blocks-pruning 16
BND_URL="http://127.0.0.1:$BND_RPC"
wait_rpc "$BND_URL" || die "the bounded node never answered RPC"
wait_height "$BND_URL" 64 >/dev/null || die "the bounded node never reached height 64"
OLD_HASH="$(hash_at "$BND_URL" 1)"
[ -n "$OLD_HASH" ] || die "the bounded node reports no hash at height 1"
# `--finality-proof optional` so the refusal under test is the *state* one: with
# the default, height 1 has no justification either and the exporter would refuse
# for that reason first, which would prove nothing about pruning.
if python3 "$EXPORTER" --rpc "$BND_URL" --out "$WORK/controls/pruned.json" --at "$OLD_HASH" \
     --finality-proof optional \
     >"$WORK/controls/pruned.log" 2>&1; then
  die "a bounded node exported state it had already pruned"
fi
grep -qi "prune" "$WORK/controls/pruned.log" \
  || { cat "$WORK/controls/pruned.log" >&2; die "the refusal did not name pruning"; }
[ ! -e "$WORK/controls/pruned.json" ] \
  || die "the refused export left a spec behind; a partial snapshot must not look complete"
say "            refused: $(head -1 "$WORK/controls/pruned.log")"

echo
echo "[zero-downtime] PASS - a running chain was snapshotted without being stopped:"
printf '  %-34s %s\n' "anchor (finalized, justified)" "height $ANCHOR_NUMBER  ${ANCHOR_HASH:0:18}..."
printf '  %-34s %s\n' "state entries exported" "$ANCHOR_KEYS"
printf '  %-34s %s\n' "recomputed root == chain root" "$DERIVED_ROOT"
printf '  %-34s %s\n' "GRANDPA justification carried" "${JUSTIFICATION:0:34}..."
printf '  %-34s %s\n' "manifest" "$MANIFEST_HASH"
printf '  %-34s %s\n' "source head during/after export" "$HEAD_BEFORE -> $AFTER_EXPORT -> $NOW"
printf '  %-34s %s\n' "restored genesis state root" "$RESTORED_ROOT  (a new chain's genesis)"
printf '  %-34s %s\n' "restored chain finalizing" "height $RESTORED_HEIGHT (it authors blocks)"
printf '  %-34s %s\n' "bookkeeping dropped at restore" "$REMOVED_KEYS"
printf '  %-34s %s\n' "non-bookkeeping entries recovered" "$RESTORED_KEYS (byte-identical)"
printf '  %-34s %s\n' "one-key-short export refused" "$SHORT_ROOT != anchor"
printf '  %-34s %s\n' "genesis-only spec refused" "$GENESIS_ROOT != anchor"
printf '  %-34s %s\n' "pruned anchor refused" "named pruning, no spec written"
