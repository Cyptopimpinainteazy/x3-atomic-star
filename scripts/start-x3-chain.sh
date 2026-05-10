#!/bin/bash
# Start mock Substrate RPC server for local development
# (Simulates X3 Chain node on ports 9933 HTTP, 9944 WebSocket)

set -e

LOG_DIR="${LOG_DIR:-/tmp/x3-chain-logs}"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/mock-rpc-server.log"

echo "🚀 Starting mock Substrate RPC server..."
echo "   HTTP RPC: http://localhost:9933"
echo "   WS RPC:   ws://localhost:9944"
echo "   Logs: $LOG_FILE"
echo ""
echo "   ℹ️  This is a mock server for development."
echo "   ℹ️  For production, run the real x3-chain-node binary."
echo ""

cd "$(dirname "$0")/.."
exec node scripts/mock-rpc-server.js 2>&1 | tee "$LOG_FILE"
