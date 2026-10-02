#!/usr/bin/env python3
"""Pin executable skip detection and comment false positives in both scan paths."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("source_scan", ROOT / "scripts/x3_fake_code_scan.py")
scanner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(scanner)


class CheatScannerTests(unittest.TestCase):
    def findings(self, fallback=False):
        ignore = "#[" + "ignore]"
        skipped = "test.sk" + "ip('disabled')"
        xit = "x" + "it('disabled')"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            files = {
                "tests/live.rs": ignore + "\n#[test]\nfn live() { assert_eq!(actual, expected); }\n",
                "tests/feature.ts": skipped + "\n",
                "tests/feature.py": "# " + skipped + "\n" + xit + "\n",
                "scripts/check.sh": "# " + ignore + " is a Rust test attribute\n# " + skipped + "\n",
            }
            for path, content in files.items():
                destination = root / path
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_text(content)
            with patch.object(scanner, "REPO_ROOT", root):
                if fallback:
                    with patch.object(scanner.shutil, "which", return_value=None):
                        return scanner.scan_cheats()
                return scanner.scan_cheats()

    def test_hash_comments_are_not_skipped_tests(self):
        findings = self.findings()
        self.assertEqual([(f["kind"], f["path"]) for f in findings], [
            ("skip", "tests/feature.py"), ("skip", "tests/feature.ts"), ("skip", "tests/live.rs")])

    def test_python_fallback_matches_rg_detection(self):
        self.assertEqual(self.findings(), self.findings(fallback=True))


if __name__ == "__main__":
    unittest.main()
