import json
from pathlib import Path
import subprocess
import tempfile
import unittest

CHECKER = Path(__file__).resolve().parents[1] / "check-docs.py"

class DocumentationLinksTests(unittest.TestCase):
    def run_check(self, markdown, files=()):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "README.md").write_text(markdown)
            for name in files:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("# Target\n")
            run = subprocess.run(
                ["python3", str(CHECKER), "--root", str(root), "--paths", "README.md"],
                capture_output=True, text=True, check=False,
            )
            return run.returncode, json.loads(run.stdout)

    def test_real_local_target_passes(self):
        code, result = self.run_check("[guide](docs/guide.md)", ["docs/guide.md"])
        self.assertEqual(code, 0)
        self.assertEqual(result["local_targets_checked"], 1)

    def test_empty_target_refers_to_current_document(self):
        code, result = self.run_check("[here]()")
        self.assertEqual(code, 0)
        self.assertEqual(result["local_targets_checked"], 0)

    def test_removed_target_fails(self):
        code, result = self.run_check("[guide](docs/missing.md)")
        self.assertEqual(code, 1)
        self.assertEqual(result["errors"][0]["error"], "target missing")

    def test_external_urls_do_not_require_network(self):
        code, result = self.run_check("[site](https://example.invalid/unavailable) [section](#heading)")
        self.assertEqual(code, 0)
        self.assertEqual(result["local_targets_checked"], 0)

    def test_target_outside_repository_is_refused(self):
        code, result = self.run_check("[parent](../outside.md)")
        self.assertEqual(code, 1)
        self.assertEqual(result["errors"][0]["error"], "target outside repository")

    def test_fenced_example_links_are_ignored(self):
        code, result = self.run_check("\x60\x60\x60markdown\n[example](missing.md)\n\x60\x60\x60\n")
        self.assertEqual(code, 0)
        self.assertEqual(result["local_targets_checked"], 0)

if __name__ == "__main__":
    unittest.main()
