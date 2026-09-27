#!/usr/bin/env python3
"""Tests for the ceremony manifest's operator attestations.

The sign/verify pair in `scripts/testnet/testnet-ceremony.py` is the only thing
that turns a locally produced record into a published one, so the interesting
cases are the refusals: a signature that does not verify, a key that is not an
authority, the same authority signing twice, a manifest below its own threshold,
a manifest that lowers its own threshold, and a manifest whose two accounts of
its own authority set disagree.

The live half (a real four-validator network, signed and then tampered with six
ways) is `scripts/testnet/testnet-ceremony-drill.sh`. This file is the part that
has to run in seconds on every commit.

    python3 tests/test_ceremony_attestations.py
"""

from __future__ import annotations

import importlib.util
import json
import pathlib
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
CEREMONY = ROOT / "scripts" / "testnet" / "testnet-ceremony.py"


def load_ceremony():
    spec = importlib.util.spec_from_file_location("x3_ceremony", CEREMONY)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


ceremony = load_ceremony()

# The key the node derives from the seed `bytes(range(32))`, read back from
# `x3-chain-node keys generate --key-type grandpa --seed 0x000102…1f --output hex`.
# Pinned so that a change to the derivation path in this script is caught here
# rather than by an operator whose manifest no longer verifies.
SEED = bytes(range(32))
SEED_PUBKEY = "03a107bff3ce10be1d70dd18e74bc09967e4d6309ba50d5f1ddc8664125531b8"
# A real SS58 address from a generated testnet spec, and the key it commits to.
SAMPLE_SS58 = "5EUdt1uuqwG2JNgdFzuQ1HPLPtyWa8RTge2HWuRfeNe31Nmr"
SAMPLE_PUBKEY = bytes.fromhex("6ab8f83a0d86f884fb439c8c2b40d99b4599d18701e3505af74d293dff18e570")
# The SS58 address of `SEED_PUBKEY` (default network prefix 42), so a fixture can
# name the same authority on both the chain side and the spec side — which is what
# `authority_keys` requires before it will hand back a set.
SEED_SS58 = "5C9TqVEs5Q51zUdjvp6tSsjHAfLseJKq1Dq6ULcuSLqEuRSf"


def scaled_authorities(keys: list[bytes]) -> str:
    """`Vec<(AuthorityId, u64)>` the way `GrandpaApi_grandpa_authorities` returns it."""
    body = b"".join(key + (1).to_bytes(8, "little") for key in keys)
    return "0x" + (bytes([len(keys) << 2]) + body).hex()


def manifest_with(keys: list[bytes], spec_addresses: list[str] | None = None) -> dict:
    return {
        "manifest_version": 1,
        "chain": "X3 Chain Testnet",
        "validators": [{"grandpa_authorities_scaled": scaled_authorities(keys)}],
        "authorities_expected_from_spec": {
            "aura": [],
            "grandpa": spec_addresses if spec_addresses is not None else [SAMPLE_SS58],
        },
    }


class Decoding(unittest.TestCase):
    def test_base58_decodes_leading_zeroes(self) -> None:
        self.assertEqual(ceremony.base58_decode("1"), b"\x00")
        self.assertEqual(ceremony.base58_decode("11"), b"\x00\x00")
        self.assertEqual(ceremony.base58_decode("2"), b"\x01")

    def test_base58_refuses_a_character_outside_the_alphabet(self) -> None:
        for bad in ("0", "O", "I", "l", "!"):
            with self.assertRaises(ValueError):
                ceremony.base58_decode(bad)

    def test_ss58_decodes_the_key_the_address_commits_to(self) -> None:
        self.assertEqual(ceremony.ss58_pubkey(SAMPLE_SS58), SAMPLE_PUBKEY)

    def test_ss58_refuses_a_wrong_checksum(self) -> None:
        # Same address with one character changed: still base58, still 35 bytes,
        # and the checksum no longer matches.
        tampered = SAMPLE_SS58[:-1] + ("m" if SAMPLE_SS58[-1] != "m" else "n")
        with self.assertRaises(ValueError):
            ceremony.ss58_pubkey(tampered)

    def test_grandpa_authorities_decode(self) -> None:
        keys = [SAMPLE_PUBKEY, bytes(32)]
        self.assertEqual(ceremony.decode_grandpa_authorities(scaled_authorities(keys)), keys)

    def test_a_truncated_authority_set_is_refused(self) -> None:
        raw = ceremony.decode_grandpa_authorities  # aliased for the line below
        with self.assertRaises(ValueError):
            raw("0x" + (bytes([2 << 2]) + SAMPLE_PUBKEY).hex())
        with self.assertRaises(ValueError):
            raw("0x")

    def test_the_threshold_is_the_workspace_rule(self) -> None:
        # floor(2n/3) + 1 — the same rule as
        # `x3_validator_attestation::supermajority_threshold`. The four- and
        # seven-validator bars are the ones the operator will actually meet.
        self.assertEqual([ceremony.supermajority_threshold(n) for n in range(1, 8)], [1, 2, 3, 3, 4, 5, 5])
        self.assertEqual(ceremony.supermajority_threshold(4), 3)
        self.assertEqual(ceremony.supermajority_threshold(7), 5)


