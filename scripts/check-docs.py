#!/usr/bin/env python3
"""Check local link targets in maintained docs; archived claims are not validated."""
import argparse
import json
from pathlib import Path
import re
import sys
from urllib.parse import unquote, urlsplit

REQUIRED = (
    "README.md", "WIKI.md", "LAUNCH_SCOPE.md", "CURRENT_MAINNET_STATUS.md",
    "RELEASE_GATES.md", "docs/README.md", "docs/archive/README.md",
    "proving/README.md", "crates/x3-chain-health-daemon/README.md",
)
GUIDES = ("architecture", "development", "operations", "verification", "release", "current")

def check(root, paths):
    errors = []
    checked = 0
    for relative in paths:
        document = root / relative
        if not document.is_file():
            errors.append({"document": relative, "error": "document missing"})
            continue
        text = re.sub(r"(?ms)^\x60\x60\x60.*?^\x60\x60\x60[^\n]*", "", document.read_text())
        for match in re.finditer(r"!?\[[^\]]*\]\(([^)]+)\)", text):
            destination = match.group(1).strip()
            if not destination:
                continue
            if destination.startswith("<"):
                destination = destination[1:destination.index(">")]
            else:
                destination = destination.split()[0]
            parsed = urlsplit(destination)
            if parsed.scheme or parsed.netloc or not parsed.path:
                continue
            target = (document.parent / unquote(parsed.path)).resolve()
            checked += 1
            if not target.is_relative_to(root):
                errors.append({"document": relative, "target": destination, "error": "target outside repository"})
            elif not target.exists():
                errors.append({"document": relative, "target": destination, "error": "target missing"})
    return {"ok": not errors, "documents": len(paths), "local_targets_checked": checked,
            "errors": errors, "scope": "maintained local file links; excludes archived contents, external URLs and anchors"}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--paths", nargs="+")
    args = parser.parse_args()
    root = args.root.resolve()
    paths = args.paths or sorted(set(REQUIRED) | {
        str(p.relative_to(root)) for guide in GUIDES for p in (root / "docs" / guide).rglob("*.md")
    })
    result = check(root, paths)
    print(json.dumps(result, indent=2))
    return 0 if result["ok"] else 1

if __name__ == "__main__":
    sys.exit(main())
