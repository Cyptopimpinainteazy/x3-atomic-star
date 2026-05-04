# X3 Chain Configuration Guide

Complete reference for X3 Chain node configuration options and environment-based setup.

## Table of Contents

1. [Configuration Tiers](#configuration-tiers)
2. [Environment Variables](#environment-variables)
3. [Chain Specs](#chain-specs)
4. [Advanced Configuration](#advanced-configuration)
5. [Performance Tuning](#performance-tuning)
6. [Network Configuration](#network-configuration)

---

## Configuration Tiers

### Tier Comparison

| Feature | Development | Local Testnet | Staging | Production |
|---------|---|---|---|---|
| **Validators** | 1 (Alice) | 2 (Alice, Bob) | 3 (Alpha, Beta, Gamma) | 5+ (Network) |
| **Endowed Accounts** | 6 test | 6 test | 3 foundation | Genesis state |
| **WASM Binary** | Optional | Optional | Required | Required |
| **Chain Type** | Development | Local | Live | Live |
| **Block Time** | 200ms | 200ms | 200ms | 200ms |
| **Finality** | GRANDPA | GRANDPA | GRANDPA | GRANDPA/Flash |
| **Telemetry** | Off | Off | Optional | On |
| **Pruning** | Full | Full | Configurable | Archive |
| **Network Sync** | No | No | Yes | Yes |

### Development (`--chain=dev`)

**When to Use**:
- Local development
- Feature testing
- Runtime debugging
- CI/CD pipelines

**Configuration**:
```bash
x3-chain-node \
  --chain=dev \
  --dev \
  --tmp \
  --alice \
  --force-authoring \
  --unsafe-rpc-external \
  --unsafe-ws-external
```

**Database**: In-memory (temporary, cleared on exit)

**RPC Endpoints**:
- HTTP: `http://localhost:9933`
- WebSocket: `ws://localhost:9944`

### Local Testnet (`--chain=local`)

**When to Use**:
- Multi-validator testing
- Consensus validation
- Finality verification
- Cross-node communication

**Configuration (Node 1 - Alice)**:
```bash
x3-chain-node \
  --chain=local \
  --tmp \
  --alice \
  --node-key=0000000000000000000000000000000000000000000000000000000000000001 \
  --rpc-port=9933 \
  --ws-port=9944 \
  --port=30333
```

**Configuration (Node 2 - Bob)**:
```bash
x3-chain-node \
  --chain=local \
  --tmp \
  --bob \
  --node-key=0000000000000000000000000000000000000000000000000000000000000002 \
  --rpc-port=9934 \
  --ws-port=9945 \
  --port=30334 \
  --bootnodes=/ip4/127.0.0.1/tcp/30333/ws
```

**Database**: Single shared per node (temporary)

### Staging (`--chain=staging`)

**When to Use**:
- Pre-production validation
- Network parameter testing
- Load testing
- Security auditing

**Configuration (Validator)**:
```bash
x3-chain-node \
  --chain=staging \
  --name=validator-staging-1 \
  --validator \
  --node-key=<validator-private-key> \
  --pruning=1000 \
  --db=rocksdb \
  --database=rocksdb \
  --telemetry-url wss://telemetry.x3-chain.io/submit,0 \
  --prometheus-external
```

**Database**: RockDB (persistent)

**Bootstrap Nodes**:
- Configured via chainspec or `--bootnodes`

### Production (`--chain=production`)

**When to Use**:
- Mainnet validators
- Archive nodes
- Infrastructure nodes
- Long-term operation

**Configuration (Validator)**:
```bash
x3-chain-node \
  --chain=production \
  --name=validator-prod-1 \
  --validator \
  --node-key=<validator-private-key> \
  --pruning=archive \
  --db=rocksdb \
  --database=rocksdb \
  --telemetry-url wss://telemetry.x3-chain.io/submit,1 \
  --prometheus-external \
  --log=info \
  --max-runtime-instances=256
```

**Database**: RocksDB (persistent, not pruned)

---

## Environment Variables

### Primary Configuration

#### `CHAIN_SPEC`

Override the default chain specification:

```bash
# Use staging instead of dev
CHAIN_SPEC=staging x3-chain-node

# Use production
CHAIN_SPEC=production x3-chain-node

# Or via CLI (takes precedence)
x3-chain-node --chain=production
```

**Valid Values**: `dev`, `local`, `staging`, `production` or file path

### Node Operation

#### `X3_DEV_SEED`

Insert a development key seed into the node's keystore:

```bash
# Use custom development key
X3_DEV_SEED="//CustomValidator" x3-chain-node --dev --validator

# Default (if not set): Uses "//Alice" for dev chain
X3_DEV_SEED="//Bob" x3-chain-node --local --validator
```

#### `RUST_LOG`

Set logging verbosity:

```bash
# Detailed logging
RUST_LOG=debug x3-chain-node --dev

# Specific module logging
RUST_LOG="x3=debug,sc_consensus=trace" x3-chain-node --dev

# Disable noisy modules
RUST_LOG="info,libp2p=warn" x3-chain-node --dev
```

### System Configuration

#### `RUST_MIN_STACK`

Increase Rust stack size (needed on constrained systems):

```bash
# Default: Safe for most systems
RUST_MIN_STACK=16777216 x3-chain-node

# Increase on low-memory systems
RUST_MIN_STACK=33554432 x3-chain-node
```

#### `RAYON_NUM_THREADS`

Set number of parallel threads:

```bash
# Use 4 threads for parallel proposer
RAYON_NUM_THREADS=4 x3-chain-node --enable-parallel-proposer

# Auto-detect from CPU count (default)
x3-chain-node
```

### Telemetry

#### `ENABLE_TELEMETRY`

Global telemetry control:

```bash
# Enable (must also specify endpoint)
ENABLE_TELEMETRY=1 x3-chain-node --telemetry-url wss://telemetry.x3-chain.io/submit

# Disable
ENABLE_TELEMETRY=0 x3-chain-node
```

#### `TELEMETRY_ENDPOINT`

Telemetry server endpoint:

```bash
# Official
TELEMETRY_ENDPOINT=wss://telemetry.x3-chain.io/submit x3-chain-node

# Self-hosted
TELEMETRY_ENDPOINT=ws://localhost:1024/submit x3-chain-node
```

### Feature Flags

#### `FLASH_FINALITY_LIVE_MODE`

Enable Flash Finality for consensus (not just shadow):

```bash
# Shadow mode (default, testing only)
x3-chain-node --enable-flash-finality

# Live consensus (requires 2/3+ validator participation)
FLASH_FINALITY_LIVE_MODE=1 x3-chain-node --enable-flash-finality
```

#### `POH_ENFORCED`

Enforce PoH validity (default: audit only):

```bash
# Audit mode (default): log validation results
x3-chain-node --enable-poh

# Enforced mode: reject invalid PoH blocks
POH_ENFORCED=1 x3-chain-node --enable-poh
```

---

## Chain Specs

### Building Chain Specs

Generate a raw (encoded) chainspec for distribution:

```bash
# Build human-readable spec
x3-chain-node build-spec --chain=staging > staging-spec.json

# Build raw spec (for production use)
x3-chain-node build-spec --chain=staging --raw > staging-spec-raw.json

# Validate spec
jq . staging-spec.json > /dev/null && echo "Valid"
```

### Chainspec Structure

```json
{
  "name": "X3 Chain Staging",
  "id": "x3_chain_staging",
  "chainType": "Live",
  "bootNodes": [
    "/ip4/127.0.0.1/tcp/30333/ws/p2p/..."
  ],
  "genesis": {
    "runtime": {
      "system": { "code": "0x..." },
      "balances": { "balances": [[...]] },
      "aura": { "authorities": [...] },
      "grandpa": { "authorities": [...] }
    }
  },
  "properties": {
    "tokenDecimals": 12,
    "tokenSymbol": "X3"
  }
}
```

### Using Custom Specs

```bash
# Load from JSON file
x3-chain-node --chain=./my-custom-spec.json

# Or via environment
CHAIN_SPEC=./my-custom-spec.json x3-chain-node
```

---

## Advanced Configuration

### Database Options

#### Storage Backend Selection

```bash
# Use RocksDB (default, faster, more CPU)
x3-chain-node --db=rocksdb

# Use ParityDB (newer, lower resource)
x3-chain-node --db=paritydb
```

#### State Caching

```bash
# Increase state cache (more memory, faster queries)
x3-chain-node --state-cache-size=512  # MB

# Maximize for high throughput
x3-chain-node --state-cache-size=2048
```

### Consensus Configuration

#### GRANDPA Settings

```bash
# Enable GRANDPA (default)
x3-chain-node

# Disable GRANDPA (use Flash Finality instead)
x3-chain-node --disable-grandpa --enable-flash-finality
```

#### Block Authoring

```bash
# Force block authoring (even if no peers)
x3-chain-node --dev --force-authoring

# Disable authoring for archive nodes
x3-chain-node --archive  # Implies not a validator
```

---

## Performance Tuning

### High Throughput Configuration

For 100K TPS target:

```bash
x3-chain-node \
  --chain=staging \
  --validator \
  --name=htp-validator-1 \
  --state-cache-size=2048 \
  --db=rocksdb \
  --max-runtime-instances=256 \
  --enable-parallel-proposer \
  --enable-poh \
  --pruning=1000 \
  --pool-kind=transaction-pool-v2
```

**Resource Requirements**:
- CPU: 8+ cores
- Memory: 32+ GB
- Storage: 500GB+ SSD

### Low Resource Configuration

For constrained hardware:

```bash
x3-chain-node \
  --chain=staging \
  --state-cache-size=32 \
  --db=paritydb \
  --max-runtime-instances=16 \
  --pruning=256 \
  --pool-kind=basic
```

**Resource Footprint**:
- CPU: 2+ cores (minimum)
- Memory: 4+ GB
- Storage: 100GB+ SSD

---

## Network Configuration

### Port Configuration

| Port | Service | Default |
|------|---------|---------|
| 30333 | P2P | TCP/UDP |
| 9933 | HTTP-RPC | TCP (localhost) |
| 9944 | WebSocket-RPC | TCP (localhost) |
| 9615 | Prometheus metrics | TCP (localhost) |

### Dual-Stack (IPv4/IPv6)

```bash
# IPv4 only
x3-chain-node --listen-addr=/ip4/0.0.0.0/tcp/30333

# IPv6 only
x3-chain-node --listen-addr=/ip6/[::]/tcp/30333

# Both
x3-chain-node \
  --listen-addr=/ip4/0.0.0.0/tcp/30333 \
  --listen-addr=/ip6/[::]/tcp/30333
```

### Firewall Rules

```bash
# Allow P2P
sudo ufw allow 30333/tcp
sudo ufw allow 30333/udp

# Allow RPC (only from trusted sources)
sudo ufw allow from 10.0.0.0/8 to any port 9933
sudo ufw allow from 10.0.0.0/8 to any port 9944

# Allow metrics (internal only)
sudo ufw allow from 10.0.0.0/8 to any port 9615
```

### Bootnodes Configuration

**In chainspec** (preferred):
```json
"bootNodes": [
  "/dns/boot1.x3-chain.io/tcp/30333/ws/p2p/...",
  "/dns/boot2.x3-chain.io/tcp/30333/ws/p2p/..."
]
```

**Via CLI**:
```bash
x3-chain-node \
  --bootnodes=/dns/boot1.x3-chain.io/tcp/30333/ws/p2p/... \
  --bootnodes=/dns/boot2.x3-chain.io/tcp/30333/ws/p2p/...
```

---

## Configuration Examples

### Local Development

```bash
#!/bin/bash
set -e

# Colors
GREEN='\033[0;32m'
NC='\033[0m'

echo -e "${GREEN}Starting X3 Chain dev node${NC}"
x3-chain-node \
  --chain=dev \
  --tmp \
  --alice \
  --unsafe-rpc-external \
  --unsafe-ws-external \
  --force-authoring \
  --log=info
```

### Staging Validator

```bash
#!/bin/bash
set -e

VALIDATOR_NAME="${1:-validator-1}"
TELEMETRY_URL="wss://telemetry.x3-chain.io/submit"

echo "Starting staging validator: $VALIDATOR_NAME"

x3-chain-node \
  --chain=staging \
  --name=$VALIDATOR_NAME \
  --validator \
  --enable-poh \
  --pruning=1000 \
  --db=rocksdb \
  --telemetry-url "$TELEMETRY_URL,1" \
  --prometheus-external \
  --log=info 2>&1 | tee logs/$VALIDATOR_NAME.log
```

### Production Infrastructure

```bash
#!/bin/bash
set -e

# For production:
# 1. Store node key in secure vault
# 2. Use systemd or similar for process management
# 3. Configure log rotation
# 4. Set up monitoring and alerts

NODE_KEY=$(cat /vault/node-key.txt)

exec x3-chain-node \
  --chain=production \
  --name=prod-archive-1 \
  --node-key=$NODE_KEY \
  --pruning=archive \
  --db=rocksdb \
  --telemetry-url "wss://telemetry.x3-chain.io/submit,1" \
  --prometheus-external \
  --log=info
```

---

## See Also

- [DEVELOPMENT.md](./DEVELOPMENT.md) - Node development guide
- [SHUTDOWN.md](./SHUTDOWN.md) - Graceful shutdown procedures
- [CLI_FLAGS.md](./CLI_FLAGS.md) - Complete CLI reference
- [Substrate Configuration](https://docs.substrate.io/reference/command-line-tools/)
