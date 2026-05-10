#!/bin/bash
# Start Cloudflare tunnel to expose local X3 services to x3star.net

TUNNEL_ID="6c118620-18cf-4795-80a8-6d44d37aecaa"
TUNNEL_NAME="atlas-sphere"
CONFIG_FILE="$HOME/.cloudflared/config.yml"

echo "🌐 Starting Cloudflare tunnel: $TUNNEL_NAME"
echo "   Tunnel ID: $TUNNEL_ID"
echo "   Config: $CONFIG_FILE"
echo ""
echo "Routing:"
echo "  ws.x3star.net  → localhost:9944 (WebSocket RPC)"
echo "  rpc.x3star.net → localhost:9933 (HTTP RPC)"
echo "  x3star.net     → localhost:5173 (Desktop app)"
echo ""

exec cloudflared tunnel run "$TUNNEL_NAME"
