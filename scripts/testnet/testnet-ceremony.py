#!/usr/bin/env python3
"""Record and verify a testnet launch against a ceremony manifest.

    testnet-ceremony.py record <spec> --node-bin <path> --rpc 9944[,9945,…] --out ceremony.json
    testnet-ceremony.py sign   <ceremony.json> [--keys-dir <dir>] [--out <signed.json>]
    testnet-ceremony.py attest <ceremony.json> --key <ed25519 seed hex>
    testnet-ceremony.py verify <ceremony.json> --rpc 9944[,9945,…]

`record` writes what was actually launched: the spec and node binary with their sha256s,
the chain name, the genesis hash the network reports, the runtime version, the authority
set, each validator's libp2p peer id and the height it has finalized.

`verify` takes that manifest to a *running* network and re-checks every one of those
claims, one line per check, naming the first disagreement. It is the check a user or an
operator should be able to run against a published testnet: not "the node answered", but
"this is the artifact that was launched, these are its authorities, and this is the
genesis hash".

`attest` and `sign` are what turn that record into a ceremony. Each validator signs the
manifest's canonical bytes with the ed25519 key its GRANDPA authority *is*, so a third
party can check the operators' agreement using only the manifest — the key that signed is
the key the chain would accept as an authority, and a key that is not one of them is
refused. `attest` adds one signature, which is the shape a real ceremony has (each
operator signs the same manifest, and the signatures accumulate); `sign` is the
rehearsal form, where one machine holds every key. `verify` requires a supermajority of
distinct authority signatures before it will look at the network, recomputes the
threshold from the authority set rather than believing the manifest's own copy of it,
and refuses an unsigned manifest unless `--allow-unsigned` says the recording is a
boot-strapping one.

Nothing here trusts the network for a value it is supposed to prove: the spec hash and
the binary hash come from the files, and the chain-side values are compared against the
manifest, not against each other.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
import urllib.request
from pathlib import Path

GENESIS_HEIGHT = 0

# ── SS58 and SCALE decoding ──────────────────────────────────────────────────
#
# A ceremony manifest has to be checkable by someone who has the manifest and
# nothing else, so everything it is verified against is decoded here rather than
# taken from the machine that produced it. Two encodings are involved: the spec
# declares its GRANDPA authorities as SS58 addresses, and the chain returns them
# as a SCALE-encoded `Vec<(AuthorityId, u64)>`. Decoding both and requiring them
# to agree means neither decoder has to be trusted on its own.

B58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def base58_decode(text: str) -> bytes:
    """Base58 (Bitcoin alphabet) decode, with leading `1`s restored as zero bytes."""
    value = 0
    for char in text:
        try:
            digit = B58_ALPHABET.index(char)
        except ValueError as exc:
            raise ValueError(f"{text!r} is not base58: {char!r} is not in the alphabet") from exc
        value = value * 58 + digit
    body = value.to_bytes((value.bit_length() + 7) // 8, "big") if value else b""
    return b"\x00" * (len(text) - len(text.lstrip("1"))) + body


def ss58_pubkey(address: str) -> bytes:
    """The 32 bytes an SS58 address commits to, with its checksum verified.

    Only the one-byte-prefix form is accepted (the 47/48 character addresses the
    node writes by default). Anything else is refused rather than guessed at: a
    silently mis-decoded authority key would make every later check meaningless.
    """
    raw = base58_decode(address)
    if len(raw) != 35:
        raise ValueError(
            f"{address!r} decodes to {len(raw)} bytes, not the 35 of a one-byte-prefix "
            f"SS58 address"
        )
    prefix, key, checksum = raw[:1], raw[1:33], raw[33:]
    expected = hashlib.blake2b(b"SS58PRE" + prefix + key, digest_size=64).digest()[:2]
    if checksum != expected:
        raise ValueError(
            f"{address!r} has checksum {checksum.hex()}, not {expected.hex()} — it is not a "
            f"well-formed SS58 address"
        )
    return key


def decode_grandpa_authorities(scaled: str) -> list[bytes]:
    """`Vec<(AuthorityId, u64)>` exactly as `GrandpaApi_grandpa_authorities` returns it."""
    raw = bytes.fromhex(scaled[2:] if scaled.startswith("0x") else scaled)
    if not raw:
        raise ValueError("the authority set is empty")
    length_mode = raw[0] & 0b11
    if length_mode != 0:
        raise ValueError("the authority set is longer than 63 entries; not a testnet authority set")
    count = raw[0] >> 2
    body = raw[1:]
    if len(body) != count * (32 + 8):
        raise ValueError(
            f"the authority set claims {count} entries ({count * 40} bytes) but carries "
            f"{len(body)}"
        )
    return [body[index * 40 : index * 40 + 32] for index in range(count)]


def rpc(port: int, method: str, params: list | None = None, timeout: float = 10.0):
    body = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": params or []}
    ).encode()
    req = urllib.request.Request(
        f"http://127.0.0.1:{port}", data=body, headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        payload = json.loads(resp.read())
    if "error" in payload:
        raise RuntimeError(f"{method} on {port}: {payload['error']}")
    return payload.get("result")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def header_number(port: int, block_hash: str) -> int | None:
    header = rpc(port, "chain_getHeader", [block_hash])
    if not header:
        return None
    return int(header["number"], 16)


def spec_authorities(spec: dict) -> dict:
    """The authority sets the spec itself declares (plain form)."""
    cfg = spec.get("genesis", {}).get("runtimeGenesis", {}).get("config", {})
    aura = cfg.get("aura", {}).get("authorities", [])
    grandpa = [
        entry[0] if isinstance(entry, list) else entry
        for entry in cfg.get("grandpa", {}).get("authorities", [])
    ]
    return {"aura": aura, "grandpa": grandpa}


# ── Operator attestations ────────────────────────────────────────────────────
#
# A manifest that only exists on the machine that produced it is not a record: it
# is a file. What makes it a ceremony is that each authority signs the artifacts
# it is agreeing to, with a key that is already published in the spec, so a third
# party can check the agreement without trusting the recorder.
#
# The signature covers the manifest with its `attestations` member removed, in a
# canonical encoding (sorted keys, no insignificant whitespace). Nothing about
# the signature depends on how the JSON was formatted on disk.

ATTESTATION_SCHEME = "ed25519"
SS58_PREFIX_NOTE = "one-byte SS58 prefix"


def supermajority_threshold(total: int) -> int:
    """The workspace's one supermajority rule: `floor(2n/3) + 1`.

    Mirrors `x3_validator_attestation::supermajority_threshold`, which the relayer
    and the verifiers call so that a producer and a consumer cannot drift into
    disagreeing about what "enough" means. The value is *not* taken from the
    manifest: it is recomputed from the authority set the chain reports, and the
    manifest's own `required` has to equal it.
    """
    return (2 * total) // 3 + 1


def canonical_manifest_bytes(manifest: dict) -> bytes:
    """The bytes an attestation is a signature over."""
    body = {key: value for key, value in manifest.items() if key != "attestations"}
    return json.dumps(body, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def authority_keys(manifest: dict) -> list[bytes]:
    """The GRANDPA authority set, from the chain, cross-checked against the spec.

    The chain's answer is the one that matters (it is what a validator would be
    checked against), and the spec's addresses are decoded independently; a
    manifest whose two accounts of its own authorities disagree is refused here,
    because every attestation below is checked against this list.
    """
    chain_scaled = manifest["validators"][0]["grandpa_authorities_scaled"]
    from_chain = decode_grandpa_authorities(chain_scaled)
    from_spec = [ss58_pubkey(entry) for entry in manifest["authorities_expected_from_spec"]["grandpa"]]
    if sorted(from_chain) != sorted(from_spec):
        raise ValueError(
            f"the chain reports {len(from_chain)} GRANDPA authorities and the spec declares "
            f"{len(from_spec)}, and they are not the same keys"
        )
    if len(set(from_chain)) != len(from_chain):
        raise ValueError("the authority set lists the same key twice")
    return from_chain


def ed25519_pubkey_from_seed(seed: bytes) -> bytes:
    """The public key for a raw ed25519 seed, via OpenSSL.

    The private key is the seed wrapped in the PKCS#8 container RFC 8410 defines
    for Ed25519 — `SEQUENCE { INTEGER 0, SEQUENCE { OID 1.3.101.112 }, OCTET STRING
    { OCTET STRING seed } }` — so the key exists only for the duration of the
    command and never in a bespoke on-disk format.

    The node derives the same key from the same seed
    (`x3-chain-node keys generate --key-type grandpa --seed 0x… --output hex`), and
    `sign` refuses any attestation whose derived key is not in the authority set,
    so a wrong derivation cannot produce a manifest that verifies.
    """
    spki = _openssl(
        ["pkey", "-inform", "DER", "-pubout", "-outform", "DER"],
        {"key.der": _pkcs8(seed)},
        "deriving the public key",
    )
    if spki is None or len(spki) != 44:
        raise RuntimeError("OpenSSL did not return a 44-byte Ed25519 SPKI")
    return spki[-32:]


def ed25519_sign(seed: bytes, message: bytes) -> bytes:
    """A detached Ed25519 signature over `message`, via OpenSSL."""
    signature = _openssl(
        ["pkeyutl", "-sign", "-rawin"],
        {"key.der": _pkcs8(seed), "message.bin": message},
        "signing",
    )
    if signature is None or len(signature) != 64:
        raise RuntimeError("OpenSSL did not return a 64-byte Ed25519 signature")
    return signature


def ed25519_verify(pubkey: bytes, message: bytes, signature: bytes) -> bool:
    """Whether `signature` is `pubkey`'s signature over `message`.

    A signature that does not verify is a `False`, not an exception: refusing one
    is this function working. A missing or unusable OpenSSL is an exception, since
    then nothing was verified at all.
    """
    if len(pubkey) != 32 or len(signature) != 64:
        return False
    spki = bytes.fromhex("302a300506032b6570032100") + pubkey
    result = _openssl(
        ["pkeyutl", "-verify", "-pubin", "-rawin"],
        {"pub.der": spki, "message.bin": message, "signature.bin": signature},
        "verifying",
        tolerate_refusal=True,
    )
    return result is not None


def _pkcs8(seed: bytes) -> bytes:
    """A raw 32-byte ed25519 seed in the PKCS#8 container (RFC 8410)."""
    if len(seed) != 32:
        raise ValueError(f"an ed25519 seed is 32 bytes, not {len(seed)}")
    return bytes.fromhex("302e020100300506032b657004220420") + seed


