#!/bin/bash
# Status check and control for X3 chain infrastructure

case "${1:-status}" in
  status)
    echo "=== X3 Chain Infrastructure Status ==="
    echo ""
    echo "Node Service:"
    systemctl --user status x3-chain-node.service --no-pager || echo "  (stopped)"
    echo ""
    echo "Tunnel Service:"
    systemctl --user status cloudflared-tunnel.service --no-pager || echo "  (stopped)"
    echo ""
    echo "RPC Health Check:"
    curl -s http://localhost:9933 -d '{"id":1,"jsonrpc":"2.0","method":"system_name","params":[]}' \
      -H 'Content-Type: application/json' 2>/dev/null | jq -r '.result // "❌ Unavailable"' 2>/dev/null || echo "  ❌ Unavailable"
    ;;
  start)
    echo "Starting X3 services..."
    systemctl --user start x3-chain-node.service
    sleep 3
    systemctl --user start cloudflared-tunnel.service
    echo "✓ Services started"
    ;;
  stop)
    echo "Stopping X3 services..."
    systemctl --user stop cloudflared-tunnel.service
    systemctl --user stop x3-chain-node.service
    echo "✓ Services stopped"
    ;;
  restart)
    systemctl --user restart x3-chain-node.service
    sleep 3
    systemctl --user restart cloudflared-tunnel.service
    echo "✓ Services restarted"
    ;;
  logs)
    echo "=== Node Logs ==="
    journalctl --user -u x3-chain-node.service -n 20 -f
    ;;
  *)
    echo "Usage: $0 {status|start|stop|restart|logs}"
    exit 1
    ;;
esac
