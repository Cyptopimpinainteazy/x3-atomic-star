#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
API_URL="http://127.0.0.1:8787"

TASK_QUEUE='[
  {
    "title": "Audit core runtime path guard",
    "feature": "swarm-forbidden-path",
    "agent": "swarm-guard",
    "permission_tier": "constrained",
    "allowed_paths": ["crates/x3-swarm-core/src", "crates/x3-swarm-core/services/x3-swarm-api/src"],
    "forbidden_paths": ["./.git", "secrets/", "node_modules/"],
    "required_commands": ["cargo test -p x3-swarm-core -- --nocapture"],
    "approval_required": "manual",
    "risk": "medium"
  },
  {
    "title": "Collect agent memory snapshot",
    "feature": "memory-store",
    "agent": "swarm-memory",
    "permission_tier": "read-only",
    "allowed_paths": ["data/agent-memory"],
    "forbidden_paths": ["./.git", "*secret*", "*.key"],
    "required_commands": ["cat data/agent-memory/*.jsonl"],
    "approval_required": "auto",
    "risk": "low"
  },
  {
    "title": "Generate swarm report",
    "feature": "swarm-report",
    "agent": "swarm-analyst",
    "permission_tier": "read-only",
    "allowed_paths": ["reports/", "scripts/swarm/", "crates/x3-swarm-core/services/x3-swarm-api/src"],
    "forbidden_paths": ["./.git", "secrets/", "node_modules/"],
    "required_commands": ["scripts/swarm/swarm_report.sh"],
    "approval_required": "auto",
    "risk": "low"
  }
]'

if command -v curl >/dev/null 2>&1 && curl -fsS "$API_URL/health" >/dev/null 2>&1; then
  echo "API available at $API_URL. Syncing task queue to swarm API..."
  export TASK_QUEUE
  python3 - <<'PY'
import json, urllib.request, os
api = os.environ.get('API_URL', 'http://127.0.0.1:8787')
tasks = json.loads(os.environ['TASK_QUEUE'])
results = []
for task in tasks:
    req = urllib.request.Request(api + '/tasks', data=json.dumps(task).encode('utf-8'), headers={'Content-Type': 'application/json'})
    with urllib.request.urlopen(req) as resp:
        results.append(json.loads(resp.read().decode('utf-8')))
print(json.dumps(results, indent=2))
PY
else
  echo "$TASK_QUEUE"
  echo "
Swarm API unavailable; generated task queue is output only. Run scripts/swarm/swarm_up.sh first to synchronize tasks." >&2
fi
