#!/usr/bin/env python3
"""Tests for `scripts/mainnet/launch_record.py`.

The launch record is what makes a launch decision re-readable: it names the operator, the commit,
the RC log that passed, and the hash of each genesis artifact. The cases that matter are the ones
where it must *refuse* — a spec that is not a mainnet spec, a spec whose authority set is not the
validator set being launched, an RC log that does not carry the gate's own finishing marker, a
missing operator, a dirty tree — and the one that comes after: a record whose genesis has changed
underneath it must fail `verify`.

    python3 tests/test_launch_record.py
"""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "mainnet" / "launch_record.py"

MARKER = "== X3 MAINNET RC GATE PASSED =="


def spec(authorities: int = 7, chain_type: str = "Live", bootnodes: int = 1) -> dict:
    return {
        "name": "X3 Chain Production",
        "id": "x3_chain_production",
        "chainType": chain_type,
        "bootNodes": [
            f"/dns4/node{i}.example/tcp/30333/p2p/12D3KooWMTqqXaRcNsqDE7kxXKy83YU66gHkQFTdkk4GMYQVm4Gn"
            for i in range(bootnodes)
        ],
        "genesis": {
            "runtimeGenesis": {
                "config": {
                    "aura": {"authorities": ["5" + "D" * 46] * authorities},
                    "grandpa": {"authorities": [["5" + "H" * 46, 1]] * authorities},
                }
            }
        },
    }


class LaunchRecord(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = pathlib.Path(self._tmp.name)
        self.spec = self.root / "chain-specs/x3-mainnet-plain.json"
        self.raw = self.root / "chain-specs/x3-mainnet-raw.json"
        self.log = self.root / "reports/mainnet_rc_gate.log"
        self.out = self.root / "reports/launch/promotion.json"
        self.write_spec()
        raw = spec()
        raw["genesis"] = {"raw": {"top": {}}}
        self.raw.parent.mkdir(parents=True, exist_ok=True)
        self.raw.write_text(json.dumps(raw), encoding="utf-8")
        self.log.parent.mkdir(parents=True, exist_ok=True)
        self.log.write_text(f"checks…\n{MARKER}\n", encoding="utf-8")

    def write_spec(self, **kwargs: object) -> None:
        self.spec.parent.mkdir(parents=True, exist_ok=True)
        self.spec.write_text(json.dumps(spec(**kwargs)), encoding="utf-8")

    def run_record(self, *extra: str, command: str = "build") -> subprocess.CompletedProcess:
        argv = ["python3", str(SCRIPT), "--root", str(self.root), command]
        if command == "build":
            argv += [
                "--spec",
                str(self.spec),
                "--raw",
                str(self.raw),
                "--rc-log",
                str(self.log),
                "--commit",
                "deadbeef",
                "--approved-by",
                "operator",
                "--out",
                str(self.out),
            ]
        else:
            argv += ["--record", str(self.out)]
        return subprocess.run(
            argv + list(extra), capture_output=True, text=True, timeout=120
        )

    def test_a_complete_promotion_is_recorded(self) -> None:
        result = self.run_record()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        record = json.loads(self.out.read_text())
        self.assertEqual(record["approved_by"], "operator")
        self.assertEqual(record["commit"], "deadbeef")
        self.assertEqual(record["expect_authorities"], 7)
        self.assertEqual(sorted(a["role"] for a in record["artifacts"]), ["mainnet-plain", "mainnet-raw"])
        self.assertTrue(record["artifacts"][0]["sha256"])
        self.assertEqual(record["rc_gate"]["passed_marker"], MARKER)

    def test_a_spec_for_a_different_validator_set_is_refused(self) -> None:
        self.write_spec(authorities=3)
        result = self.run_record()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Aura authorities", result.stderr)
        self.assertFalse(self.out.exists(), "a refused promotion must not leave a record")

    def test_a_non_live_spec_is_refused(self) -> None:
        self.write_spec(chain_type="Local")
        result = self.run_record()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not 'Live'", result.stderr)

    def test_an_rc_log_without_the_marker_is_refused(self) -> None:
        self.log.write_text("checks…\nFAILED: something\n", encoding="utf-8")
        result = self.run_record()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("PASSED", result.stderr)

    def test_a_missing_operator_is_refused(self) -> None:
        result = self.run_record("--approved-by", "   ")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("operator", result.stderr)

    def test_a_missing_artifact_is_refused(self) -> None:
        self.raw.unlink()
        result = self.run_record()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("raw spec not found", result.stderr)

    def test_a_dirty_tree_is_refused(self) -> None:
        repo = self.root / "git-tree"
        repo.mkdir()
        (repo / "tracked.txt").write_text("original\n", encoding="utf-8")
        env = {"GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@example.com",
               "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@example.com"}
        for argv in (
            ["git", "init", "-q"],
            ["git", "add", "tracked.txt"],
            ["git", "commit", "-q", "-m", "init"],
        ):
            subprocess.run(argv, cwd=repo, env=env, check=True, capture_output=True)
        (repo / "tracked.txt").write_text("changed\n", encoding="utf-8")
        result = self.run_record()
        self.assertEqual(result.returncode, 0, "the fixture tree outside git is fine")

        dirty = subprocess.run(
            ["python3", str(SCRIPT), "--root", str(repo), "build",
             "--spec", str(self.spec), "--raw", str(self.raw), "--rc-log", str(self.log),
             "--approved-by", "operator", "--out", str(repo / "out.json")],
            capture_output=True, text=True, timeout=120,
        )
        self.assertNotEqual(dirty.returncode, 0)
        self.assertIn("working tree is dirty", dirty.stderr)

    def test_verify_detects_a_changed_genesis(self) -> None:
        self.assertEqual(self.run_record().returncode, 0)
        self.assertEqual(self.run_record(command="verify").returncode, 0)
        self.write_spec(authorities=7, bootnodes=2)  # same shape, different bytes
        result = self.run_record(command="verify")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("sha256", result.stderr)

    def test_verify_detects_a_changed_rc_log(self) -> None:
        self.assertEqual(self.run_record().returncode, 0)
        self.log.write_text(f"{MARKER}\nplus a line nobody recorded\n", encoding="utf-8")
        result = self.run_record(command="verify")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("RC log has changed", result.stderr)

    def test_the_record_is_cited_by_the_launch_gate_row(self) -> None:
        registry = (ROOT / "FEATURE_REGISTRY.toml").read_text(encoding="utf-8")
        row = registry.split("[launch_gate]", 1)[1].split("\n[", 1)[0]
        self.assertIn("launch_record.py", row)


def main() -> int:
    loader = unittest.TestLoader()
    suite = loader.loadTestsFromTestCase(LaunchRecord)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())
