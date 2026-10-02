#!/usr/bin/env python3
"""Exercise the launcher's shell orchestration without starting chain processes.

Only the test harness replaces start_node/curl; these tests prove launch selection,
not block production, connectivity, or finality.
"""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/testnet/run-7-validators-local.sh"


class ValidatorLauncherTests(unittest.TestCase):
    def launch_selection(self, index):
        source = SCRIPT.read_text()
        footer = source[source.index('echo "Starting node 1 (bootnode)..."'):]
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "starts"
            harness = '''set -euo pipefail
start_node() { echo "$1" >> "$START_LOG"; }
curl() { echo '{"result":"test-peer"}'; }
COUNT=7
RPC_BASE=9944
P2P_BASE=30333
LISTEN_IP=127.0.0.1
LOG_DIR=/unused
PID_DIR=/unused
'''
            result = subprocess.run(["bash", "-c", harness + footer],
                                    env={**os.environ, "ONLY_INDEX": str(index), "START_LOG": str(log)},
                                    text=True, capture_output=True, timeout=5)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            return log.read_text().splitlines()

    def test_full_launch_starts_each_validator_once(self):
        self.assertEqual(self.launch_selection(0), [str(i) for i in range(1, 8)])

    def test_restart_other_validator_does_not_launch_bootnode(self):
        self.assertEqual(self.launch_selection(3), ["3"])

    def test_restart_bootnode_starts_it_only_once(self):
        self.assertEqual(self.launch_selection(1), ["1"])

    def test_restart_with_wipe_is_rejected_before_touching_data(self):
        with tempfile.TemporaryDirectory() as directory:
            sentinel = Path(directory) / "preserve"
            sentinel.write_text("existing state")
            result = subprocess.run(["bash", str(SCRIPT), "--only", "3", "--wipe",
                                     "--base-dir", directory], text=True, capture_output=True, timeout=5)
            self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
            self.assertIn("cannot be combined", result.stderr)
            self.assertEqual(sentinel.read_text(), "existing state")

    def test_invalid_restart_index_is_rejected_before_creating_directories(self):
        for index in ("8", "-1", "abc"):
            with self.subTest(index=index), tempfile.TemporaryDirectory() as directory:
                base = Path(directory) / "untouched"
                result = subprocess.run(["bash", str(SCRIPT), "--only", index,
                                         "--base-dir", str(base)],
                                        text=True, capture_output=True, timeout=5)
                self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                self.assertFalse(base.exists())


if __name__ == "__main__":
    unittest.main()
