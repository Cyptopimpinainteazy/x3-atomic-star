#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# testnet_genesis_lint.sh — structural lint for a testnet chain spec.
#
# This check used to require that a `chain-specs/` directory existed, print a
# hand-off line saying the real validation was still to be integrated, and exit 0.
# Both RC gates ran it as their genesis check, so the only thing standing behind
# "genesis lint passed" was the presence of a directory.
#
# It now lints a real artifact and refuses when there is none — an absent spec is
# not a passing lint:
#
#   * the spec parses as a JSON object with an identity and a chain type;
#   * its genesis is in a form the node can load (plain config, or raw);
#   * the Aura and GRANDPA authority sets exist and agree in size, and the
#     GRANDPA entries are `[address, weight]` pairs;
#   * a Live spec publishes at least one joinable bootnode multiaddr;
#   * and finally the node's own loader accepts it (`build-spec --chain <spec>`),
#     because the checks above cannot see a config key the runtime refuses.
#
# Usage: scripts/testnet/testnet_genesis_lint.sh [spec.json]
#        (default: the newest chain-specs/*-plain.json)
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

fail() { echo "FAILED: $1" >&2; exit 1; }

spec="${1:-}"
if [ -z "$spec" ]; then
  shopt -s nullglob
  candidates=("$ROOT_DIR"/chain-specs/*-plain.json)
  shopt -u nullglob
  if [ "${#candidates[@]}" -eq 0 ]; then
    fail "no chain spec to lint — run scripts/testnet/generate_testnet_chain_spec.sh first, or pass a spec path"
  fi
  spec="$(ls -1t "${candidates[@]}" | head -1)"
fi
[ -f "$spec" ] || fail "chain spec not found: $spec"

echo "== X3 Testnet Genesis Lint =="
echo "spec: $spec"

python3 - "$spec" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, encoding="utf-8") as handle:
    spec = json.load(handle)


def need(condition, message):
    if not condition:
        print(f"FAILED: {message}", file=sys.stderr)
        sys.exit(1)


need(isinstance(spec, dict), "the spec is not a JSON object")
for key in ("name", "id", "chainType", "genesis"):
    need(spec.get(key), f"the spec has no `{key}`")

chain_type = spec["chainType"]
need(
    chain_type in ("Live", "Local", "Development", "Custom"),
    f"unexpected chainType {chain_type!r}",
)

genesis = spec["genesis"]
config = (genesis.get("runtimeGenesis") or {}).get("config")
if config is None:
    need("raw" in genesis, "neither genesis.runtimeGenesis.config (plain) nor genesis.raw is present")
    print("  genesis form: raw (structure not walked)")
else:
    aura = (config.get("aura") or {}).get("authorities") or []
    grandpa = (config.get("grandpa") or {}).get("authorities") or []
    need(len(aura) > 0, "the spec names no Aura authority, which a Live chain spec the node refuses")
    need(len(grandpa) > 0, "the spec names no GRANDPA authority")
    need(
        len(aura) == len(grandpa),
        f"Aura authorities ({len(aura)}) and GRANDPA authorities ({len(grandpa)}) disagree",
    )
    for index, authority in enumerate(aura):
        need(
            isinstance(authority, str) and len(authority) in (47, 48),
            f"Aura authority {index} is not an SS58 address: {authority!r}",
        )
    for index, entry in enumerate(grandpa):
        need(
            isinstance(entry, list) and len(entry) == 2 and isinstance(entry[1], int),
            f"GRANDPA authority {index} is not an [address, weight] pair: {entry!r}",
        )
    print(f"  chainType={chain_type} id={spec['id']} authorities={len(aura)}")

if chain_type == "Live":
    bootnodes = spec.get("bootNodes") or []
    need(len(bootnodes) > 0, "a Live spec with no bootNodes cannot be joined")
    for index, node in enumerate(bootnodes):
        need(
            isinstance(node, str) and "/p2p/" in node,
            f"bootNode {index} is not a multiaddr carrying a peer id: {node!r}",
        )
    print(f"  bootNodes={len(bootnodes)}")

print("  structural checks passed")
PY

# The node's own loader is the authority on whether a spec is loadable.
node_bin="${X3_NODE_BIN:-}"
if [ -z "$node_bin" ]; then
  for candidate in "$ROOT_DIR/target/release/x3-chain-node" "$ROOT_DIR/target/debug/x3-chain-node"; do
    if [ -x "$candidate" ]; then
      node_bin="$candidate"
      break
    fi
  done
fi
if [ -z "$node_bin" ] || [ ! -x "$node_bin" ]; then
  fail "no node binary to load-test $spec with — build x3-chain-node or set X3_NODE_BIN (an unloaded spec is not a passing lint)"
fi

if ! "$node_bin" build-spec --chain "$spec" --disable-log-color >/dev/null; then
  fail "$node_bin refused $spec"
fi
echo "  the node's own loader accepted the spec"

echo "== X3 Testnet Genesis Lint PASSED =="
