#!/bin/bash
# Phase 5 Complete Execution Launcher
# Runs all three concurrent Phase 5 tasks: Settlement Testing | Indexer | Monitoring

set -e

WORKSPACE="/home/lojak/Desktop/X3_ATOMIC_STAR"
LOG_DIR="/tmp/x3-testnet-logs"
mkdir -p "$LOG_DIR"

echo "╔════════════════════════════════════════════════════════════════╗"
echo "║    PHASE 5 - COMPLETE PARALLEL EXECUTION LAUNCHER             ║"
echo "║  🔴 5a: Settlement E2E | 🟡 5b: Indexer | 🟢 5c: Monitoring   ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo

# Kill any existing Phase 5 processes
echo "🧹 Cleaning up any existing Phase 5 processes..."
pkill -f "p4_p5_production_release" 2>/dev/null || true
pkill -f "x3-indexer" 2>/dev/null || true
sleep 2
echo "✅ Cleanup complete"
echo

# ===== PHASE 5a: Settlement Flow E2E Tests =====
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🔴 [Phase 5a] Settlement Flow E2E Testing"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Command: python3 p4_p5_production_release.py --validators 3 --testnet-enabled"
echo "Logging to: $LOG_DIR/settlement-tests.log"
echo "Starting in 3 seconds..."
sleep 3
cd "$WORKSPACE/tests_phase4"
timeout 900 python3 p4_p5_production_release.py --validators 3 --testnet-enabled \
  2>&1 | tee "$LOG_DIR/settlement-tests.log" &
SETTLEMENT_PID=$!
echo "✅ Started (PID: $SETTLEMENT_PID)"
echo

# ===== PHASE 5b: Indexer Build & Deployment =====
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🟡 [Phase 5b] X3 Indexer Build & Deployment"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Location: crates/x3-indexer"
echo "Logging to: $LOG_DIR/indexer.log"
echo "Target: Release binary for :4000"
echo "Starting in 10 seconds (after Phase 5a started)..."
sleep 10

cd "$WORKSPACE/crates/x3-indexer"

# Build indexer
echo "🔨 Building X3 Indexer..."
timeout 600 cargo build --release 2>&1 | tee "$LOG_DIR/indexer-build.log"
echo "✅ Build complete"

# Deploy indexer
echo
echo "🚀 Deploying X3 Indexer on :4000..."
timeout 600 ./target/release/x3-indexer \
  --listen 0.0.0.0:4000 \
  --rpc-urls http://127.0.0.1:9933 \
             http://127.0.0.1:9934 \
             http://127.0.0.1:9935 \
  2>&1 | tee -a "$LOG_DIR/indexer.log" &
INDEXER_PID=$!
echo "✅ Indexer deployed (PID: $INDEXER_PID)"
echo

# ===== PHASE 5c: Live Monitoring =====
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🟢 [Phase 5c] Real-Time Block Production Monitoring"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Tracking: Validator states, block height, GRANDPA finality"
echo "Log display: Real-time tail of validator logs"
echo