def _openssl(
    subcommand: list[str],
    files: dict[str, bytes],
    what: str,
    *,
    tolerate_refusal: bool = False,
) -> bytes | None:
    """Run one OpenSSL key operation against a scratch directory.

    The paths are bound by name because that is the only way OpenSSL will take
    them, and the directory is removed whether the command succeeded or not — a
    private key must not survive a crash here.
    """
    import subprocess
    import tempfile

    with tempfile.TemporaryDirectory() as work:
        for name, content in files.items():
            with open(f"{work}/{name}", "wb") as handle:
                handle.write(content)
        command = ["openssl", *subcommand]
        if "key.der" in files:
            # `pkey` reads its input with `-in` and infers DER from `-inform`;
            # `pkeyutl` insists on `-inkey` plus `-keyform`.
            if subcommand[0] == "pkeyutl":
                command += ["-inkey", f"{work}/key.der", "-keyform", "DER"]
            else:
                command += ["-in", f"{work}/key.der"]
        if "pub.der" in files:
            command += ["-inkey", f"{work}/pub.der", "-keyform", "DER"]
        if "message.bin" in files:
            command += ["-in", f"{work}/message.bin"]
        if "signature.bin" in files:
            command += ["-sigfile", f"{work}/signature.bin"]
        result = subprocess.run(command, capture_output=True)
        if result.returncode != 0:
            if tolerate_refusal:
                return None
            raise RuntimeError(
                f"openssl refused {what}: {result.stderr.decode(errors='replace').strip()[:200]}"
            )
        return result.stdout


