#!/usr/bin/env python3
"""Tests for the release-candidate gates.

Two gates decide whether a testnet or a mainnet candidate may go out:

    scripts/testnet/testnet_rc_gate.sh
    scripts/mainnet/mainnet_rc_gate.sh

Neither had ever been checked by anything. One of them ran every single check as
`... || true` and finished by printing "COMPLETED" rather than "PASSED", so it
exited 0 whatever happened; the other named a chain-spec generator that was never
created in this repository, so under `set -e` it exited 127 on its third line and
could not pass. `scripts/x3/yolo_autoprove.sh` runs the testnet one.

These tests drive the real scripts. Each gate is copied byte-for-byte into a
fixture root together with stand-in prerequisites, so the gate resolves its own
`ROOT_DIR` inside the fixture and every check it runs is one the test chooses to
pass or fail. Nothing here replaces the gates themselves: it proves that a
failing prerequisite reddens the gate, that a missing prerequisite is a failure
rather than a skip, and that every path either gate names exists in the tree.

    python3 tests/test_rc_gates.py
"""

from __future__ import annotations

import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]

GATES = {
    "testnet": ROOT / "scripts" / "testnet" / "testnet_rc_gate.sh",
    "mainnet": ROOT / "scripts" / "mainnet" / "mainnet_rc_gate.sh",
}

GENESIS_LINT = ROOT / "scripts" / "testnet" / "testnet_genesis_lint.sh"
REHEARSAL_WRAPPER = ROOT / "scripts" / "testnet" / "runtime_upgrade_rehearsal.sh"
SPEC_GENERATOR = ROOT / "scripts" / "testnet" / "generate_testnet_chain_spec.sh"

PREREQ_ARRAY = re.compile(
    r"^[A-Z]+_RC_PREREQS=\((.*?)^\)", re.MULTILINE | re.DOTALL
)
PREREQ_ENTRY = re.compile(r"^\s*([A-Za-z0-9_./-]+\.(?:sh|py))\s*$", re.MULTILINE)
SCRIPT_INVOCATION = re.compile(r"^\s*(?:\./|bash |python3 )?(scripts/\S+\.(?:sh|py))", re.MULTILINE)
SWALLOWED_CHECK = re.compile(
    r"^\s*(?:\./|bash |python3 )?scripts/\S+\.(?:sh|py)\b.*\|\|\s*true", re.MULTILINE
)


