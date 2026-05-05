#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPORT_FILE="$ROOT_DIR/reports/swarm_report.md"
API_URL="http://127.0.0.1:8787"

mkdir -p "$ROOT_DIR/reports"

echo "# X3 Swarm Report" > "$REPORT_FILE"
echo >> "$REPORT_FILE"
echo "- API path: crates/x3-swarm-core/services/x3-swarm-api" >> "$REPORT_FILE"
echo "- Worker path: crates/x3-swarm-core/services/x3-swarm-worker" >> "$REPORT_FILE"
echo "- Memory path: data/agent-memory" >> "$REPORT_FILE"
echo "- Generated at: $(date -u +'%Y-%m-%dT%H:%M:%SZ')" >> "$REPORT_FILE"
echo >> "$REPORT_FILE"
echo "## Swarm Health Summary" >> "$REPORT_FILE"

if command -v curl >/dev/null 2>&1 && curl -fsS "$API_URL/health" >/dev/null 2>&1; then
  echo "- API /health: ok" >> "$REPORT_FILE"
  echo "- API URL: $API_URL" >> "$REPORT_FILE"
  echo >> "$REPORT_FILE"
  echo "## API Snapshot" >> "$REPORT_FILE"
  echo '```json' >> "$REPORT_FILE"
  curl -fsS "$API_URL/status" >> "$REPORT_FILE"
  echo >> "$REPORT_FILE"
  echo '```' >> "$REPORT_FILE"
  echo >> "$REPORT_FILE"
  echo "### Current Tasks" >> "$REPORT_FILE"
  echo '```json' >> "$REPORT_FILE"
  curl -fsS "$API_URL/tasks" >> "$REPORT_FILE"
  echo >> "$REPORT_FILE"
  echo '```' >> "$REPORT_FILE"
  echo >> "$REPORT_FILE"
  echo "### Memory Entries" >> "$REPORT_FILE"
  echo '```json' >> "$REPORT_FILE"
  curl -fsS "$API_URL/memory" >> "$REPORT_FILE"
  echo >> "$REPORT_FILE"
  echo '```' >> "$REPORT_FILE"
  echo >> "$REPORT_FILE"
  echo "### Recent Events" >> "$REPORT_FILE"
  echo '```json' >> "$REPORT_FILE"
  curl -fsS "$API_URL/events" >> "$REPORT_FILE"
  echo >> "$REPORT_FILE"
  echo '```' >> "$REPORT_FILE"
else
  echo "- API /health: unavailable" >> "$REPORT_FILE"
  echo "- Unable to collect live swarm state. Please start x3-swarm-api and rerun this script." >> "$REPORT_FILE"
fi

echo >> "$REPORT_FILE"
echo "## Local Scan" >> "$REPORT_FILE"
echo "- Generated file list from scripts/swarm/swarm_scan.sh" >> "$REPORT_FILE"
echo >> "$REPORT_FILE"
find "$ROOT_DIR" -type f | grep -E '\.(rs|sh|toml|md)$' | head -50 >> "$REPORT_FILE" || true

echo "X3 swarm report written to $REPORT_FILE"