def collect(port: int) -> dict:
    health = rpc(port, "system_health") or {}
    peer_id = rpc(port, "system_localPeerId")
    finalized = rpc(port, "chain_getFinalizedHead")
    return {
        "rpc_port": port,
        "peer_id": peer_id,
        "chain": rpc(port, "system_chain"),
        "genesis_hash": rpc(port, "chain_getBlockHash", [GENESIS_HEIGHT]),
        "runtime_version": rpc(port, "state_getRuntimeVersion"),
        "grandpa_authorities_scaled": rpc(
            port, "state_call", ["GrandpaApi_grandpa_authorities", "0x"]
        ),
        "peers": health.get("peers"),
        "finalized_hash": finalized,
        "finalized_height": header_number(port, finalized) if finalized else None,
    }


def cmd_record(args: argparse.Namespace) -> int:
    spec_path = Path(args.spec).resolve()
    if not spec_path.is_file():
        print(f"spec not found: {spec_path}", file=sys.stderr)
        return 2
    spec = json.loads(spec_path.read_text())
    node_bin = Path(args.node_bin).resolve()
    if not node_bin.is_file():
        print(f"node binary not found: {node_bin}", file=sys.stderr)
        return 2

    observers = [collect(port) for port in args.rpc]
    first = observers[0]
    manifest = {
        "manifest_version": 1,
        "chain": first["chain"],
        "chain_id": spec.get("id"),
        "chain_type": spec.get("chainType"),
        "spec": {
            "path": str(spec_path),
            "sha256": sha256_file(spec_path),
            "bytes": spec_path.stat().st_size,
        },
        "node_binary": {"path": str(node_bin), "sha256": sha256_file(node_bin)},
        "runtime_version": first["runtime_version"],
        "genesis": {
            "height": GENESIS_HEIGHT,
            "hash": first["genesis_hash"],
        },
        "authorities_expected_from_spec": spec_authorities(spec),
        "validators": [
            {
                "rpc_port": obs["rpc_port"],
                "peer_id": obs["peer_id"],
                "finalized_height": obs["finalized_height"],
                "peers": obs["peers"],
                "grandpa_authorities_scaled": obs["grandpa_authorities_scaled"],
            }
            for obs in observers
        ],
    }
    out = Path(args.out).resolve()
    out.write_text(json.dumps(manifest, indent=2) + "\n")
    print(
        f"[ceremony] recorded {len(observers)} validator(s): genesis {manifest['genesis']['hash']}, "
        f"spec {manifest['spec']['sha256'][:16]}…, spec_version "
        f"{manifest['runtime_version']['specVersion']} -> {out}"
    )
    return 0


