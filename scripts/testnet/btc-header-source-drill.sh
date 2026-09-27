#!/usr/bin/env bash
# Does the BTC header relayer follow a real public chain — and refuse a lying source?
#
# Companion to `btc-header-push-drill.sh` (which needs a Bitcoin Core install and a dev node) and
# to `btc-checkpoint-genesis-drill.sh` (which proves a chain can be *born* anchored). This one
# needs neither: it talks to a public Esplora endpoint and proves the *source* half of the
# relayer — that `scripts/btc/push-headers.py` reads consecutive real headers, checks the run
# links, emits the `submitBtcHeaders` payload, and refuses a run whose source tampered with one
# answer.
#
# It needs the network. Without it the drill **skips loudly** and verifies nothing — it does not
# pass.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

exec python3 "$ROOT/scripts/testnet/btc-header-source-drill.py" "$@"