class Signatures(unittest.TestCase):
    def test_the_derivation_matches_the_nodes_own(self) -> None:
        self.assertEqual(ceremony.ed25519_pubkey_from_seed(SEED).hex(), SEED_PUBKEY)

    def test_a_signature_verifies_and_nothing_else_does(self) -> None:
        pubkey = ceremony.ed25519_pubkey_from_seed(SEED)
        signature = ceremony.ed25519_sign(SEED, b"the manifest")
        self.assertTrue(ceremony.ed25519_verify(pubkey, b"the manifest", signature))
        self.assertFalse(ceremony.ed25519_verify(pubkey, b"the manifesT", signature))
        flipped = bytearray(signature)
        flipped[0] ^= 0x01
        self.assertFalse(ceremony.ed25519_verify(pubkey, b"the manifest", bytes(flipped)))
        self.assertFalse(ceremony.ed25519_verify(bytes(32), b"the manifest", signature))
        self.assertFalse(ceremony.ed25519_verify(pubkey, b"the manifest", b"short"))

    def test_a_short_seed_is_refused(self) -> None:
        with self.assertRaises(ValueError):
            ceremony.ed25519_sign(b"\x00" * 31, b"x")

    def test_the_canonical_bytes_ignore_the_attestations_member(self) -> None:
        manifest = manifest_with([SAMPLE_PUBKEY])
        unsigned = ceremony.canonical_manifest_bytes(manifest)
        manifest["attestations"] = {"scheme": "ed25519", "authorities": []}
        self.assertEqual(ceremony.canonical_manifest_bytes(manifest), unsigned)
        # ... and key order on disk does not matter either.
        reordered = dict(reversed(list(manifest.items())))
        self.assertEqual(ceremony.canonical_manifest_bytes(reordered), unsigned)
        # ... but a change to a signed field does.
        manifest["chain"] = "somewhere else"
        self.assertNotEqual(ceremony.canonical_manifest_bytes(manifest), unsigned)


class AuthoritySets(unittest.TestCase):
    def test_a_manifest_whose_two_authority_accounts_agree_is_accepted(self) -> None:
        self.assertEqual(ceremony.authority_keys(manifest_with([SAMPLE_PUBKEY])), [SAMPLE_PUBKEY])

    def test_a_manifest_whose_accounts_disagree_is_refused(self) -> None:
        with self.assertRaises(ValueError):
            ceremony.authority_keys(manifest_with([bytes(range(1, 33))]))

    def test_a_repeated_authority_key_is_refused(self) -> None:
        with self.assertRaises(ValueError):
            ceremony.authority_keys(manifest_with([SAMPLE_PUBKEY, SAMPLE_PUBKEY]))

    def test_a_manifest_that_is_not_json_is_the_callers_problem(self) -> None:
        # `authority_keys` reads three members; a manifest missing them raises a
        # KeyError, which `verify` turns into a named failure rather than a crash.
        with self.assertRaises(KeyError):
            ceremony.authority_keys({})


class VerifyAttestations(unittest.TestCase):
    """The verifier half, driven with a real signature over a real manifest body."""

    def setUp(self) -> None:
        self.lines: list[tuple[str, bool]] = []

        def check(name: str, ok: bool, detail: str = "") -> None:
            self.lines.append((name, ok))

        self.check = check
        # One authority, named by both accounts, whose private key the test has.
        self.authority = ceremony.ed25519_pubkey_from_seed(SEED)
        self.manifest = manifest_with([self.authority], [SEED_SS58])

    def sign(self, seed: bytes = SEED, key: bytes | None = None) -> None:
        key = ceremony.ed25519_pubkey_from_seed(seed) if key is None else key
        signature = ceremony.ed25519_sign(seed, ceremony.canonical_manifest_bytes(self.manifest))
        self.manifest["attestations"] = {
            "scheme": "ed25519",
            "authority_set_size": 1,
            "required": ceremony.supermajority_threshold(len([key])),
            "obtained": 1,
            "authorities": [
                {"index": 1, "public_key": "0x" + key.hex(), "signature": "0x" + signature.hex()}
            ],
        }

    def failures(self) -> list[str]:
        return [name for name, ok in self.lines if not ok]

    def test_a_valid_attestation_passes(self) -> None:
        self.sign()
        ceremony.verify_attestations(self.manifest, self.check)
        self.assertEqual(self.failures(), [])

    def test_a_missing_attestation_member_fails(self) -> None:
        ceremony.verify_attestations(self.manifest, self.check)
        self.assertIn("manifest carries operator attestations", self.failures())

    def test_a_foreign_signer_fails_even_with_a_valid_signature(self) -> None:
        foreign = bytes(range(64, 96))
        self.sign(seed=foreign, key=ceremony.ed25519_pubkey_from_seed(foreign))
        ceremony.verify_attestations(self.manifest, self.check)
        self.assertIn("attestation 0 (#1) is an authority key", self.failures())

    def test_a_lowered_threshold_fails(self) -> None:
        self.sign()
        self.manifest["attestations"]["required"] = 0
        ceremony.verify_attestations(self.manifest, self.check)
        self.assertTrue(
            any("workspace rule" in name for name in self.failures()), self.failures()
        )

    def test_a_duplicate_signer_does_not_count_twice(self) -> None:
        self.sign()
        entry = dict(self.manifest["attestations"]["authorities"][0], index=2)
        self.manifest["attestations"]["authorities"].append(entry)
        self.manifest["attestations"]["obtained"] = 2
        ceremony.verify_attestations(self.manifest, self.check)
        self.assertIn("attestation 1 (#2) is a distinct signer", self.failures())
        # One real signature counts once, however many copies the manifest carries,
        # and the manifest's own `obtained: 2` is not what the check reads.
        threshold_line = [
            (name, ok)
            for name, ok in self.lines
            if name.startswith("distinct authority signatures reached the threshold")
        ]
        self.assertEqual(len(threshold_line), 1)
        self.assertEqual(threshold_line[0][0].endswith("(1/1)"), True)
        self.assertTrue(threshold_line[0][1], "one distinct signature meets a one-of-one bar")


if __name__ == "__main__":
    unittest.main(verbosity=2 if "-v" in sys.argv else 1)
