#!/usr/bin/env bash
# X3 repo scanner — entry point.
#
# This used to dump every `TODO|FIXME|unwrap(` line plus a list of filenames whose
# name contained a subsystem word: no severity, no symbol, no fix, no gate, no
# test, and nothing ran it. The implementation now lives in
# `scripts/swarm/x3_repo_scan.py`, which emits a finding schema (severity, exact
# file and symbol, why it matters, the fix, the test that would prove the fix,
# the gate that catches a regression) to both Markdown and JSON.
#
# Usage:
#   scripts/swarm/swarm_scan.sh                 # write reports/ (citations only)
#   scripts/swarm/swarm_scan.sh --check         # exit 1 when a ratcheted count grew
#   scripts/swarm/swarm_scan.sh --patches       # also write .ai/patches/*.patch
#   scripts/swarm/swarm_scan.sh --json          # findings JSON on stdout
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
exec python3 "$ROOT_DIR/scripts/swarm/x3_repo_scan.py" --root "$ROOT_DIR" "$@"
