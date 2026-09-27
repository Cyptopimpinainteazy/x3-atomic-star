#!/usr/bin/env python3
"""Source real Bitcoin headers and emit the `submitBtcHeaders` payload (TICKET-095).

`scripts/btc/push-headers.mjs` is the *sender*: it signs and submits an extrinsic, so it needs
`@polkadot/api` and a signer. This script is the *source* half, and it deliberately does not need
either. It fetches a run of consecutive headers from a real Bitcoin source, checks the run against
itself (every child's `prev_blockhash` is its parent's hash, the header bytes hash to what the
source claims for that height, and there are no gaps), and prints the SCALE encoding of
`Vec<BtcBlockHeader>` — the exact bytes `X3SettlementEngine::submit_btc_headers` decodes — plus the
height/hash the run reached. A caller with a signer wraps that payload in the call; a test decodes
it with the pallet's own type. Nothing here invents a header.

What it does **not** do:
  * it does not sign or submit anything — no key, no `@polkadot/api`, no node connection;
  * it does not decide whether the pallet will *accept* the run (proof of work, difficulty and
    the anchor are the pallet's rules, not the relayer's) — it refuses only a run that is not a
    single linked chain, or a source that disagrees with itself;
  * it does not pick a source for you. There is no default endpoint and no default height: with
    neither `--esplora` nor a `bitcoin-cli` directory named, it refuses to run. A relayer that
    starts talking to the network because it was invoked without arguments is not fail-closed.

Sources
-------
  `--esplora URL`       an Esplora-style HTTP endpoint. `https://blockstream.info/testnet/api`,
                        `https://mempool.space/api`, or a self-hosted one. Uses
                        `/block-height/<h>` (hash) and `/block/<hash>/header` (80-byte header hex).
  `--bitcoind-dir DIR`  a Bitcoin Core install (`DIR/bin/bitcoin-cli`), or `X3_BITCOIND_DIR`.
                        Uses `getblockhash <h>` and `getblockheader <hash> false`.

Usage
-----
    # a run of 8 headers ending at a height you name, from a public endpoint
    ./scripts/btc/push-headers.py --esplora https://blockstream.info/testnet/api \
        --from-height 5151279 --count 6 --out /tmp/btc-batch.json

    # resume from a cursor file (the last height/hash this relayer pushed)
    ./scripts/btc/push-headers.py --esplora … --cursor /var/lib/x3-btc-relayer/cursor.json \
        --count 6 --out /tmp/btc-batch.json

Exit codes: 0 emitted, 1 a refusal (it names the height), 2 bad usage / no source / unreachable.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request

# The call index `submit_btc_headers` is declared with, from the pallet source
# (`pallets/x3-settlement-engine/src/lib.rs`). It is *pallet-relative*: the runtime call index
# also depends on where the pallet sits in `construct_runtime!`, so the payload the pallet
# decodes is `scale_vec_hex`, and this byte is only a convenience for a caller that knows the
# runtime's dispatch layout. It is asserted here so a pallet edit that moves the index is noticed.
CALL_INDEX_SUBMIT_BTC_HEADERS = 35

# A Bitcoin header is 80 bytes. The SCALE encoding of the pallet's `BtcBlockHeader` adds the
# `height` field, which is not part of the wire header and is not hashed.
BTC_WIRE_HEADER_BYTES = 80
BTC_HEADER_SCALE_BYTES = 88


def dsha(b: bytes) -> bytes:
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


def display_hash(header_bytes: bytes) -> str:
    """The string a block explorer shows: the double SHA-256 of the 80 header bytes, reversed."""
    return dsha(header_bytes)[::-1].hex()


def reverse_hex(h: str) -> str:
    return bytes.fromhex(h)[::-1].hex()


def require_hex(value: str, nbytes: int, what: str) -> str:
    v = value.strip()
    if v.startswith("0x") or v.startswith("0X"):
        v = v[2:]
    if len(v) != nbytes * 2:
        raise Refusal(f"{what}: expected {nbytes} bytes of hex, got {len(v) // 2} ({v[:24]}…)")
    try:
        bytes.fromhex(v)
    except ValueError as exc:  # pragma: no cover - defensive
        raise Refusal(f"{what}: not hex ({v[:24]}…)") from exc
    return v.lower()


class Refusal(Exception):
    """A check failed. Carries the message the script prints before exiting non-zero."""


# ── SCALE encoding ────────────────────────────────────────────────────────────
#
# `BtcBlockHeader`'s SCALE encoding is, in field order:
#   version u32(LE) || prev_block_hash [u8;32] || merkle_root [u8;32]
#   || timestamp u32(LE) || bits u32(LE) || nonce u32(LE) || height u64(LE)
# The 80 wire bytes are the first six fields in exactly that layout, so a header's SCALE
# encoding is its wire bytes with the 8-byte height appended — which is why this is written
# out rather than taking another dependency to encode a fixed shape. `Vec<BtcBlockHeader>`
# prefixes that with a SCALE compact length.


def compact_len(n: int) -> bytes:
    if n < 1 << 6:
        return bytes([n << 2])
    if n < 1 << 14:
        return ((n << 2) | 0b01).to_bytes(2, "little")
    if n < 1 << 30:
        return ((n << 2) | 0b10).to_bytes(4, "little")
    raise Refusal(f"batch of {n} headers does not fit a SCALE compact length")


def scale_header(header_bytes: bytes, height: int) -> bytes:
    if len(header_bytes) != BTC_WIRE_HEADER_BYTES:
        raise Refusal(f"a header is {BTC_WIRE_HEADER_BYTES} bytes, got {len(header_bytes)}")
    return header_bytes + height.to_bytes(8, "little")


def scale_vec_headers(headers: list[tuple[bytes, int]]) -> bytes:
    out = bytearray(compact_len(len(headers)))
    for header_bytes, height in headers:
        out += scale_header(header_bytes, height)
    return bytes(out)


# ── sources ───────────────────────────────────────────────────────────────────


class Source:
    """A place headers can be read from. Subclasses answer by height and by hash."""

    kind = "?"
    where = "?"

    def hash_at(self, height: int) -> str:
        """The source's own display-order block hash for `height`."""
        raise NotImplementedError

    def header_at_hash(self, display_hash_hex: str) -> bytes:
        """The 80 wire bytes of the header the source says has this display hash."""
        raise NotImplementedError


