#!/usr/bin/env bash
set -euo pipefail

# Resolve the checkout from this script's own location (scripts/systemd/ -> repo root) rather than
# a hardcoded desktop path that does not exist on this box.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT/x3fronend"
export PATH="/usr/local/bin:/usr/bin:/bin:$PATH"
export PORT=4174
# `simple-server.py` never existed in this tree; the served artifact is the Next static export in
# x3fronend/out (built by `npm run build`). Serve that with a server that is present on the box.
exec python3 -m http.server "$PORT" --directory out