# Start monitoring loop
(
  ITERATION=0
  while true; do
    ITERATION=$((ITERATION + 1))
    TIMESTAMP=$(date '+%Y-%m-%d %H:%M:%S')
    
    echo "━━━ [$ITERATION] $TIMESTAMP ━━━"
    echo
    
    # Check settlement tests
    if [ -f "$LOG_DIR/settlement-tests.log" ]; then
      LINES=$(wc -l < "$LOG_DIR/settlement-tests.log")
      if grep -q "test result: ok" "$LOG_DIR/settlement-tests.log" 2>/dev/null; then
        PASSED=$(grep -c "test result: ok" "$LOG_DIR/settlement-tests.log" 2>/dev/null || echo "0")
        echo "✅ SETTLEMENT TESTS: $PASSED tests passed"
      elif grep -q "FAILED\|Error\|error" "$LOG_DIR/settlement-tests.log" 2>/dev/null; then
        echo "❌ SETTLEMENT TESTS: Errors detected"
      else
        echo "🔄 SETTLEMENT TESTS: Running... ($LINES lines)"
      fi
    else
      echo "⏳ SETTLEMENT TESTS: Starting..."
    fi
    echo
    
    # Check indexer
    if pgrep -f "x3-indexer" > /dev/null 2>&1; then
      echo "✅ INDEXER: Running on :4000"
      curl -s http://127.0.0.1:4000/graphql -X POST \
        -H "Content-Type: application/json" \
        -d '{"query":"{ __typename }"}' > /dev/null 2>&1 && echo "   GraphQL: Responsive" || echo "   GraphQL: Pending..."
    elif [ -f "$LOG_DIR/indexer-build.log" ] && grep -q "Finished" "$LOG_DIR/indexer-build.log" 2>/dev/null; then
      echo "🚀 INDEXER: Build complete, starting deployment..."
    else
      echo "🔨 INDEXER: Building..."
    fi
    echo
    
    # Display validator states
    echo "📊 VALIDATOR CONSENSUS STATE:"
    for i in {1,2,3}; do
      LOG="$LOG_DIR/validator$i.log"
      if [ -f "$LOG" ]; then
        LATEST=$(tail -1 "$LOG" 2>/dev/null)
        if [[ $LATEST == *"Idle"* ]]; then
          # Extract peer count and block height
          PEERS=$(echo "$LATEST" | grep -oP '(?<=\()\d+(?= peers)' || echo "?")
          BLOCK=$(echo "$LATEST" | grep -oP '#\d+' | head -1 || echo "#?")
          FINALIZED=$(echo "$LATEST" | grep -oP 'finalized \#\d+' || echo "finalized #?")
          echo "   Val-$i: Peers=$PEERS, $BLOCK, $FINALIZED ✅"
        else
          echo "   Val-$i: $LATEST" | cut -c 1-80
        fi
      else
        echo "   Val-$i: No logs yet"
      fi
    done
    echo
    
    # Check if settlement tests completed
    if [ -f "$LOG_DIR/settlement-tests.log" ] && grep -q "test result:" "$LOG_DIR/settlement-tests.log"; then
      FINAL_RESULT=$(tail -5 "$LOG_DIR/settlement-tests.log" | grep "test result:" | tail -1)
      if [[ "$FINAL_RESULT" == *"ok"* ]]; then
        echo "🎉 SETTLEMENT TESTS PASSED! $FINAL_RESULT"
      else
        echo "⚠️  SETTLEMENT TESTS COMPLETED. $FINAL_RESULT"
      fi
      echo
      echo "═══════════════════════════════════════════════════════════════"
      break
    fi
    
    # Wait before next update
    sleep 15
  done
) &
MONITOR_PID=$!
echo "✅ Monitoring started (PID: $MONITOR_PID)"
echo

# ===== Wait for Completion =====
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "⏳ All Phase 5 tasks launched!"
echo "   🔴 Settlement tests: Running (timeout: 15 min)"
echo "   🟡 Indexer: Building & deploying"
echo "   🟢 Monitoring: Live display every 15 seconds"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo

# Wait for settlement tests to complete (primary blocker)
echo "🔴 Waiting for Phase 5a (Settlement Tests) to complete..."
wait $SETTLEMENT_PID 2>/dev/null || true
echo "✅ Phase 5a complete!"
echo

# Wait for monitoring to complete
sleep 5
kill $MONITOR_PID 2>/dev/null || true

# ===== Summary Report =====
echo
echo "╔════════════════════════════════════════════════════════════════╗"
echo "║              PHASE 5 EXECUTION SUMMARY                        ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo

