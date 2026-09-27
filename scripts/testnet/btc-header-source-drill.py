#!/usr/bin/env python3
"""The BTC header *source* proven against a real public endpoint, and proven to refuse a lie.

`scripts/btc/push-headers.py` is the relayer's source half: it fetches consecutive Bitcoin
headers, checks that the run links and that the source agrees with itself, and prints the SCALE
payload `submit_btc_headers` decodes. This drill is the live evidence for that:

  1. fetch a real run from a real public testnet Esplora endpoint (`--esplora`, default
     `https://blockstream.info/testnet/api`), print every height/hash/link, and check that the
     emitted payload is a well-formed SCALE `Vec<BtcBlockHeader>`;
  2. run the fetcher against a local proxy that forwards to that same endpoint and tampers
     exactly one answer, and require it to **refuse**:
       * `gap`      — the endpoint claims a height is a later block's hash, so the run skips one
                      and the child's `prev_blockhash` is not its parent's hash (a "tampered
                      parent" is the same failure: the link check, not the hash check, catches it);
       * `disagree` — the endpoint returns header bytes that do not hash to the hash it claimed
                      for that height (a lying or badly cached source);
     each must exit non-zero with a message that names the height;
  3. `--write-fixture PATH` regenerates the checked-in pallet fixture
     (`pallets/x3-settlement-engine/src/tests/data/btc_relayer_testnet_headers.txt`) from a real
     run, so the Rust test that decodes the payload has a deterministic, offline input.

Exit codes: 0 pass, 1 a check failed, 2 skip (network unreachable / dependency missing) — a skip
verifies nothing and is never reported as a pass.

What this proves and what it does not: it proves the fetcher follows a real public chain and
refuses a source that lies, and that what it emits is the pallet's payload. It does **not** prove
the pallet accepts a public-network header on a live chain — the Rust test anchors a real header
and admits the emitted batch in a mock runtime, and no public chain has a checkpoint pinned yet.
It also does not exercise the `bitcoin-cli` source: this host has no Bitcoin Core install.
"""

from __future__ import annotations

import argparse
import http.server
import importlib.util
import json
import pathlib
import subprocess
import sys
import threading
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
PUSH = ROOT / "scripts" / "btc" / "push-headers.py"
# Public testnet Esplora endpoints, tried in order when the caller names none. A single public
# host rate-limits (`429`) under repeated use, and the drill must not read a throttle as "the
# chain is wrong": it falls through to another real endpoint, printing which one it used. Both
# serve the same testnet chain, so a header run is identical either way.
ENDPOINT_CANDIDATES = (
    "https://blockstream.info/testnet/api",
    "https://mempool.space/testnet/api",
)
FIXTURE_PATH = (ROOT / "pallets" / "x3-settlement-engine" / "src" / "tests" / "data"
                / "btc_relayer_testnet_headers.txt")


def load_push_headers():
    spec = importlib.util.spec_from_file_location("x3_push_headers", PUSH)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


PH = load_push_headers()


def run(args: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(PUSH), *args], capture_output=True, text=True)


# ── the checks ────────────────────────────────────────────────────────────────


def check(name: str, ok: bool, detail: str = "") -> None:
    print(f"  {'PASS' if ok else 'FAIL'}  {name}{'' if ok else ' — ' + detail}")
    if not ok:
        raise SystemExit(1)


def window_from_source(esplora: str, first: int, count: int) -> dict:
    """Fetch the real run this drill will talk about, straight from the public endpoint."""
    src = PH.EsploraSource(esplora, 20.0)
    anchor = PH.fetch(first - 1, src, "anchor")
    run = PH.fetch_run(first, count, src, anchor)
    return {"anchor": anchor, "run": run}


def check_payload_shape(report: dict) -> None:
    """The emitted bytes must be a well-formed SCALE `Vec<BtcBlockHeader>`."""
    count = report["count"]
    scale = bytes.fromhex(report["scale_vec_hex"])
    prefix = PH.compact_len(count)
    expected_len = len(prefix) + PH.BTC_HEADER_SCALE_BYTES * count
    check("payload is compact(len) + 88*count bytes",
          len(scale) == expected_len and scale[:len(prefix)] == prefix,
          f"got {len(scale)} bytes for {count} headers")
    body = scale[len(prefix):]
    for i, h in enumerate(report["headers"]):
        chunk = body[i * PH.BTC_HEADER_SCALE_BYTES:(i + 1) * PH.BTC_HEADER_SCALE_BYTES]
        height = int.from_bytes(chunk[80:88], "little")
        check(f"payload header {i} carries height {h['height']}", height == h["height"],
              f"encoded {height}")
    # And the whole call payload is the call index plus that Vec.
    check("call_data is call index 35 plus the same Vec",
          report["call_data_hex"] == f"{report['call_index']:02x}" + report["scale_vec_hex"],
          "call payload mismatch")


