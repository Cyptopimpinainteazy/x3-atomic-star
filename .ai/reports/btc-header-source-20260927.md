# The missing half of the BTC path: a real header source, drilled live

Date: 2026-09-27. Registry row `btc_fortress_gateway`, now **72%** (was 45).

## What was missing

The chain could be *born* anchored on a Bitcoin checkpoint (`spec_version` 14/15) and could
validate and extend a header chain under Bitcoin's rules, and `submit_btc_headers` (call index 35,
`BtcHeaderOrigin`) was the relayer's receiving end. What did not exist was anything that *fetches*
headers from Bitcoin: `.ai/reports/btc-header-source-design-20260922.md` wrote the design, and the
registry blocker said so plainly — "there is no header source and no peer-sourced sync".

## What landed

* `scripts/btc/push-headers.py` — the source. It reads a run of consecutive headers from an
  Esplora HTTP endpoint (`--esplora`, e.g. `https://blockstream.info/testnet/api`) or a local
  `bitcoin-cli` (`--bitcoind-dir` / `X3_BITCOIND_DIR`), checks the run against itself (every
  child's `prev_blockhash` is its parent's hash; every header's 80 bytes hash to the hash the
  source claimed for that height; no gaps), keeps a resumable `--cursor`, and prints the SCALE
  encoding of `Vec<BtcBlockHeader>` — exactly the bytes `submit_btc_headers` decodes — plus the
  height/hash it reached. It needs no signer, so it is testable on its own; the signing half stays
  with `scripts/btc/push-headers.mjs`.
* `scripts/testnet/btc-header-source-drill.sh` (+ `.py`) — the live drill. It follows a real public
  testnet endpoint, prints every height/hash/link it verified, checks the payload's shape, and then
  runs the fetcher against a **local proxy that forwards to the same endpoint and tampers one
  answer** — a `gap` (a height that is a later block's hash, caught by the link check) and a
  `disagree` (real bytes under a hash they do not produce, caught by the self-consistency check).
  Both must exit non-zero with a message naming the height. A tampered source is never a canned
  header set: the proxy resolves the same real window the fetcher would.
* Two pallet tests (`pallets/x3-settlement-engine/src/tests.rs`) that read the payload the drill
  produced from a real testnet3 run — checked in at
  `pallets/x3-settlement-engine/src/tests/data/btc_relayer_testnet_headers.txt` (heights
  5151279..5151284, anchored on 5151278) — decode it with the pallet's own `BtcBlockHeader`, anchor
  the real parent header as a genesis checkpoint, and require the whole batch to be admitted in
  order. A second test drops the first header and requires `BtcParentMissing`. This is the link
  that makes the drill evidence rather than a script that prints JSON.

## Live evidence

`bash scripts/testnet/btc-header-source-drill.sh` — PASS (log: `.ai/runlogs/btc-header-source-20260927T132225Z/drill.txt`):

```
[btc-source-drill] https://blockstream.info/testnet/api unusable: … HTTP Error 429: Too Many Requests
[btc-source-drill] endpoint https://mempool.space/testnet/api
[btc-source-drill] tip height 5151296
    anchor 5151283 0000000000e2ff61dc4b66424ae6ab22c043710e9230ef8a0ca5bdda4fe41b34
    height 5151284 0000000000322d6fdb54c3c7b58bc33379b85ae100f2a5cc198bc2712a231b87 prev→5151283 link-ok
    … through height 5151289 …
  PASS  gap at height 5151286 is refused non-zero and names it
  PASS  a source disagreeing with itself at height 5151286 is refused and names it
  PASS  a refused run emits no payload
[btc-source-drill] PASS — the relayer followed a real public chain and refused a lie
```

`blockstream.info` was rate-limiting this host (`429`) at run time, so the drill fell through to
`mempool.space/testnet` — the same testnet3 chain, and the drill prints which endpoint it used.
The drill is a loud **skip** (`exit 2`) when no endpoint answers, never a pass.

`cargo test -p pallet-x3-settlement-engine` — 160 lib tests + 23 integration, all passing; the two
new tests are in `.ai/runlogs/btc-header-source-20260927T132225Z/pallet-tests.txt`.

## Break-it-first

* Removed the link check from `push-headers.py` (`fetch_run`): the drill went **red** — "FAIL gap
  at height 5151286 is refused non-zero and names it — exit 0". Restored byte-identically
  (`sha256 0339995c…`) → **green**.
* Moved the fixture's `anchor_height` from 5151278 to 5151277: the pallet test went **red** —
  "assertion `left == right` failed: no gaps in the run (left 5151279, right 5151278)". Restored
  byte-identically (`sha256 6478350f…`) → **green**.

## A real finding, recorded rather than papered over

The pallet's difficulty rule is **mainnet's**: `btc_bits_follow_parent` requires a child to carry
its parent's `nBits` off a retarget boundary. Testnet3 mines "minimum difficulty" blocks whenever
twenty minutes pass, so its `nBits` alternates *mid-epoch* (observed: `0x1a109630` and
`0x1d00ffff` alternating at heights 5151277/5151285). The pallet would refuse those legitimate
headers, so the relayer's fixture uses a uniform-`nBits` window and the registry now carries this
as an open blocker: tracking testnet as it actually mines needs the network's own rule modelled.
This is a liveness limitation on the pallet, not a reason to weaken it.

## What this does and does not prove

* **Proved:** the relayer reads real public-network headers, verifies the run links, emits the
  pallet's exact payload, refuses a gap and a source that lies, and the pallet admits that payload
  in order on a checkpoint-anchored chain (mock runtime).
* **Not proved:** any *live chain* has admitted a public-network header. No public chain has a
  checkpoint pinned, the push still needs a root origin (`BtcHeaderOrigin = EnsureRoot`), the
  `bitcoin-cli` source is exercised only by its code path (no Bitcoin Core on this host), no coin has
  moved, nothing bonds the relayer for withholding, and there is no audit. `mainnet_ready` stays 50
  for exactly those reasons — a public-network chain run is what would lift it.
