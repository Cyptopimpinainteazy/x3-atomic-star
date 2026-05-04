#!/usr/bin/env bash
set -euo pipefail

# aggregate-sigill-telemetry.sh
# Usage: ./aggregate-sigill-telemetry.sh [--days N] [--min-runs N]

DAYS=7
MIN_RUNS=1
OUT_DIR="artifacts"
SUMMARY_JSON="${OUT_DIR}/sigils-fallback-summary.json"
REPO="${GITHUB_REPOSITORY:-Cyptopimpinainteazy/x3-chain}"

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --days) DAYS="$2"; shift 2;;
    --min-runs) MIN_RUNS="$2"; shift 2;;
    -h|--help) echo "Usage: $0 [--days N] [--min-runs N]"; exit 0;;
    *) echo "Unknown arg: $1"; exit 2;;
  esac
done

mkdir -p /tmp/sigill-telemetry || true
cd /tmp/sigill-telemetry
# write artifacts to a local path inside the temp dir so the script can run both locally and in CI
OUT_DIR="$PWD/artifacts"
mkdir -p "$OUT_DIR"
SINCE=$(date -u -d "$DAYS days ago" +%Y-%m-%dT%H:%M:%SZ)
echo "Looking for workflow runs since: $SINCE"

# fetch recent runs of the sigill-triage-integration workflow
gh api -H "Accept: application/vnd.github+json" "/repos/$REPO/actions/workflows/sigill-triage-integration.yml/runs?per_page=100" > runs.json

FOUND=0
jq -r --arg since "$SINCE" '.workflow_runs[] | select(.created_at > $since) | .id' runs.json | while read -r run_id; do
  echo "Checking run $run_id"
  gh api "/repos/$REPO/actions/runs/$run_id/artifacts" > art_${run_id}.json
  artifact_id=$(jq -r '.artifacts[] | select(.name=="sigill-fallback-telemetry") | .id' art_${run_id}.json || true)
  archive_url=$(jq -r '.artifacts[] | select(.name=="sigill-fallback-telemetry") | .archive_download_url' art_${run_id}.json || true)
  if [ -n "$artifact_id" ] && [ "$artifact_id" != "null" ]; then
    echo "Found artifact $artifact_id for run $run_id"
    TOKEN=$(gh auth token)
    curl -L -H "Authorization: token $TOKEN" -o artifact_${artifact_id}.zip "$archive_url"
    if unzip -Z1 artifact_${artifact_id}.zip | grep -iq "fallback-telemetry.json"; then
      unzip -p artifact_${artifact_id}.zip "fallback-telemetry.json" > telemetry_run_${run_id}.json || true
    else
      file=$(unzip -Z1 artifact_${artifact_id}.zip | grep -i '\.json$' | head -n1)
      if [ -n "$file" ]; then
        unzip -p artifact_${artifact_id}.zip "$file" > telemetry_run_${run_id}.json || true
      fi
    fi
    echo "Saved telemetry for run $run_id"
    FOUND=1
  fi
done

if [ -z "$(ls telemetry_run_*.json 2>/dev/null || true)" ]; then
  echo "No telemetry files found in last ${DAYS} days; writing empty summary"
  jq -n --arg wd "$DAYS" --arg gen "$(date --utc +%Y-%m-%dT%H:%M:%SZ)" '{total_runs_with_telemetry:0, fallback_count:0, fallback_rate:0.0, avg_attempts:0.0, window_days:($wd|tonumber), generated_at:$gen}' > "$SUMMARY_JSON"
  echo "Summary saved to $SUMMARY_JSON"
  exit 0
fi

jq -s --arg wd "$DAYS" --arg gen "$(date --utc +%Y-%m-%dT%H:%M:%SZ)" '{
  total_runs_with_telemetry: length,
  fallback_count: (map(select(.fallback_used=="true")) | length),
  fallback_rate: (if length == 0 then 0 else ((map(select(.fallback_used=="true")) | length) / length) end),
  avg_attempts: ( (map(.fallback_attempts | tonumber) | if length==0 then 0 else (add/length) end) ),
  window_days: ($wd|tonumber),
  generated_at: $gen,
  runs: map({run_id: .run_id, pr: .pr, fallback_used: .fallback_used, fallback_attempts: .fallback_attempts, timestamp: .timestamp})
}' telemetry_run_*.json > "$SUMMARY_JSON"

echo "Summary saved to $SUMMARY_JSON"
cat "$SUMMARY_JSON" | jq -C .

# simple check for alert
TOTAL=$(jq '.total_runs_with_telemetry' "$SUMMARY_JSON")
FALLBACK_RATE=$(jq '.fallback_rate' "$SUMMARY_JSON")
if [ "$TOTAL" -ge "$MIN_RUNS" ] && (( $(echo "$FALLBACK_RATE > 0.05" | bc -l) )); then
  echo "Alert condition met: fallback_rate=${FALLBACK_RATE}, total=${TOTAL}"
else
  echo "No alert (fallback_rate=${FALLBACK_RATE}, total=${TOTAL})"
fi

exit 0
