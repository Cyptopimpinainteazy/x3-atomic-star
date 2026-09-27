#!/usr/bin/env bash
set -euo pipefail
# x3-detect-stubs.sh — bounded stub detection with a ratchet.
#
# `AGENTS.md` names this path, and so do scripts/x3-pre-task.sh,
# scripts/x3-install-git-hooks.sh and scripts/x3-proof-check.sh, so the path
# stays. The scan lives in scripts/x3_fake_code_scan.py: this wrapper used to
# *be* the scan and it walked build output and vendored trees, so it never
# finished (exit 124 at the 240 s timeout) and the check that `AGENTS.md`
# mandates read as satisfied while it had never run. That script's docstring
# says which shapes are detected and which are deliberately not.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$SCRIPT_DIR/x3_fake_code_scan.py" stubs "$@"