def read_validator_seed(keys_dir: Path, index: int) -> bytes:
    """The GRANDPA ed25519 seed a validator's own key file records.

    `build-x3-testnet-spec.py` writes `seed=`/`aura=`/`grandpa=` per validator,
    all the same master for the fresh testnet; the spec's GRANDPA authority is
    derived from the last of them, so that is the one an attestation must use.
    """
    path = keys_dir / f"validator-{index}.suri"
    if not path.is_file():
        raise FileNotFoundError(f"{path} does not exist")
    for line in path.read_text().splitlines():
        if line.startswith("grandpa="):
            raw = line.split("=", 1)[1].strip()
            seed = bytes.fromhex(raw[2:] if raw.startswith("0x") else raw)
            if len(seed) != 32:
                raise ValueError(f"{path}: the grandpa seed is {len(seed)} bytes, not 32")
            return seed
    raise ValueError(f"{path} carries no `grandpa=` line")


class AttestationRefused(Exception):
    """An attestation that must not be added, and why."""


def append_attestation(manifest: dict, seed: bytes, label: str) -> dict:
    """Add one authority's signature, refusing anything that would weaken the set.

    Separate from `sign` because a real ceremony is not one machine holding every
    key: each operator signs the same manifest and the signatures accumulate. So
    this is the primitive, and it refuses three things a careless append would
    let through:

    * a key that is not one of the manifest's authorities;
    * a key that has already signed — which would inflate the count without
      adding a distinct attestation;
    * appending to a manifest that has been **edited since it was signed**: the
      older signatures no longer cover the current bytes, so the manifest would
      end up carrying a mix that no verifier can accept, and it says so now
      rather than at verification time.
    """
    authorities = authority_keys(manifest)
    public_key = ed25519_pubkey_from_seed(seed)
    if public_key not in authorities:
        raise AttestationRefused(
            f"{label} derives {public_key.hex()}, which is not one of the "
            f"{len(authorities)} GRANDPA authorities this manifest recorded"
        )

    attestations = manifest.get("attestations")
    if attestations is None:
        attestations = {
            "scheme": ATTESTATION_SCHEME,
            "covered": "the manifest with its `attestations` member removed, JSON with sorted "
            "keys and no insignificant whitespace",
            "authority_set_size": len(authorities),
            "required": supermajority_threshold(len(authorities)),
            "obtained": 0,
            "authorities": [],
        }
        manifest["attestations"] = attestations

    canonical = canonical_manifest_bytes(manifest)
    for existing in attestations["authorities"]:
        try:
            existing_key = bytes.fromhex(str(existing["public_key"]).removeprefix("0x"))
            existing_signature = bytes.fromhex(str(existing["signature"]).removeprefix("0x"))
        except (KeyError, ValueError) as exc:
            raise AttestationRefused(f"an existing attestation is malformed: {exc}") from exc
        if existing_key == public_key:
            raise AttestationRefused(f"{label} has already attested to this manifest")
        if not ed25519_verify(existing_key, canonical, existing_signature):
            raise AttestationRefused(
                f"attestation #{existing.get('index')} no longer verifies over this manifest — it "
                f"has been edited since it was signed, so adding to it would produce a set no "
                f"verifier can accept"
            )

    existing_indices = [
        int(existing.get("index", 0)) for existing in attestations["authorities"]
    ]
    entry = {
        "index": max(existing_indices, default=0) + 1,
        "public_key": "0x" + public_key.hex(),
        "signature": "0x" + ed25519_sign(seed, canonical).hex(),
    }
    attestations["authorities"].append(entry)
    attestations["obtained"] = len(attestations["authorities"])
    return entry


