# X3 Chain CLI Flags Reference

Complete reference for X3 Chain node command-line flags and options.

## Table of Contents

1. [Quick Reference](#quick-reference)
2. [Core Flags](#core-flags)
3. [Feature Flags](#feature-flags)
4. [Network Configuration](#network-configuration)
5. [RPC Configuration](#rpc-configuration)
6. [Database Configuration](#database-configuration)
7. [Consensus Configuration](#consensus-configuration)
8. [Developer Flags](#developer-flags)
9. [Examples](#examples)

---

## Quick Reference

```bash
# Local development (single node)
x3-chain-node --dev

# Multi-validator testing (Alice)
x3-chain-node --chain=local --alice

# Staging validator
x3-chain-node --chain=staging --validator --name=validator-1

# Production validator
x3-chain-node --chain=production --validator --pruning=archive

# With all telemetry
x3-chain-node \
  --chain=staging \
  --validator \
  --telemetry-url wss://telemetry.x3-chain.io/submit,1 \
  --prometheus-external
```

---

## Core Flags

### `--chain <CHAIN_SPEC>`

Select the blockchain to run. Determines genesis state, validators, accounts, and network parameters.

**Valid Values**:
- `dev` – Single-validator development chain (default)
- `local` – Two-validator local testnet
- `staging` – Three-validator staging network
- `production` – Mainnet configuration
- `<file>` – Path to custom chainspec JSON

**Default**: `dev`

**Examples**:
```bash
x3-chain-node --chain=dev
x3-chain-node --chain=/path/to/custom-spec.json
x3-chain-node --chain=staging
```

### `--name <NAME>`

Human-readable name for this node (appears in telemetry, logs, peer discovery).

**Default**: Auto-generated

**Examples**:
```bash
x3-chain-node --name=validator-alpha
x3-chain-node --name=archive-node-1
```

### `--node-key <KEY>`

P2P networking secret key (Ed25519). Determines peer identity and bootnodes list.

**Format**: 64 hex characters or `FILE` path

**Default**: Random key generated each run

**Examples**:
```bash
# Use specific key
x3-chain-node --node-key=0000000000000000000000000000000000000000000000000000000000000001

# Use key from file
x3-chain-node --node-key=FILE

# Regenerate if needed
rm ~/.local/share/x3-chain-node/network/secret
```

**Warning**: ⚠️ Private key – keep secure! Not suitable for production (use hardware wallet).

### `--base-path <PATH>`

Directory for blockchain data, database, and state (default: `~/.local/share/x3-chain-node`).

**Default**: Platform-specific user data directory

**Examples**:
```bash
x3-chain-node --base-path=/var/lib/x3-chain-node
x3-chain-node --base-path=/tmp/x3-dev
```

### `--tmp`

Use temporary directory for all data (cleared on exit).

**Default**: Off (uses `--base-path`)

**Use**: Development, testing, CI/CD

**Examples**:
```bash
x3-chain-node --dev --tmp   # Clean state each run
```

---

## Feature Flags

### `--enable-parallel-proposer`

Enable parallel block proposal pipeline (shadow mode).

**Status**: Development/Testing

**Default**: Off (safe)

**Performance**: +30-40% block throughput

**Example**:
```bash
x3-chain-node --dev --enable-parallel-proposer
```

### `--enable-flash-finality`

Enable Flash Finality consensus gadget (shadow by default).

**Status**: Live Testing (shadow mode)

**Default**: Off (GRANDPA active)

**Effect**: Disables GRANDPA, enables Flash Finality

**Enable Live Mode**:
```bash
FLASH_FINALITY_LIVE_MODE=1 x3-chain-node --enable-flash-finality
```

**Example**:
```bash
# Shadow mode (monitors only)
x3-chain-node --enable-flash-finality

# Requires 2/3+ validator participation
FLASH_FINALITY_LIVE_MODE=1 x3-chain-node --enable-flash-finality
```

### `--enable-poh`

Enable Proof of History validation (audit mode).

**Status**: Validation Only

**Default**: Off

**Performance**: +30-50ms per block

**Example**:
```bash
x3-chain-node --enable-poh

# Enforce validity (future)
POH_ENFORCED=1 x3-chain-node --enable-poh
```

### `--gpu-required`

Require GPU for performance-critical operations.

**Default**: Off (GPU optional, CPU fallback)

**When to Use**: Guaranteed GPU deployments

**Example**:
```bash
# Fail if no GPU
x3-chain-node --gpu-required=true

# Optional GPU (default)
x3-chain-node
```

---

## Network Configuration

### `--listen-addr <ADDRESS>`

P2P listen address and port.

**Format**: `/ip4/0.0.0.0/tcp/30333` or `/ip6/[::]/tcp/30333`

**Default**: `/ip4/0.0.0.0/tcp/30333`

**Examples**:
```bash
# IPv4 only
x3-chain-node --listen-addr=/ip4/0.0.0.0/tcp/30333

# IPv6 only
x3-chain-node --listen-addr=/ip6/[::]/tcp/30333

# Multiple addresses (dual-stack)
x3-chain-node \
  --listen-addr=/ip4/0.0.0.0/tcp/30333 \
  --listen-addr=/ip6/[::]/tcp/30333
```

### `--port <PORT>`

P2P port (shorthand for `--listen-addr`).

**Default**: `30333`

**Examples**:
```bash
x3-chain-node --port=30334
x3-chain-node --port=30335
```

### `--bootnodes <BOOTNODE>`

Connect to initial peer nodes.

**Format**: `/ip4/x.x.x.x/tcp/30333/ws/p2p/QmAbcDef...`

**Examples**:
```bash
x3-chain-node \
  --bootnodes=/ip4/boot1.example.com/tcp/30333/ws/p2p/Qm... \
  --bootnodes=/ip4/boot2.example.com/tcp/30333/ws/p2p/Qm...
```

### `--public-addr <ADDRESS>`

Advertise different address than listening address (for NAT behind).

**Format**: `/ip4/1.2.3.4/tcp/30333/ws/p2p/...`

**Examples**:
```bash
# Behind NAT
x3-chain-node --listen-addr=/ip4/127.0.0.1/tcp/30333 \
              --public-addr=/ip4/203.0.113.1/tcp/30333/ws
```

---

## RPC Configuration

### `--rpc-port <PORT>`

HTTP RPC endpoint port.

**Default**: `9933` (localhost only)

**Examples**:
```bash
x3-chain-node --rpc-port=9933
x3-chain-node --rpc-port=8080
```

### `--ws-port <PORT>`

WebSocket RPC endpoint port.

**Default**: `9944` (localhost only)

**Examples**:
```bash
x3-chain-node --ws-port=9944
x3-chain-node --ws-port=8081
```

### `--unsafe-rpc-external`

Allow RPC on all interfaces (bypass localhost restriction).

**Default**: Off (localhost only)

**Warning**: ⚠️ Security risk – exposes RPC to network. Use only in trusted environments with firewall.

**Examples**:
```bash
# Dev node exposed for testing
x3-chain-node --dev --unsafe-rpc-external

# Staging with firewall protection
x3-chain-node --chain=staging --unsafe-rpc-external
```

### `--unsafe-ws-external`

Allow WebSocket on all interfaces (bypass localhost restriction).

**Default**: Off (localhost only)

**Warning**: ⚠️ Security risk – same cautions as `--unsafe-rpc-external`

### `--rpc-max-substrate-connections <NUM>`

Maximum concurrent RPC connections.

**Default**: `100`

**Examples**:
```bash
x3-chain-node --rpc-max-substrate-connections=1000
```

---

## Database Configuration

### `--db <TYPE>`

Database backend selection.

**Valid Values**:
- `rocksdb` – Fast, higher CPU/memory (default)
- `paritydb` – New, lower resource usage

**Default**: `rocksdb`

**Examples**:
```bash
x3-chain-node --db=rocksdb
x3-chain-node --db=paritydb
```

### `--database <TYPE>`

Alias for `--db`.

### `--state-cache-size <MB>`

Runtime state cache size (memory).

**Default**: `128` MB

**Tuning**:
- High throughput: `512`-`2048`
- Low resource: `32`-`64`

**Examples**:
```bash
# High throughput
x3-chain-node --state-cache-size=2048

# Low resource
x3-chain-node --state-cache-size=32
```

### `--pruning <MODE>`

Block pruning strategy.

**Valid Values**:
- `archive` – Keep all blocks (validator required)
- `1000` – Keep last 1000 blocks + all finalized
- `256` – Keep last 256 blocks + all finalized

**Default**: `256`

**Examples**:
```bash
# Full node (all history)
x3-chain-node --pruning=archive

# Archive node
x3-chain-node --pruning=archive --name=archive-1

# Space-constrained
x3-chain-node --pruning=128
```

### `--max-runtime-instances <NUM>`

Maximum cached runtime instances.

**Default**: `8`

**High Throughput**: `256`

**Examples**:
```bash
x3-chain-node --max-runtime-instances=256
```

---

## Consensus Configuration

### `--validator`

Enable validator mode (requires session keys injected).

**Default**: Off (archive/full node)

**Requirements**:
- Session keys in keystore
- Authority seat on chain (staking/governance)

**Examples**:
```bash
x3-chain-node --chain=staging --validator --name=validator-1
x3-chain-node --dev --validator --alice
```

### `--force-authoring`

Force block authoring even with 0 peers (for testing).

**Default**: Off

**Use**: Development, testing

**Examples**:
```bash
x3-chain-node --dev --force-authoring
x3-chain-node --tmp --alice --force-authoring
```

### `--disable-grandpa`

Disable GRANDPA finality (must use alternative).

**Default**: Off (GRANDPA enabled)

**Requires**: Alternative finality enabled (Flash Finality)

**Examples**:
```bash
# Flash Finality instead
x3-chain-node --enable-flash-finality  # Disables GRANDPA automatically

# Don't disable unless you know what you're doing
```

---

## Developer Flags

### `--dev`

Quick setup: dev config + force authoring + temp storage + dev keys.

**Default**: Off

**Equivalent to**:
```bash
x3-chain-node \
  --chain=dev \
  --force-authoring \
  --tmp \
  --alice
```

### `--alice`

Use Alice's development keys for block authoring.

**Default**: Not set

**Examples**:
```bash
x3-chain-node --dev --alice
x3-chain-node --chain=local --alice
```

### `--bob`

Use Bob's development keys (second validator).

**Examples**:
```bash
# Node 2 in a two-validator setup
x3-chain-node --chain=local --bob
```

### `--charlie`, `--dave`, `--eve`, `--ferdie`

Additional pre-configured development keys.

---

## Examples

### Development Node (Single)

```bash
x3-chain-node --dev
# Or equivalent:
x3-chain-node \
  --chain=dev \
  --force-authoring \
  --alice \
  --tmp
```

### Local Testnet (Two Nodes)

**Terminal 1 (Alice)**:
```bash
x3-chain-node \
  --chain=local \
  --alice \
  --node-key=0000000000000000000000000000000000000000000000000000000000000001 \
  --port=30333 \
  --rpc-port=9933 \
  --ws-port=9944 \
  --tmp
```

**Terminal 2 (Bob)**:
```bash
x3-chain-node \
  --chain=local \
  --bob \
  --node-key=0000000000000000000000000000000000000000000000000000000000000002 \
  --port=30334 \
  --rpc-port=9934 \
  --ws-port=9945 \
  --tmp \
  --bootnodes=/ip4/127.0.0.1/tcp/30333/ws
```

### Staging Validator

```bash
x3-chain-node \
  --chain=staging \
  --name=validator-alpha \
  --validator \
  --pruning=1000 \
  --db=rocksdb \
  --state-cache-size=512 \
  --telemetry-url wss://telemetry.x3-chain.io/submit,1 \
  --prometheus-external
```

### Production Full Node

```bash
x3-chain-node \
  --chain=production \
  --name=archive-1 \
  --pruning=archive \
  --db=rocksdb \
  --state-cache-size=1024 \
  --telemetry-url wss://telemetry.x3-chain.io/submit,1 \
  --prometheus-external \
  --log=info
```

### Production Validator (Secure)

```bash
x3-chain-node \
  --chain=production \
  --name=validator-prod-1 \
  --node-key=FILE \
  --validator \
  --pruning=archive \
  --db=rocksdb \
  --state-cache-size=2048 \
  --max-runtime-instances=256 \
  --telemetry-url wss://telemetry.x3-chain.io/submit,1 \
  --prometheus-external \
  --log=info
```

### High-Throughput Testnet

```bash
x3-chain-node \
  --chain=staging \
  --name=htp-validator-1 \
  --validator \
  --enable-parallel-proposer \
  --enable-poh \
  --state-cache-size=2048 \
  --max-runtime-instances=256 \
  --db=rocksdb \
  --pruning=1000 \
  --log=debug
```

---

## Help Output

Get full help with descriptions:

```bash
# All flags
x3-chain-node --help

# Specific subcommand
x3-chain-node build-spec --help
x3-chain-node run --help

# Feature flags only
x3-chain-node run --help | grep -A5 "enable-"
```

---

## See Also

- [DEVELOPMENT.md](./DEVELOPMENT.md) - Development guide
- [CONFIG.md](./CONFIG.md) - Configuration reference
- [SHUTDOWN.md](./SHUTDOWN.md) - Shutdown procedures
- [Substrate CLI](https://docs.substrate.io/reference/command-line-tools/)