class TamperProxy:
    """Forwards Esplora reads to the real endpoint, changing one answer.

    The proxy resolves the real window up front (through the same client the fetcher uses), so
    what it serves is real Bitcoin data with exactly one field altered — never a canned header.
    """

    def __init__(self, esplora: str, first: int, count: int, mode: str, target: int):
        self.mode = mode
        self.target = target
        self.by_height = {}
        self.by_hash = {}
        src = PH.EsploraSource(esplora, 20.0)
        anchor = PH.fetch(first - 1, src, "anchor")
        run = PH.fetch_run(first, count, src, anchor)
        for h in [anchor, *run]:
            self.by_height[h.height] = h
            self.by_hash[h.display_hash] = h
        self.order = [anchor.height, *[h.height for h in run]]
        self.server: http.server.ThreadingHTTPServer | None = None
        self.thread: threading.Thread | None = None
        self.port = 0

    def _claimed(self, height: int) -> str:
        if self.mode == "gap" and height == self.target:
            # Claim this height is a *later* block: the next height in the run, so the run has
            # skipped one. The header that comes back is self-consistent; only the link fails.
            later = height + 2 if (height + 2) in self.by_height else height + 1
            return self.by_height[later].display_hash
        return self.by_height[height].display_hash

    def _header(self, claimed_hash: str) -> bytes:
        header = self.by_hash.get(claimed_hash)
        if header is None:
            raise KeyError(claimed_hash)
        if self.mode == "disagree" and header.height == self.target:
            # A source that returns real bytes under a hash those bytes do not produce.
            flipped = bytearray(header.wire)
            flipped[76] ^= 0x01  # the nonce; the header no longer hashes to `claimed_hash`
            return bytes(flipped)
        return header.wire

    def start(self) -> str:
        proxy = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_args):  # keep the drill output clean
                pass

            def _send(self, payload: str, code: int = 200) -> None:
                body = payload.encode()
                self.send_response(code)
                self.send_header("Content-Type", "text/plain")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def do_GET(self):  # noqa: N802 - the stdlib's spelling
                try:
                    if self.path.startswith("/block-height/"):
                        height = int(self.path.rsplit("/", 1)[1])
                        self._send(proxy._claimed(height))
                    elif self.path.startswith("/block/") and self.path.endswith("/header"):
                        claimed = self.path[len("/block/"):-len("/header")]
                        self._send(proxy._header(claimed).hex())
                    elif self.path == "/blocks/tip/height":
                        self._send(str(max(proxy.by_height)))
                    else:
                        self._send("not found", code=404)
                except (KeyError, ValueError):
                    self._send("not found", code=404)

        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.port = self.server.server_address[1]
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        return f"http://127.0.0.1:{self.port}"

    def stop(self) -> None:
        if self.server:
            self.server.shutdown()
            self.server.server_close()
        if self.thread:
            self.thread.join(timeout=5)


def refusal_case(esplora: str, first: int, count: int, mode: str, target: int) -> str:
    proxy = TamperProxy(esplora, first, count, mode, target)
    url = proxy.start()
    try:
        r = run(["--esplora", url, "--from-height", str(first), "--count", str(count)])
    finally:
        proxy.stop()
    return (r.returncode, r.stdout, r.stderr)


# ── fixture regeneration ──────────────────────────────────────────────────────


def uniform_bits_window(esplora: str, count: int, scan: int) -> int:
    """The most recent real window of `count`+1 consecutive headers sharing one `nBits`.

    Bitcoin's difficulty changes only at a retarget boundary on mainnet, and the pallet's
    `btc_bits_follow_parent` requires a child to carry its parent's `nBits` off one. Testnet3
    additionally mines "minimum difficulty" blocks whenever twenty minutes pass, so its `nBits`
    alternates away from the boundary and only a uniform run is admissible under the pallet's
    (mainnet) rule. This searches for such a run rather than pretending the rule does not bite.
    """
    src = PH.EsploraSource(esplora, 20.0)
    tip = src.tip_height()
    heights = list(range(tip - scan, tip + 1))
    window: list = []
    found = None
    for height in heights:
        header = PH.fetch(height, src, "scan")
        bits = int.from_bytes(header.wire[72:76], "little")
        if window and bits != int.from_bytes(window[-1].wire[72:76], "little"):
            window = [header]
        else:
            window.append(header)
        if len(window) >= count + 1:
            found = window[-1].height - count + 1
    if found is not None:
        return found
    raise PH.Refusal(
        f"no run of {count + 1} consecutive headers with one nBits in the last {scan} blocks "
        f"of {esplora}; widen --scan"
    )


def assert_uniform_bits(esplora: str, first: int, count: int) -> None:
    """Refuse to bake a fixture the pallet's own difficulty rule would reject."""
    src = PH.EsploraSource(esplora, 20.0)
    bits = {
        int.from_bytes(PH.fetch(height, src, "fixture").wire[72:76], "little")
        for height in range(first - 1, first + count)
    }
    if len(bits) != 1:
        raise PH.Refusal(
            f"heights {first - 1}..{first + count - 1} do not share one nBits "
            f"({', '.join(hex(b) for b in sorted(bits))}); the pallet's rule admits only a "
            f"uniform run off a retarget boundary"
        )