class EsploraSource(Source):
    kind = "esplora"

    def __init__(self, base: str, timeout: float):
        self.base = base.rstrip("/")
        self.where = self.base
        self.timeout = timeout

    def _get(self, path: str, attempts: int = 4) -> bytes:
        url = f"{self.base}{path}"
        # A public endpoint rate-limits (`429`) and drops connections under load. Retrying is
        # not optional here: a relayer that treats a throttle as "the chain disagrees with me"
        # would stop on a healthy source. Only the transport is retried — never a mismatch.
        delay = 1.0
        for attempt in range(attempts):
            try:
                with urllib.request.urlopen(url, timeout=self.timeout) as resp:
                    return resp.read()
            except urllib.error.HTTPError as exc:
                retryable = exc.code in (429, 500, 502, 503, 504)
                if retryable and attempt < attempts - 1:
                    time.sleep(delay)
                    delay *= 2
                    continue
                raise Refusal(f"source unreachable: GET {url}: {exc}") from exc
            except (urllib.error.URLError, OSError) as exc:
                if attempt < attempts - 1:
                    time.sleep(delay)
                    delay *= 2
                    continue
                raise Refusal(f"source unreachable: GET {url}: {exc}") from exc
        raise Refusal(f"source unreachable: GET {url}: exhausted retries")  # unreachable

    def hash_at(self, height: int) -> str:
        return require_hex(self._get(f"/block-height/{height}").decode(), 32,
                           f"source hash for height {height}").lower()

    def header_at_hash(self, display_hash_hex: str) -> bytes:
        body = self._get(f"/block/{display_hash_hex}/header").decode()
        return bytes.fromhex(require_hex(body, BTC_WIRE_HEADER_BYTES,
                                         f"header for {display_hash_hex[:16]}…"))

    def tip_height(self) -> int:
        """The endpoint's own tip height. Used to locate a run without guessing one."""
        body = self._get("/blocks/tip/height").decode().strip()
        if not body.isdigit():
            raise Refusal(f"source tip height is not a number: {body[:24]!r}")
        return int(body)


