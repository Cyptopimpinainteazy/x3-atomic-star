#!/usr/bin/env bash
# YOLO Optimizer Runner
# Orchestrates iterative YOLO optimization rounds with artifact collection.
# Usage: tools/yolo_run.sh [max_rounds (default 10)]

set -euo pipefail

MAX_ROUNDS="${1:-10}"
TIMESTAMP=$(date +%Y%m%dT%H%M%S)
OUTDIR="bench-results/${TIMESTAMP}"
mkdir -p "$OUTDIR"

echo "=========================================="
echo "  X3 YOLO Optimizer Runner"
echo "  Timestamp: $TIMESTAMP"
echo "  Max Rounds: $MAX_ROUNDS"
echo "=========================================="

# Build
echo ""
echo "[1/3] Building x3-opt, x3-bench..."
cargo build -p x3-opt -p x3-bench -p x3-backend --release 2>&1 | tail -5

# Baseline
echo ""
echo "[2/3] Running baseline..."
cargo run -p x3-bench --release -- --baseline --out "$OUTDIR/baseline" 2>&1 | grep -E "gas:|Complete" || true

# Extract baseline gas (or set to 0 if file doesn't exist)
if [ -f "$OUTDIR/baseline/report.json" ]; then
    PREV_GAS=$(jq -r '.global.gas' "$OUTDIR/baseline/report.json" 2>/dev/null || echo "0")
    echo "Baseline gas: $PREV_GAS"
else
    PREV_GAS="0"
    echo "Baseline report not found; starting at gas=0"
fi

STREAK_NO_IMPROVE=0

echo ""
echo "[3/3] Running YOLO rounds..."

for i in $(seq 1 "$MAX_ROUNDS"); do
    echo ""
    echo "=== YOLO Round $i / $MAX_ROUNDS ==="
    ROUNDDIR="$OUTDIR/round-$i"
    mkdir -p "$ROUNDDIR"

    # Run YOLO pass
    if cargo run -p x3-bench --release -- --yolo --out "$ROUNDDIR" 2>&1 | grep -E "gas:|Complete" || true; then
        :
    fi

    # Extract new gas
    if [ -f "$ROUNDDIR/report.json" ]; then
        NEW_GAS=$(jq -r '.global.gas' "$ROUNDDIR/report.json" 2>/dev/null || echo "$PREV_GAS")
        echo "Round $i gas: $PREV_GAS → $NEW_GAS"

        if (( $(echo "$NEW_GAS < $PREV_GAS" | bc -l 2>/dev/null || echo "0") )); then
            STREAK_NO_IMPROVE=0
            echo "  ✓ Improvement"
        else
            STREAK_NO_IMPROVE=$((STREAK_NO_IMPROVE + 1))
            echo "  ✗ No improvement (streak: $STREAK_NO_IMPROVE)"
        fi
        PREV_GAS="$NEW_GAS"
    else
        echo "Round $i report not found"
        STREAK_NO_IMPROVE=$((STREAK_NO_IMPROVE + 1))
    fi

    if [ "$STREAK_NO_IMPROVE" -ge 3 ]; then
        echo ""
        echo "No improvement streak >= 3, stopping..."
        break
    fi
done

echo ""
echo "=========================================="
echo "  Aggregating Results..."
echo "=========================================="

# Aggregate into summary.json
python3 - "$OUTDIR" <<'PYEOF'
import json
import glob
import os
import sys

base = sys.argv[1]
agg = {"rounds": []}

# Add baseline
if os.path.exists(os.path.join(base, "baseline", "report.json")):
    with open(os.path.join(base, "baseline", "report.json")) as f:
        agg["rounds"].append({"stage": "baseline", "metrics": json.load(f)["global"]})

# Add rounds in order
for r in sorted(glob.glob(base + "/round-*")):
    rnum = int(r.split("-")[-1])
    if os.path.exists(os.path.join(r, "report.json")):
        with open(os.path.join(r, "report.json")) as f:
            agg["rounds"].append({"stage": f"round-{rnum}", "metrics": json.load(f)["global"]})

# Write summary
with open(os.path.join(base, "summary.json"), "w") as f:
    json.dump(agg, f, indent=2)

print(f"✓ Wrote {os.path.join(base, 'summary.json')}")
print(f"✓ {len(agg['rounds'])} stage(s) recorded")
PYEOF

# Print summary
if [ -f "$OUTDIR/summary.json" ]; then
    echo ""
    echo "Summary:"
    python3 - "$OUTDIR/summary.json" <<'PYEOF2'
import json, sys
with open(sys.argv[1]) as f:
    data = json.load(f)
for stage in data["rounds"]:
    m = stage["metrics"]
    print(f"  {stage['stage']:20s}: gas={m['gas']:6d} instr={m['instr']:4d} bytes={m['bytes']:5d}")
PYEOF2
fi

echo ""
echo "=========================================="
echo "✓ DONE. Results in: $OUTDIR"
echo "=========================================="