# Settlement Results
echo "📋 PHASE 5a - Settlement Flow E2E Testing:"
if [ -f "$LOG_DIR/settlement-tests.log" ]; then
  PASSED=$(grep -c "test result: ok" "$LOG_DIR/settlement-tests.log" 2>/dev/null || echo "0")
  FAILED=$(grep -c "test result: FAILED" "$LOG_DIR/settlement-tests.log" 2>/dev/null || echo "0")
  TOTAL=$((PASSED + FAILED))
  echo "   ✅ Passed: $PASSED"
  echo "   ❌ Failed: $FAILED"
  echo "   📊 Total: $TOTAL"
  if [ "$FAILED" -eq 0 ] && [ "$PASSED" -gt 0 ]; then
    echo "   🎉 STATUS: ✅ ALL TESTS PASSED"
  elif [ "$FAILED" -gt 0 ]; then
    echo "   ⚠️  STATUS: ❌ SOME TESTS FAILED"
  else
    echo "   ⏳ STATUS: Tests may not have executed"
  fi
else
  echo "   ⚠️  No settlement test results found"
fi
echo

# Indexer Results
echo "🔧 PHASE 5b - X3 Indexer Deployment:"
if [ -f "$LOG_DIR/indexer-build.log" ] && grep -q "Finished" "$LOG_DIR/indexer-build.log"; then
  echo "   ✅ Build: Successful"
  BINARY_SIZE=$(du -h "$WORKSPACE/crates/x3-indexer/target/release/x3-indexer" 2>/dev/null | cut -f1)
  echo "   📦 Binary Size: $BINARY_SIZE"
  if pgrep -f "x3-indexer" > /dev/null 2>&1; then
    echo "   🚀 Deployment: Running on :4000"
  else
    echo "   ⏳ Deployment: Binary ready, needs manual startup"
  fi
  echo "   Command: ./crates/x3-indexer/target/release/x3-indexer --listen 0.0.0.0:4000 --rpc-urls http://127.0.0.1:9933"
else
  echo "   🔨 Build: In progress or pending"
fi
echo

# Validator Network State
echo "🌐 PHASE 5c - Validator Network State:"
RUNNING=$(pgrep -f "x3-chain-node.*--validator" 2>/dev/null | wc -l)
echo "   Validators Running: $RUNNING/3"
for i in {1,2,3}; do
  LOG="$LOG_DIR/validator$i.log"
  if [ -f "$LOG" ]; then
    LATEST=$(tail -1 "$LOG" 2>/dev/null)
    if [[ $LATEST == *"Idle"* ]]; then
      PEERS=$(echo "$LATEST" | grep -oP '(?<=\()\d+(?= peers)' || echo "?")
      BLOCK=$(echo "$LATEST" | grep -oP '#\d+' | head -1 || echo "#?")
      echo "   ✅ Validator-$i: $BLOCK, $PEERS peer(s) connected"
    fi
  fi
done
echo

# Log File Locations
echo "📂 Artifact Locations:"
echo "   Settlement Tests: $LOG_DIR/settlement-tests.log"
echo "   Indexer Build: $LOG_DIR/indexer-build.log"
echo "   Indexer Runtime: $LOG_DIR/indexer.log"
echo "   Validator Logs: $LOG_DIR/validator{1,2,3}.log"
echo

# Next Steps
echo "🚀 Next Actions:"
echo "   1. Verify Phase 5a test results:"
echo "      tail -50 $LOG_DIR/settlement-tests.log | grep -E 'PASS|FAIL|ok'"
echo
echo "   2. Start/check indexer (if not running):"
echo "      cd $WORKSPACE/crates/x3-indexer"
echo "      ./target/release/x3-indexer --listen 0.0.0.0:4000 --rpc-urls http://127.0.0.1:9933"
echo
echo "   3. Verify indexer GraphQL:"
echo "      curl http://127.0.0.1:4000/graphql -X POST -H 'Content-Type: application/json' -d '{\"query\":\"{ __typename }\"}''"
echo
echo "   4. Monitor block production:"
echo "      watch -n 2 'tail -1 $LOG_DIR/validator1.log'"
echo
echo "   5. Check cross-VM bridge status:"
echo "      curl -s http://127.0.0.1:9933 -X POST -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"chain_getLatestHeader\",\"params\":[],\"id\":1}' | jq"
echo

echo "═══════════════════════════════════════════════════════════════"
echo "✨ Phase 5 Execution Complete! Ready for Phase 6 planning."
echo "═══════════════════════════════════════════════════════════════"