def write_fixture(esplora: str, count: int, scan: int, path: pathlib.Path,
                  from_height: int | None) -> None:
    path = path.resolve()
    first = from_height if from_height is not None else uniform_bits_window(esplora, count, scan)
    assert_uniform_bits(esplora, first, count)
    r = run(["--esplora", esplora, "--from-height", str(first), "--count", str(count),
             "--format", "lines"])
    if r.returncode != 0:
        raise SystemExit(f"[btc-source-drill] fixture run refused: {r.stderr.strip()}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(r.stdout)
    try:
        shown = path.relative_to(ROOT)
    except ValueError:
        shown = path
    print(f"[btc-source-drill] wrote fixture {shown} "
          f"({count} headers from height {first}, source {esplora})")


# ── main ──────────────────────────────────────────────────────────────────────


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--esplora", default=None,
                    help="an Esplora base URL; defaults to the first reachable public one")
    ap.add_argument("--count", type=int, default=6, help="headers per run")
    ap.add_argument("--back", type=int, default=12, help="how far below the tip to start")
    ap.add_argument("--scan", type=int, default=64, help="blocks to scan for the fixture window")
    ap.add_argument("--write-fixture", type=pathlib.Path, default=None)
    ap.add_argument("--fixture-from-height", type=int, default=None,
                    help="pin the fixture's first header height instead of scanning")
    args = ap.parse_args()

    candidates = (args.esplora,) if args.esplora else ENDPOINT_CANDIDATES
    esplora, tip = None, None
    for candidate in candidates:
        try:
            tip = PH.EsploraSource(candidate, 20.0).tip_height()
        except PH.Refusal as exc:
            print(f"[btc-source-drill] {candidate} unusable: {exc}", file=sys.stderr)
            continue
        esplora = candidate
        break
    if esplora is None:
        print("[btc-source-drill] SKIP — no public Esplora endpoint reachable. "
              "Nothing was verified.", file=sys.stderr)
        return 2
    args.esplora = esplora
    print(f"[btc-source-drill] endpoint {args.esplora}")
    print(f"[btc-source-drill] tip height {tip}")

    first = tip - args.back
    try:
        window = window_from_source(args.esplora, first, args.count)
    except PH.Refusal as exc:
        print(f"[btc-source-drill] FAIL — could not read a real run: {exc}", file=sys.stderr)
        return 1

    print(f"[btc-source-drill] verifying {args.count} headers from {first} "
          f"(anchor {window['anchor'].height})")
    prev = window["anchor"]
    print(f"    anchor {prev.height} {prev.display_hash} (checkpoint this run must link to)")
    for header in window["run"]:
        linked = header.prev_display == prev.display_hash
        print(f"    height {header.height} {header.display_hash} "
              f"prev→{prev.height} {'link-ok' if linked else 'LINK-BAD'}")
        check(f"height {header.height} links to {prev.height}", linked,
              f"prev {header.prev_display} != {prev.display_hash}")
        prev = header

    r = run(["--esplora", args.esplora, "--from-height", str(first), "--count", str(args.count)])
    check("the fetcher emitted a payload", r.returncode == 0, r.stderr.strip())
    report = json.loads(r.stdout)
    check_payload_shape(report)
    print(f"    payload {report['count']} headers, tip {report['to_height']} "
          f"{report['headers'][-1]['hash']}, {len(report['scale_vec_hex']) // 2} bytes")

    # Default OFF: no source named, no run.
    r = run(["--from-height", str(first), "--count", "1"])
    check("no source named is refused (the relayer is off by default)",
          r.returncode == 2 and "no header source named" in r.stderr, r.stderr.strip())

    # Refusals: a tampered source at a named height.
    target = first + 2
    code, out, err = refusal_case(args.esplora, first, args.count, "gap", target)
    check(f"gap at height {target} is refused non-zero and names it",
          code == 1 and f"height {target}" in err and "not linked" in err,
          f"exit {code}: {err.strip()}")
    print(f"    gap     → {err.strip()}")

    code, out, err = refusal_case(args.esplora, first, args.count, "disagree", target)
    check(f"a source disagreeing with itself at height {target} is refused and names it",
          code == 1 and f"height {target}" in err and "disagrees with itself" in err,
          f"exit {code}: {err.strip()}")
    print(f"    disagree→ {err.strip()}")

    # A batch that is not a single linked run is never emitted: the refusal above produced no
    # payload on stdout.
    check("a refused run emits no payload", out == "", f"stdout was {out[:80]!r}")

    if args.write_fixture:
        write_fixture(args.esplora, args.count, args.scan, args.write_fixture,
                      args.fixture_from_height)

    print("[btc-source-drill] PASS — the relayer followed a real public chain and refused a lie")
    return 0


if __name__ == "__main__":
    sys.exit(main())
