#!/bin/bash
# Configure firewall on X3 Chain testnet nodes

set -e

NODE_TYPE="${1:-validator}"  # validator, rpc, bootnode, or monitoring
ADMIN_IP="${ADMIN_IP:-0.0.0.0/0}"  # Restrict SSH to admin IP

echo "🔒 Configuring firewall for $NODE_TYPE node..."

# Update system
sudo apt-get update
sudo apt-get install -y ufw

# Default policies
sudo ufw default deny incoming
sudo ufw default allow outgoing

# SSH access (restrict to admin IP in production!)
sudo ufw allow from "$ADMIN_IP" to any port 22 proto tcp

# Common: P2P port for all nodes
sudo ufw allow 30333/tcp comment 'X3 P2P'

# Node-type specific rules
case "$NODE_TYPE" in
    validator)
        echo "Validator: Opening RPC port (localhost only)"
        # RPC only accessible locally
        ;;
    rpc)
        echo "RPC Node: Opening public RPC port"
        sudo ufw allow 9944/tcp comment 'X3 RPC'
        ;;
    bootnode)
        echo "Bootnode: P2P only (already configured)"
        ;;
    monitoring)
        echo "Monitoring: Opening Prometheus and Grafana ports"
        sudo ufw allow 9090/tcp comment 'Prometheus'
        sudo ufw allow 3000/tcp comment 'Grafana'
        ;;
esac

# Metrics port (accessible from monitoring server only)
# TODO: Restrict to monitoring server IP
sudo ufw allow 9615/tcp comment 'Prometheus metrics'

# Enable firewall
sudo ufw --force enable

# Show status
sudo ufw status verbose

echo "✅ Firewall configured for $NODE_TYPE node"