def cmd_attest(args: argparse.Namespace) -> int:
    """Add one operator's signature to a manifest."""
    manifest_path = Path(args.manifest).resolve()
    manifest = json.loads(manifest_path.read_text())
    seed = bytes.fromhex(args.key.removeprefix("0x"))
    try:
        entry = append_attestation(manifest, seed, f"the supplied key ({args.key[:10]}…)")
    except (AttestationRefused, KeyError, ValueError) as exc:
        print(f"[ceremony] refused: {exc}", file=sys.stderr)
        return 1
    out = Path(args.out).resolve() if args.out else manifest_path
    out.write_text(json.dumps(manifest, indent=2) + "\n")
    print(
        f"[ceremony] attested by one key -> {len(manifest['attestations']['authorities'])} of "
        f"{manifest['attestations']['authority_set_size']} (need "
        f"{manifest['attestations']['required']}) -> {out}"
    )
    return 0


def cmd_sign(args: argparse.Namespace) -> int:
    """Sign a recorded manifest with every validator key the operator holds.

    This is the rehearsal/mirror form: all the keys are on one machine. A real
    ceremony is `attest` once per operator — the two share `append_attestation`,
    so the refusals are the same either way.
    """
    manifest_path = Path(args.manifest).resolve()
    manifest = json.loads(manifest_path.read_text())

    authorities = authority_keys(manifest)
    keys_dir = (
        Path(args.keys_dir)
        if args.keys_dir
        else Path(manifest["spec"]["path"]).parent / "validator-keys"
    )
    for index in range(1, len(authorities) + 1):
        seed = read_validator_seed(keys_dir, index)
        try:
            append_attestation(manifest, seed, f"validator-{index}")
        except (AttestationRefused, KeyError, ValueError) as exc:
            print(f"[ceremony] refused: {exc}", file=sys.stderr)
            return 1

    attestations = manifest["attestations"]
    if attestations["obtained"] < attestations["required"]:
        print(
            f"only {attestations['obtained']} of {attestations['authority_set_size']} validators "
            f"signed; {attestations['required']} are required — refusing to write a manifest that "
            f"cannot meet its own threshold",
            file=sys.stderr,
        )
        return 1

    out = Path(args.out).resolve() if args.out else manifest_path
    out.write_text(json.dumps(manifest, indent=2) + "\n")
    print(
        f"[ceremony] signed by {attestations['obtained']} of "
        f"{attestations['authority_set_size']} GRANDPA authorities "
        f"(threshold {attestations['required']}) -> {out}"
    )
    return 0


