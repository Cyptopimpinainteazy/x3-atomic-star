#!/bin/bash
# Start X3 Chain node in dev mode (ports 9933 HTTP RPC, 9944 WS RPC)

set -e

NODE_BIN="${NODE_BIN:-./target/release/x3-chain-node}"
LOG_DIR="${LOG_DIR:-/tmp/x3-chain-logs}"

if [ ! -f "$NODE_BIN" ]; then
  echo "❌ Node binary not found: $NODE_BIN"
  echo "Run: cargo build -p x3-chain-node --release"
  exit 1
fi

mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/x3-chain-node.log"

echo "🚀 Starting X3 Chain node in dev mode..."
echo "   HTTP RPC: http://localhost:9933"
echo "   WS RPC:   ws://localhost:9944"
echo "   Logs: $LOG_FILE"
echo ""

exec "$NODE_BIN" \
  --dev \
  --rpc-port=9933 \
  --ws-port=9944 \
  --rpc-methods=unsafe \
  --ws-max-connections=2000 \
  --alice \
  --tmp \
  2>&1 | tee "$LOG_FILE"