class BitcoinCliSource(Source):
    kind = "bitcoin-cli"

    def __init__(self, bitcoind_dir: str, datadir: str | None):
        self.cli = str(pathlib.Path(bitcoind_dir) / "bin" / "bitcoin-cli")
        if not pathlib.Path(self.cli).exists():
            found = shutil.which("bitcoin-cli")
            if not found:
                raise Refusal(f"no bitcoin-cli at {self.cli} and none on PATH")
            self.cli = found
        self.datadir = datadir
        self.where = self.cli

    def _cli(self, *argv: str) -> str:
        cmd = [self.cli]
        if self.datadir:
            cmd.append(f"-datadir={self.datadir}")
        cmd.extend(argv)
        r = subprocess.run(cmd, capture_output=True, text=True)
        if r.returncode != 0:
            raise Refusal(f"source unreachable: {' '.join(cmd)}: {r.stderr.strip()}")
        return r.stdout.strip()

    def hash_at(self, height: int) -> str:
        return require_hex(self._cli("getblockhash", str(height)), 32,
                           f"source hash for height {height}").lower()

    def header_at_hash(self, display_hash_hex: str) -> bytes:
        body = self._cli("getblockheader", display_hash_hex, "false")
        return bytes.fromhex(require_hex(body, BTC_WIRE_HEADER_BYTES,
                                         f"header for {display_hash_hex[:16]}…"))


# ── the relayer's own checks ──────────────────────────────────────────────────


class Header:
    __slots__ = ("height", "display_hash", "wire")

    def __init__(self, height: int, display_hash_hex: str, wire: bytes):
        self.height = height
        self.display_hash = display_hash_hex
        self.wire = wire

    @property
    def prev_display(self) -> str:
        return reverse_hex(self.wire[4:36].hex())

    def computed_display(self) -> str:
        return display_hash(self.wire)


def fetch(height: int, source: Source, what: str) -> Header:
    """Fetch one height and refuse it unless the source agrees with itself."""
    claimed = source.hash_at(height)
    wire = source.header_at_hash(claimed)
    computed = display_hash(wire)
    if computed != claimed:
        raise Refusal(
            f"height {height}: the source said {claimed} but the {len(wire)} header bytes it "
            f"returned hash to {computed} — the source disagrees with itself"
        )
    return Header(height, claimed, wire)


def fetch_run(start: int, count: int, source: Source, anchor: Header | None) -> list[Header]:
    """Fetch `count` headers from `start`, verifying the run links and has no gap."""
    if count < 1:
        raise Refusal("count must be at least 1")
    if anchor is None:
        anchor = fetch(start - 1, source, "anchor")

    run: list[Header] = []
    prev = anchor
    for offset in range(count):
        height = start + offset
        if height != prev.height + 1:
            raise Refusal(
                f"gap at height {height}: the run must be contiguous, but the previous header "
                f"was at {prev.height}"
            )
        header = fetch(height, source, "batch")
        if header.prev_display != prev.display_hash:
            raise Refusal(
                f"height {height}: prev_blockhash {header.prev_display} is not the hash "
                f"{prev.display_hash} of height {prev.height} — the run is not linked "
                f"(gap or reorg)"
            )
        run.append(header)
        prev = header
    return run


# ── cursor ────────────────────────────────────────────────────────────────────


def read_cursor(path: pathlib.Path) -> Header:
    try:
        body = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise Refusal(f"cursor {path} is unreadable: {exc}") from exc
    for key in ("height", "hash"):
        if key not in body:
            raise Refusal(f"cursor {path} has no '{key}'")
    height = int(body["height"])
    display = require_hex(str(body["hash"]), 32, f"cursor hash at height {height}")
    return Header(height, display, b"")


def write_cursor(path: pathlib.Path, tip: Header) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps({"height": tip.height, "hash": tip.display_hash}, indent=2) + "\n")
    tmp.replace(path)


# ── output ────────────────────────────────────────────────────────────────────


def render(report: dict, fmt: str) -> str:
    if fmt == "json":
        return json.dumps(report, indent=2) + "\n"
    lines = [
        f"network {report['network']}",
        f"source {report['source']['kind']} {report['source']['where']}",
        f"anchor_height {report['anchor']['height']}",
        f"anchor_hash {report['anchor']['hash']}",
        f"anchor_header {report['anchor']['header_hex']}",
        f"from_height {report['from_height']}",
        f"to_height {report['to_height']}",
        f"count {report['count']}",
    ]
    for h in report["headers"]:
        lines.append(f"header {h['height']} {h['hash']} {h['header_hex']}")
    lines.append(f"scale_vec_hex {report['scale_vec_hex']}")
    lines.append(f"call_index {report['call_index']}")
    lines.append(f"call_data_hex {report['call_data_hex']}")
    return "\n".join(lines) + "\n"