def verify_attestations(manifest: dict, check) -> None:  # noqa: ANN001 - a local closure
    """Every check an operator attestation has to pass, named one line at a time."""
    try:
        authorities = authority_keys(manifest)
    except (KeyError, ValueError) as exc:
        check("manifest declares a decodable authority set", False, str(exc))
        return

    required = supermajority_threshold(len(authorities))
    attestations = manifest.get("attestations")
    if not isinstance(attestations, dict):
        check("manifest carries operator attestations", False, "no `attestations` member")
        return

    check(
        "attestation scheme",
        attestations.get("scheme") == ATTESTATION_SCHEME,
        f"{attestations.get('scheme')!r}",
    )
    # The bar is recomputed from the authority set, never taken from the manifest:
    # otherwise a recorder could lower its own threshold.
    check(
        f"the manifest's required count is the workspace rule ({required} of {len(authorities)})",
        attestations.get("required") == required,
        f"the manifest says {attestations.get('required')}",
    )

    canonical = canonical_manifest_bytes(manifest)
    entries = attestations.get("authorities")
    if not isinstance(entries, list) or not entries:
        check("manifest carries at least one signature", False, "`authorities` is empty")
        return

    verified = 0
    seen: set[bytes] = set()
    for position, entry in enumerate(entries):
        label = f"attestation {position} (#{entry.get('index') if isinstance(entry, dict) else '?'})"
        if not isinstance(entry, dict):
            check(f"{label} is well-formed", False, "not an object")
            continue
        try:
            public_key = bytes.fromhex(str(entry["public_key"]).removeprefix("0x"))
            signature = bytes.fromhex(str(entry["signature"]).removeprefix("0x"))
        except (KeyError, ValueError) as exc:
            check(f"{label} is well-formed", False, str(exc))
            continue
        if public_key not in authorities:
            check(f"{label} is an authority key", False, "not in the authority set")
            continue
        if public_key in seen:
            check(f"{label} is a distinct signer", False, "this key signed twice")
            continue
        if not ed25519_verify(public_key, canonical, signature):
            check(
                f"{label} verifies over the manifest",
                False,
                "the signature does not cover these manifest bytes",
            )
            continue
        seen.add(public_key)
        verified += 1
        check(f"{label} verifies over the manifest", True)

    check(
        f"distinct authority signatures reached the threshold ({verified}/{required})",
        verified >= required,
        f"{verified} verified, {required} required",
    )


