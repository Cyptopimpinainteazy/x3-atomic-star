#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# runtime_upgrade_rehearsal.sh — the testnet-side entry point for the rehearsal.
#
# This wrapper reports the rehearsal, so an absent rehearsal is a failure, not a
# printed message followed by exit 0. It used to end with a hand-off line when the
# delegate was missing or non-executable, which is exactly how a skipped upgrade
# rehearsal would have been read as a passing step inside both RC gates.
#
# The rehearsal itself lives in `scripts/mainnet/runtime_upgrade_rehearsal.sh`.
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DELEGATE="$ROOT_DIR/scripts/mainnet/runtime_upgrade_rehearsal.sh"

echo "== X3 Testnet Runtime Upgrade Rehearsal =="

if [ ! -f "$DELEGATE" ]; then
  echo "FAILED: the runtime upgrade rehearsal is missing: $DELEGATE" >&2
  exit 1
fi

if [ ! -x "$DELEGATE" ]; then
  echo "FAILED: the runtime upgrade rehearsal is not executable: $DELEGATE" >&2
  echo "  chmod +x it (a rehearsal that cannot run is not a passing step)" >&2
  exit 1
fi

exec "$DELEGATE" "$@"
