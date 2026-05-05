#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
API_URL="http://127.0.0.1:8787"

usage() {
  cat <<EOF
Usage: $0 <command> [task-id]
Commands:
  list               List current swarm tasks from the API
  status <task-id>   Show a single task status
  approve <task-id>  Approve a manual task for worker processing
  reject <task-id>   Reject a manual task

Example:
  $0 approve x3-task-0001
EOF
  exit 1
}

if [ "$#" -lt 1 ]; then
  usage
fi

command="$1"
shift || true

task_id="${1:-}"

if [ "$command" != "list" ] && [ -z "$task_id" ]; then
  echo "ERROR: task-id is required for '$command'"
  usage
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "ERROR: curl is required to talk to the swarm API"
  exit 1
fi

case "$command" in
  list)
    curl -fsS "$API_URL/tasks" | python3 -c 'import json,sys; tasks=json.load(sys.stdin); print("TASK_ID\tSTATUS\tAPPROVAL\tTITLE")
for t in tasks:
    print("{}\t{}\t{}\t{}".format(t["id"], t["status"], t["approval_required"], t["title"]))'
    ;;
  status)
    curl -fsS "$API_URL/tasks/$task_id" | python3 -c 'import json,sys; t=json.load(sys.stdin); print(json.dumps(t, indent=2))'
    ;;
  approve)
    curl -fsS -X POST "$API_URL/tasks/$task_id/approve" | python3 -c 'import json,sys; print(json.dumps(json.load(sys.stdin), indent=2))'
    ;;
  reject)
    curl -fsS -X POST "$API_URL/tasks/$task_id/reject" | python3 -c 'import json,sys; print(json.dumps(json.load(sys.stdin), indent=2))'
    ;;
  *)
    echo "ERROR: unknown command '$command'"
    usage
    ;;
esac