def read(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8")


def declared_prerequisites(script: pathlib.Path) -> list[str]:
    match = PREREQ_ARRAY.search(read(script))
    if not match:
        return []
    return PREREQ_ENTRY.findall(match.group(1))


def invoked_scripts(script: pathlib.Path) -> list[str]:
    return SCRIPT_INVOCATION.findall(read(script))


def write_executable(path: pathlib.Path, body: str) -> pathlib.Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")
    path.chmod(0o755)
    return path


class GateFixture:
    """A fixture root holding a byte-identical gate and its prerequisites."""

    def __init__(self, tmp: str, gate: str):
        self.gate = gate
        self.source = GATES[gate]
        self.root = pathlib.Path(tmp) / gate
        self.relpath = self.source.relative_to(ROOT)
        self.bin = pathlib.Path(tmp) / f"{gate}-bin"

        self.script = write_executable(
            self.root / self.relpath, read(self.source)
        )
        for rel in declared_prerequisites(self.source):
            self.pass_prereq(rel)
        # The gates shell out to cargo for the formatting and test steps.
        write_executable(self.bin / "cargo", "#!/usr/bin/env bash\nexit 0\n")

    def pass_prereq(self, rel: str, body: str = "#!/usr/bin/env bash\necho ok\nexit 0\n") -> None:
        write_executable(self.root / rel, body)

    def fail_prereq(self, rel: str) -> None:
        write_executable(self.root / rel, "#!/usr/bin/env bash\necho 'this check failed' >&2\nexit 1\n")

    def fail_cargo(self) -> None:
        write_executable(self.bin / "cargo", "#!/usr/bin/env bash\nexit 1\n")

    def run(self) -> subprocess.CompletedProcess:
        env = dict(os.environ)
        env["PATH"] = f"{self.bin}{os.pathsep}{env.get('PATH', '')}"
        env.pop("X3_NODE_BIN", None)
        return subprocess.run(
            ["bash", str(self.script)],
            cwd=str(self.root),
            env=env,
            capture_output=True,
            text=True,
            timeout=300,
        )


class GatePrerequisitesExist(unittest.TestCase):
    def test_every_prerequisite_the_gates_name_exists(self) -> None:
        for gate, script in GATES.items():
            for rel in declared_prerequisites(script):
                self.assertTrue(
                    (ROOT / rel).exists(),
                    f"{gate}: the gate names {rel}, which is not in the tree",
                )

    def test_every_script_a_gate_runs_is_a_declared_prerequisite(self) -> None:
        for gate, script in GATES.items():
            declared = set(declared_prerequisites(script))
            for rel in invoked_scripts(script):
                self.assertIn(
                    rel,
                    declared,
                    f"{gate}: runs {rel} without declaring it as a prerequisite",
                )

    def test_no_gate_check_is_swallowed(self) -> None:
        for gate, script in GATES.items():
            body = read(script)
            swallowed = SWALLOWED_CHECK.findall(body)
            self.assertEqual(
                swallowed,
                [],
                f"{gate}: these checks cannot fail because their status is discarded: {swallowed}",
            )

    def test_the_spec_generator_the_gates_call_is_real(self) -> None:
        text = read(SPEC_GENERATOR)
        self.assertIn("build-x3-testnet-spec.py", text)
        self.assertTrue((ROOT / "scripts/testnet/build-x3-testnet-spec.py").is_file())


class GatesFailClosed(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)

    def test_a_gate_passes_when_every_prerequisite_passes(self) -> None:
        for gate in GATES:
            fixture = GateFixture(self._tmp.name, gate)
            result = fixture.run()
            self.assertEqual(result.returncode, 0, f"{gate}: {result.stdout}{result.stderr}")
            self.assertIn("PASSED", result.stdout)

    def test_a_gate_fails_when_any_prerequisite_fails(self) -> None:
        for gate, script in GATES.items():
            for rel in declared_prerequisites(script):
                with self.subTest(gate=gate, prerequisite=rel):
                    fixture = GateFixture(self._tmp.name, gate)
                    fixture.fail_prereq(rel)
                    result = fixture.run()
                    self.assertNotEqual(result.returncode, 0, f"{gate}: {rel} failing did not redden the gate")
                    self.assertIn("FAILED:", result.stdout + result.stderr)

    def test_a_gate_fails_when_a_prerequisite_is_missing(self) -> None:
        for gate, script in GATES.items():
            rel = declared_prerequisites(script)[0]
            with self.subTest(gate=gate, prerequisite=rel):
                fixture = GateFixture(self._tmp.name, gate)
                (fixture.root / rel).unlink()
                result = fixture.run()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("do not exist", result.stderr)
                self.assertIn(rel, result.stderr)

    def test_a_gate_fails_when_a_cargo_check_fails(self) -> None:
        for gate in GATES:
            with self.subTest(gate=gate):
                fixture = GateFixture(self._tmp.name, gate)
                fixture.fail_cargo()
                result = fixture.run()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("FAILED:", result.stdout + result.stderr)

    def test_the_testnet_gate_no_longer_reports_success_for_a_broken_chain(self) -> None:
        """The defect this suite was written for: `|| true` on every check."""
        fixture = GateFixture(self._tmp.name, "testnet")
        for rel in declared_prerequisites(GATES["testnet"]):
            fixture.fail_prereq(rel)
        fixture.fail_cargo()
        self.assertNotEqual(fixture.run().returncode, 0)


class GenesisLintBehaviour(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = pathlib.Path(self._tmp.name)
        self.script = write_executable(
            self.root / GENESIS_LINT.relative_to(ROOT), read(GENESIS_LINT)
        )
        self.node = write_executable(
            pathlib.Path(self._tmp.name) / "node", "#!/usr/bin/env bash\nexit 0\n"
        )

    def write_spec(self, spec: dict, name: str = "x3-testnet-plain.json") -> pathlib.Path:
        directory = self.root / "chain-specs"
        directory.mkdir(exist_ok=True)
        path = directory / name
        path.write_text(json.dumps(spec), encoding="utf-8")
        return path

    @staticmethod
    def spec(authorities: int = 3, chain_type: str = "Live") -> dict:
        return {
            "name": "X3 Chain Testnet",
            "id": "x3_chain_testnet",
            "chainType": chain_type,
            "bootNodes": ["/ip4/127.0.0.1/tcp/30333/p2p/12D3KooWMTqqXaRcNsqDE7kxXKy83YU66gHkQFTdkk4GMYQVm4Gn"],
            "genesis": {
                "runtimeGenesis": {
                    "config": {
                        "aura": {"authorities": ["5" + "D" * 46] * authorities},
                        "grandpa": {"authorities": [["5" + "H" * 46, 1]] * authorities},
                    }
                }
            },
        }

    def run_lint(self, *args: str, node: pathlib.Path | None = None) -> subprocess.CompletedProcess:
        env = dict(os.environ)
        env["X3_NODE_BIN"] = str(node or self.node)
        return subprocess.run(
            ["bash", str(self.script), *args],
            cwd=str(self.root),
            env=env,
            capture_output=True,
            text=True,
            timeout=120,
        )

    def test_a_valid_spec_passes(self) -> None:
        self.write_spec(self.spec())
        result = self.run_lint()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("PASSED", result.stdout)

    def test_a_spec_with_no_authorities_is_refused(self) -> None:
        self.write_spec(self.spec(authorities=0))
        result = self.run_lint()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Aura", result.stderr)

    def test_a_live_spec_with_no_bootnode_is_refused(self) -> None:
        spec = self.spec()
        spec["bootNodes"] = []
        self.write_spec(spec)
        result = self.run_lint()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("bootNodes", result.stderr)

    def test_the_node_loader_is_the_authority(self) -> None:
        self.write_spec(self.spec())
        refusing_node = write_executable(
            pathlib.Path(self._tmp.name) / "refusing-node", "#!/usr/bin/env bash\nexit 1\n"
        )
        result = self.run_lint(node=refusing_node)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("refused", result.stderr)

    def test_an_absent_spec_is_not_a_passing_lint(self) -> None:
        result = self.run_lint()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("no chain spec to lint", result.stderr)


class RehearsalWrapperBehaviour(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = pathlib.Path(self._tmp.name)
        self.script = write_executable(
            self.root / REHEARSAL_WRAPPER.relative_to(ROOT), read(REHEARSAL_WRAPPER)
        )

    def run_wrapper(self) -> subprocess.CompletedProcess:
        return subprocess.run(
            ["bash", str(self.script)],
            cwd=str(self.root),
            capture_output=True,
            text=True,
            timeout=120,
        )

    def test_a_missing_rehearsal_is_a_failure(self) -> None:
        result = self.run_wrapper()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing", result.stderr)

    def test_a_non_executable_rehearsal_is_a_failure(self) -> None:
        delegate = self.root / "scripts/mainnet/runtime_upgrade_rehearsal.sh"
        delegate.parent.mkdir(parents=True, exist_ok=True)
        delegate.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
        delegate.chmod(0o644)
        result = self.run_wrapper()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not executable", result.stderr)

    def test_an_executable_rehearsal_runs(self) -> None:
        write_executable(
            self.root / "scripts/mainnet/runtime_upgrade_rehearsal.sh",
            "#!/usr/bin/env bash\necho rehearsal ran\nexit 0\n",
        )
        result = self.run_wrapper()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("rehearsal ran", result.stdout)


class RegistryCitations(unittest.TestCase):
    """The readiness row for the launch gate must cite these functions."""

    def test_launch_gate_cites_resolvable_gate_functions(self) -> None:
        registry = read(ROOT / "FEATURE_REGISTRY.toml")
        row = re.search(r"^\[launch_gate\]\n(.*?)(?=^\[|\Z)", registry, re.MULTILINE | re.DOTALL)
        self.assertIsNotNone(row, "FEATURE_REGISTRY.toml has no [launch_gate] row")
        body = row.group(1)

        target = re.search(r'^crate_or_service\s*=\s*"([^"]+)"', body, re.MULTILINE)
        self.assertIsNotNone(target)
        self.assertTrue((ROOT / target.group(1)).exists(), f"{target.group(1)} is not in the tree")

        citations = re.search(r"^required_tests\s*=\s*\[(.*?)\]", body, re.MULTILINE | re.DOTALL)
        self.assertIsNotNone(citations, "[launch_gate] cites no tests")
        names = re.findall(r'"([^"]+)"', citations.group(1))
        self.assertTrue(names, "[launch_gate] cites no tests")

        for citation in names:
            with self.subTest(citation=citation):
                self.assertIn("::", citation, "a citation whose target is a script must be `path::function`")
                path_text, function = citation.rsplit("::", 1)
                path = ROOT / path_text
                self.assertTrue(path.is_file(), f"{path_text} does not exist")
                self.assertRegex(
                    read(path),
                    rf"(?m)^\s*{re.escape(function)}\s*\(\)\s*\{{",
                    f"{path_text} does not define `{function}()`",
                )


def main() -> int:
    if not shutil.which("bash"):
        print("SKIP: bash is not on PATH")
        return 0
    loader = unittest.TestLoader()
    suite = unittest.TestSuite(
        loader.loadTestsFromTestCase(case)
        for case in (
            GatePrerequisitesExist,
            GatesFailClosed,
            GenesisLintBehaviour,
            RehearsalWrapperBehaviour,
            RegistryCitations,
        )
    )
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())