def cmd_verify(args: argparse.Namespace) -> int:
    manifest = json.loads(Path(args.manifest).read_text())
    failures: list[str] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        if ok:
            print(f"  ok    {name}")
        else:
            print(f"  FAIL  {name}{(' — ' + detail) if detail else ''}")
            failures.append(name)

    spec_path = Path(manifest["spec"]["path"])
    if spec_path.is_file():
        check(
            "spec sha256 matches the manifest",
            sha256_file(spec_path) == manifest["spec"]["sha256"],
            f"expected {manifest['spec']['sha256'][:16]}…",
        )
    else:
        check("spec is present", False, f"missing {spec_path}")

    # The operators' own agreement over the artifacts. Checked before the running
    # network is consulted, because it is the one claim a live network cannot
    # corroborate: a chain will happily run from artifacts nobody attested to.
    if manifest.get("attestations") is None and args.allow_unsigned:
        print("  skip  operator attestations (--allow-unsigned)")
    else:
        verify_attestations(manifest, check)

    node_bin = Path(manifest["node_binary"]["path"])
    if args.node_bin and Path(args.node_bin).is_file():
        node_bin = Path(args.node_bin)
    if node_bin.is_file():
        check(
            "node binary sha256 matches the manifest",
            sha256_file(node_bin) == manifest["node_binary"]["sha256"],
            f"{node_bin} is not the launched binary",
        )
    else:
        print(f"  skip  node binary not available locally ({node_bin})")

    observers = [collect(port) for port in args.rpc]
    for obs in observers:
        label = f"rpc {obs['rpc_port']}"
        check(f"{label}: chain name", obs["chain"] == manifest["chain"], str(obs["chain"]))
        check(
            f"{label}: genesis hash",
            obs["genesis_hash"] == manifest["genesis"]["hash"],
            f"{obs['genesis_hash']} != {manifest['genesis']['hash']}",
        )
        expected_version = manifest["runtime_version"]
        check(
            f"{label}: spec_version",
            obs["runtime_version"]["specVersion"] == expected_version["specVersion"],
            f"{obs['runtime_version']['specVersion']} != {expected_version['specVersion']}",
        )
        check(
            f"{label}: transaction_version",
            obs["runtime_version"]["transactionVersion"]
            == expected_version["transactionVersion"],
            "runtime transaction version differs from the manifest",
        )
        check(
            f"{label}: GRANDPA authority set",
            obs["grandpa_authorities_scaled"] == manifest["validators"][0]["grandpa_authorities_scaled"],
            "this node's authority set is not the one the manifest recorded",
        )
        check(
            f"{label}: finality advancing",
            obs["finalized_height"] is not None
            and obs["finalized_height"] >= args.min_finalized,
            f"finalized height {obs['finalized_height']} < {args.min_finalized}",
        )

        recorded = next(
            (v for v in manifest["validators"] if v["rpc_port"] == obs["rpc_port"]), None
        )
        if recorded:
            check(
                f"{label}: peer id matches the manifest",
                obs["peer_id"] == recorded["peer_id"],
                f"{obs['peer_id']} != {recorded['peer_id']}",
            )
        else:
            check(
                f"{label}: peer id is one the manifest lists",
                obs["peer_id"] in [v["peer_id"] for v in manifest["validators"]],
                obs["peer_id"],
            )

    print()
    if failures:
        print(f"[ceremony] FAILED — {len(failures)} check(s): {', '.join(failures)}")
        return 1
    print(f"[ceremony] PASS — {len(observers)} validator(s) match the manifest")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="cmd", required=True)

    rec = sub.add_parser("record", help="write a manifest from files + a running network")
    rec.add_argument("spec")
    rec.add_argument("--node-bin", required=True)
    rec.add_argument("--rpc", type=lambda s: [int(p) for p in s.split(",")], default=[9944])
    rec.add_argument("--out", default="ceremony.json")
    rec.set_defaults(func=cmd_record)

    ver = sub.add_parser("verify", help="check a running network against a manifest")
    ver.add_argument("manifest")
    ver.add_argument("--rpc", type=lambda s: [int(p) for p in s.split(",")], default=[9944])
    ver.add_argument("--node-bin", default="")
    ver.add_argument(
        "--allow-unsigned",
        action="store_true",
        help=(
            "accept a manifest with no operator attestations. Off by default: a record that "
            "only exists on the machine that produced it is a file, not a ceremony."
        ),
    )
    ver.add_argument(
        "--min-finalized",
        type=int,
        default=1,
        help="require every validator's finalized height to be at least this (default 1)",
    )
    ver.set_defaults(func=cmd_verify)

    sig = sub.add_parser("sign", help="attest a recorded manifest with the validators' keys")
    sig.add_argument("manifest")
    sig.add_argument(
        "--keys-dir",
        default="",
        help="directory of validator-N.suri files (default: beside the recorded spec)",
    )
    sig.add_argument("--out", default="", help="where to write the signed manifest (default: in place)")
    sig.set_defaults(func=cmd_sign)

    att = sub.add_parser(
        "attest", help="add one operator's signature to a manifest (the ceremony form)"
    )
    att.add_argument("manifest")
    att.add_argument(
        "--key",
        required=True,
        help="the operator's ed25519 seed in hex (the seed its GRANDPA authority is derived from)",
    )
    att.add_argument("--out", default="", help="where to write the manifest (default: in place)")
    att.set_defaults(func=cmd_attest)

    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
