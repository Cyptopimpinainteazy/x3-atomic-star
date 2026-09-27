#!/usr/bin/env bash
set -euo pipefail
# x3-detect-test-cheats.sh — bounded test-cheat detection with a ratchet.
#
# Same shape as scripts/x3-detect-stubs.sh: the path is named by AGENTS.md, the
# git hooks and scripts/x3-proof-check.sh, and the scan itself is
# scripts/x3_fake_code_scan.py (cheats mode). See its docstring for the four
# shapes detected and the ones deliberately left to scripts/test_cheat_guard.py.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$SCRIPT_DIR/x3_fake_code_scan.py" cheats "$@"