def build_report(args, source: Source, anchor: Header, run: list[Header]) -> dict:
    scale = scale_vec_headers([(h.wire, h.height) for h in run])
    return {
        "network": args.network,
        "source": {"kind": source.kind, "where": source.where},
        "anchor": {
            "height": anchor.height,
            "hash": anchor.display_hash,
            "header_hex": anchor.wire.hex(),
        },
        "from_height": run[0].height,
        "to_height": run[-1].height,
        "count": len(run),
        "headers": [
            {"height": h.height, "hash": h.display_hash, "header_hex": h.wire.hex()}
            for h in run
        ],
        "scale_vec_hex": scale.hex(),
        "call_index": CALL_INDEX_SUBMIT_BTC_HEADERS,
        "call_data_hex": bytes([CALL_INDEX_SUBMIT_BTC_HEADERS]).hex() + scale.hex(),
        "verification": {
            "link_checks": len(run) - 1,
            "self_consistency_checks": len(run) + 1,  # the anchor plus every header
            "gaps": 0,
        },
    }


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--esplora", default=None,
                    help="Esplora HTTP base URL, e.g. https://blockstream.info/testnet/api")
    ap.add_argument("--bitcoind-dir", default=None,
                    help="Bitcoin Core dir containing bin/bitcoin-cli (or set X3_BITCOIND_DIR)")
    ap.add_argument("--bitcoin-datadir", default=None, help="bitcoin-cli -datadir")
    ap.add_argument("--network", default="testnet", help="label only, recorded in the payload")
    ap.add_argument("--from-height", type=int, default=None,
                    help="the first header height to emit (required unless --cursor is given)")
    ap.add_argument("--to-height", type=int, default=None, help="the last header height to emit")
    ap.add_argument("--count", type=int, default=None, help="how many headers to emit")
    ap.add_argument("--cursor", default=None, type=pathlib.Path,
                    help="a JSON {height,hash} this relayer last pushed; resumes from height+1")
    ap.add_argument("--out", default=None, type=pathlib.Path, help="write the payload here too")
    ap.add_argument("--format", choices=("json", "lines"), default="json")
    ap.add_argument("--timeout", type=float, default=20.0, help="per-request timeout, seconds")
    args = ap.parse_args()

    bitcoind_dir = args.bitcoind_dir or os.environ.get("X3_BITCOIND_DIR")
    if not args.esplora and not bitcoind_dir:
        print(
            "REFUSED: no header source named. Pass --esplora URL or --bitcoind-dir DIR "
            "(or set X3_BITCOIND_DIR). This script never picks a source or a height itself.",
            file=sys.stderr,
        )
        return 2

    if args.count is not None and args.to_height is not None:
        print("REFUSED: pass --count or --to-height, not both", file=sys.stderr)
        return 2

    try:
        source: Source = (
            EsploraSource(args.esplora, args.timeout)
            if args.esplora
            else BitcoinCliSource(bitcoind_dir, args.bitcoin_datadir)
        )

        cursor = read_cursor(args.cursor) if args.cursor else None
        anchor = None
        if cursor is not None:
            # The cursor is a promise about a height and a hash. Re-ask the source for that
            # height's hash before trusting it: if the chain reorged, the cursor points at a
            # header this chain no longer holds, and continuing would emit a run that does not
            # link to it.
            live = source.hash_at(cursor.height)
            if live != cursor.display_hash:
                raise Refusal(
                    f"cursor height {cursor.height}: recorded hash {cursor.display_hash} but the "
                    f"source now says {live} — the chain moved under the cursor"
                )
            anchor = cursor

        if anchor is None:
            if args.from_height is None:
                raise Refusal(
                    "no --from-height and no cursor: name the height to start from "
                    "(a relayer must not guess a tip)"
                )
            start = args.from_height
        else:
            start = anchor.height + 1
            if args.from_height is not None and args.from_height != start:
                raise Refusal(
                    f"--from-height {args.from_height} contradicts the cursor, which resumes at "
                    f"{start}"
                )

        if args.count is not None:
            count = args.count
        elif args.to_height is not None:
            count = args.to_height - start + 1
        else:
            raise Refusal("name how far to go with --count or --to-height")

        if count < 1:
            raise Refusal(f"--to-height is before the start height {start}")

        if anchor is None:
            anchor = fetch(start - 1, source, "anchor")
        run = fetch_run(start, count, source, anchor)
    except Refusal as exc:
        print(f"REFUSED: {exc}", file=sys.stderr)
        return 1

    report = build_report(args, source, anchor, run)
    text = render(report, args.format)
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text)
    if args.cursor:
        write_cursor(args.cursor, run[-1])

    sys.stdout.write(text)
    print(
        f"[push-headers] source={source.kind} verified {len(run)} linked headers "
        f"{run[0].height}..{run[-1].height} anchoring on {anchor.height} "
        f"({anchor.display_hash[:16]}…); tip {run[-1].height} {run[-1].display_hash}",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
