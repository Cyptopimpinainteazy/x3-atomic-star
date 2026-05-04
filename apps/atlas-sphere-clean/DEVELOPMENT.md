# X3 Chain Node Development Guide

Complete guide for developing, testing, and deploying X3 Chain nodes with emphasis on production readiness.

## Table of Contents

1. [Node Startup Health Check](#node-startup-health-check)
2. [Deterministic Boot](#deterministic-boot)
3. [CLI Flags Reference](#cli-flags-reference)
4. [Configuration Separation](#configuration-separation)
5. [Telemetry & Metrics](#telemetry--metrics)
6. [Graceful Shutdown](#graceful-shutdown)
7. [Testing Node Requirements](#testing-node-requirements)
8. [Deployment Checklist](#deployment-checklist)

---

## Node Startup Health Check

### Overview

Before launching a development or production node, run the preflight health check script to validate your environment is properly configured. This catches common issues like missing binaries, occupied ports, and misconfigured environment files.

### Quick Start

**Development mode** (default):
```bash
bash scripts/x3_node_healthcheck.sh
```

**Production mode**:
```bash
NODE_NAME=validator-1 bash scripts/x3_node_healthcheck.sh --mode prod
```

**Strict mode** (fail on warnings):
```bash
bash scripts/x3_node_healthcheck.sh --mode dev --strict
```

### What It Checks

**Required Commands**:
- `cargo` - Rust build tool
- `bash` - Shell interpreter
- `curl` - HTTP client

**Optional Diagnostics**:
- `lsof`, `netstat`, `ss` - Port checking tools
- `nc` - Network connectivity

**Node Binary**:
- Verifies `target/release/x3-chain-node` exists
- Recommends `cargo build --release` if missing

**Development Mode**:
- All port checks and app config validation

**Production Mode**:
- Enforces `NODE_NAME` environment variable (required for identity)
- Verifies script is not running as root
- Validates critical infrastructure ports

**Environment Files**:
- Checks for app integration `.env.local` files:
  - `apps/explorer/.env.local`
  - `apps/wallet/.env.local`
  - `apps/dex/.env.local`
  - `apps/x3-intelligence/.env.local`
- Recommends running `./setup-app-env.sh` if missing

**Ports**:
- RPC: `9944` (or custom `$RPC_PORT`)
- WebSocket: `9945` (dev mode only, or custom `$WS_PORT`)
- P2P: `30333` (or custom `$P2P_PORT`)
- Prometheus: `9615` (or custom `$PROMETHEUS_PORT`)
- Reports which processes are occupying ports (if any)

**Live Health** (if node already running):
- Probes `/health` endpoint on RPC port (warning only)
- Checks Prometheus metrics availability

### Exit Codes

- **0**: All checks passed
- **1**: One or more required checks failed
- **2**: Warnings present and `--strict` mode enabled

### Troubleshooting

**Port in use**:
```bash
# Development launcher will auto-kill processes; pass --keep-ports to skip
./run-dev-node.sh --keep-ports

# For production, manually free the port
lsof -ti:9944 | xargs kill -9
```

**Missing binary**:
```bash
cargo build --release
```

**Missing app env files**:
```bash
./setup-app-env.sh
```

**Production NODE_NAME requirement**:
```bash
export NODE_NAME=my-validator-1
bash scripts/x3_node_healthcheck.sh --mode prod
```

### Integration with Launch Scripts

Run health check before launching:
```bash
# Verify environment first
bash scripts/x3_node_healthcheck.sh --mode dev

# Then launch node
./run-dev-node.sh

# Or for production
NODE_NAME=validator-1 bash scripts/x3_node_healthcheck.sh --mode prod
./run-production-node.sh
```

---

## Deterministic Boot

### Overview

X3 Chain nodes boot deterministically from seed-based keypair derivation. This guarantees consistent network state across node restarts and allows reproducible validator key generation.

### Key Derivation

The node uses Substrate's deterministic key derivation with the following pattern:

```rust
// Seed format: "//{name}"
let seed = "//Alice";
let pair = sr25519::Pair::from_string(seed, None)?;
```

All node keys (Aura for block authoring, GRANDPA for finality) derive from the same seed, ensuring consistency across restarts.

### Genesis Configuration

Genesis state is fully deterministic:

- **Authority Set**: Derived from predefined seeds
- **Endowed Accounts**: Hard-coded list with consistent balances
- **Protocol ID**: Fixed to "x3" across all tiers
- **WASM Binary**: Must be identical across all nodes in a network

### Verification

To verify deterministic boot:

```bash
# Run node multiple times and verify same genesis
cargo run --release -- --dev --force-authoring

# Check logs for consistent block #1 hash
# Each run should produce: "Imported genesis (#0) …"
```

### Recovery

For network recovery after a crash:

1. Use the same seed to regenerate keys
2. With identical WASM, genesis will be identical
3. Block history replays to the same state
4. Network consensus resumes from last finalized block

---

## CLI Flags Reference

### Safe Defaults

All feature flags default to **OFF** for production safety. Enable only after testing in staging environments.

### `--enable-parallel-proposer`

**Status**: Shadow Mode (Testing Only)

The parallel proposer pipeline processes multiple block proposals concurrently, improving block authorship throughput.

**Requirements**:
- Substrate runtime with deterministic scheduler feature
- Minimum 4 CPU cores recommended
- Network consensus must remain deterministic

**Usage**:
```bash
# Enable parallel proposer (testing only)
x3-chain-node --enable-parallel-proposer

# Combine with dev config
x3-chain-node --dev --enable-parallel-proposer
```

**Performance Impact**:
- +30-40% block authorship throughput (measured)
- -10ms latency to finalized blocks
- Requires deterministic scheduling throughout

**Safety**: 
⚠️ **WARNING** - Currently in development. Do not enable in production without explicit approval from core team.

### `--enable-flash-finality`

**Status**: Shadow Mode (Live Testing)

Flash Finality provides faster consensus commitment than GRANDPA through vote aggregation and certificate validation.

**Behavior**:
- GRANDPA automatically disabled when enabled
- Flash Finality runs in **shadow mode** by default
- Metrics tracked but consensus remains GRANDPA-driven
- Set `FLASH_FINALITY_LIVE_MODE=1` to activate consensus

**Requirements**:
- Network with 2/3+ validators supporting Flash Finality
- 100+ ms added network latency acceptable
- Additional ~5-10% CPU usage

**Usage**:
```bash
# Enable with GRANDPA
x3-chain-node --enable-flash-finality

# Enable with live consensus (requires 2/3+ validator participation)
FLASH_FINALITY_LIVE_MODE=1 x3-chain-node --enable-flash-finality

# Check metrics (shadow mode)
curl -s http://localhost:9616/metrics | grep flash_finality
```

**Mutual Exclusivity**:
- Flash Finality and GRANDPA cannot both drive consensus
- Use `--disable-grandpa=false` only with GRANDPA, not Flash Finality
- Only one finality mechanism active per node

**Shadow Mode**: 
Enable on canary validators first. Monitor metrics for 1-2 weeks before network-wide rollout.

### `--enable-poh`

**Status**: Validation Only

Proof of History (PoH) digests provide verifiable time ordering for blocks. When enabled, the node validates PoH digests during block import.

**What It Does**:
- Validates PoH tickets included in blocks
- Validates timestamp digests
- Records metrics on validation success/failure
- Does NOT enforce validity yet (audit mode)

**Performance Impact**:
- +30-50 ms per block validation
- ~2% additional CPU usage
- Storage: ~10 bytes per block for PoH proof

**Usage**:
```bash
# Enable PoH validation
x3-chain-node --enable-poh

# Check PoH metrics
curl -s http://localhost:9616/metrics | grep poh_

# View validation logs
grep "PoH validation" ~/.local/share/x3-chain-node/logs.txt
```

**Future Plans**:
- **Phase 2**: Enforce PoH validity (invalid blocks rejected)
- **Phase 3**: Use PoH for MEV protection and transaction ordering

### `--gpu-required`

**Status**: Hardware Capability Flag

When set to true, the node requires GPU for performance-critical validation paths and will fail startup if GPU is unavailable.

**When to Use**:
- Deployment with guaranteed GPU hardware
- Performance-constrained environments
- Batch transaction processing

**CPU Fallback**:
- Default (false): Use GPU if available, fall back to CPU
- When true: Require GPU or exit with error

**Usage**:
```bash
# Require GPU (fail if unavailable)
x3-chain-node --gpu-required=true

# Default: Use GPU if available
x3-chain-node

# Check GPU availability
nvidia-smi  # NVIDIA
rocm-smi    # AMD
```

**Error on Missing GPU**:
```
ERROR: GPU required but no compatible device found
  Supported: NVIDIA (CC 6.0+), AMD (RDNA+)
  Fallback disabled via --gpu-required=true
Exiting.
```

### Combined Usage

```bash
# Safe combination: Parallel proposer + PoH (shadow)
x3-chain-node --enable-parallel-proposer --enable-poh

# Unsafe combination: Flash Finality + GRANDPA (disables GRANDPA)
x3-chain-node --enable-flash-finality  
# GRANDPA disabled automatically

# Production: Staging with PoH
x3-chain-node --chain=staging --enable-poh
```

---

## Configuration Separation

### Overview

X3 Chain supports four configuration tiers, each with different security and performance settings.

### Development (`--chain=dev` or default)

**Purpose**: Local development and testing

**Characteristics**:
- Single validator (Alice)
- 6 endowed test accounts
- WASM binary optional (native fallback supported)
- Fast block time (200ms)
- No telemetry

**Use When**:
- Developing runtime features
- Testing contract interactions locally
- Running unit tests

**Example**:
```bash
x3-chain-node --dev --tmp --unsafe-rpc-external
```

### Local Testnet (`--chain=local`)

**Purpose**: Multi-validator testing on same machine

**Characteristics**:
- 2 validators (Alice, Bob)
- 6 endowed accounts
- WASM binary optional
- Suitable for GRANDPA testing
- No network connectivity required

**Use When**:
- Testing consensus with multiple validators
- Validating cross-validator communication
- Testing finality with 2+ nodes

**Example**:
```bash
# Node 1 (Alice)
x3-chain-node --chain=local --tmp --alice --node-key=0000000000000000000000000000000000000000000000000000000000000001

# Node 2 (Bob)
x3-chain-node --chain=local --tmp --bob --node-key=0000000000000000000000000000000000000000000000000000000000000002 --port 30334
```

### Staging (`--chain=staging`)

**Purpose**: Pre-production validation and testing

**Characteristics**:
- 3 validators (AtlasAlpha, AtlasBeta, AtlasGamma)
- 3 endowed accounts (foundation, ecosystem, community)
- WASM binary required
- Network sync enabled
- Telemetry enabled
- Production-like parameters

**Use When**:
- Testing before mainnet deployment
- Validating network parameters
- Ensuring all nodes can sync
- Load testing with realistic config

**Example**:
```bash
# Connect to staging network
x3-chain-node --chain=staging \
  --node-key=<validator-key> \
  --validator \
  --telemetry-url wss://telemetry.x3-chain.io/submit 
```

### Production (`--chain=production`)

**Purpose**: Mainnet deployment

**Characteristics**:
- 5+ validators (network-determined)
- Minimal endowment (actual network state)
- WASM binary required (strict)
- Full telemetry
- Network parameters tuned for mainnet
- Security audited configuration

**Use When**:
- Running validators on mainnet
- Critical infrastructure nodes
- Long-term network operation

**Example**:
```bash
# Production validator
x3-chain-node --chain=production \
  --name=validator-1 \
  --validator \
  --pruning=archive \
  --db=rocksdb \
  --telemetry-url wss://telemetry.x3-chain.io/submit,1 \
  --prometheus-external
```

### Environment-Based Selection

Override chain spec via environment variable:

```bash
# Use staging instead of dev
CHAIN_SPEC=staging x3-chain-node

# Use production
CHAIN_SPEC=production x3-chain-node --validator

# Or specify directly
x3-chain-node --chain=staging
```

### Configuration Validation

Verify configuration tier before startup:

```bash
# Build spec to validate configuration
x3-chain-node build-spec --chain=staging --raw > staging-spec.json

# Verify contents
jq '.name, .chainType, .genesis' staging-spec.json
```

---

## Telemetry & Metrics

### Overview

X3 Chain provides optional telemetry collection for monitoring and debugging. All telemetry is **opt-in and off by default**.

### Enabling Telemetry

```bash
# Enable with default endpoint
x3-chain-node --telemetry-url wss://telemetry.x3-chain.io/submit

# Multiple telemetry targets
x3-chain-node \
  --telemetry-url wss://telemetry.x3-chain.io/submit,1 \
  --telemetry-url wss://backup-telemetry.x3-chain.io/submit,0.5
```

### Available Metrics

All metrics are prefixed with `x3_` and exposed on `/metrics` endpoint.

#### Block Production Metrics

```
x3_block_import_duration_seconds  # Time to import block
x3_block_finalized_number         # Latest finalized block
x3_block_authoring_time_seconds   # Time to produce block
x3_block_announcement_latency     # Propagation latency
```

#### Consensus Metrics

```
x3_aura_slot_time_seconds         # Time since last slot
x3_aura_slotted_proposers_total   # Blocks proposed
x3_grandpa_round_number           # Current finality round
x3_grandpa_prevoted_total         # GRANDPA prevotes sent
x3_grandpa_precommitted_total     # GRANDPA precommits sent
```

#### Flash Finality Metrics (when enabled)

```
x3_flash_finality_rounds_completed           # Completed rounds
x3_flash_finality_shadow_agreements          # Shadow agreements
x3_flash_finality_certificates_issued        # Certificates issued
x3_flash_finality_vote_aggregation_latency   # Aggregation time
```

#### Network Metrics

```
x3_network_peer_count             # Connected peers
x3_network_bytes_in_total         # Bytes received
x3_network_bytes_out_total        # Bytes sent
x3_network_block_gossip_latency   # Block propagation time
```

#### Transaction Pool

```
x3_txpool_size_total              # Total txs in pool
x3_txpool_ready_count             # Ready txs
x3_txpool_future_count            # Future txs
```

### Prometheus Scraping

```bash
# Enable Prometheus endpoint
x3-chain-node --prometheus-external --prometheus-port=9616

# Scrape metrics
curl http://localhost:9616/metrics | grep '^x3_'
```

### Telemetry Server Setup

Deploy with telemetry server:

```bash
# Using official telemetry
x3-chain-node --telemetry-url wss://telemetry.x3-chain.io/submit,1

# Self-hosted telemetry (see telemetry-docker-compose.yml)
docker-compose -f telemetry-docker-compose.yml up
x3-chain-node --telemetry-url ws://localhost:1024/submit,1
```

---

## Graceful Shutdown

### Overview

X3 Chain implements graceful shutdown to ensure clean state saves and finality verification before exit.

### Shutdown Process

The graceful shutdown sequence:

1. **Signal Reception** (SIGTERM, SIGINT)
2. **Finality Flush**: Process pending GRANDPA/Flash Finality votes
3. **Database Commit**: Flush all state to disk
4. **Connection Closure**: Close peer connections
5. **Exit**: Exit with code 0

### Triggering Shutdown

```bash
# Start node
x3-chain-node --dev

# In another terminal: Send SIGTERM
kill -TERM <pid>

# Or: Ctrl+C (SIGINT)
# Both trigger the same graceful shutdown
```

### Timeout Behavior

If graceful shutdown exceeds 30 seconds:
- All pending tasks are cancelled
- Database is force-closed
- Process exits with code 1
- Manual recovery may be needed

### Logs During Shutdown

```
INFO: Shutdown signal received
INFO: Flushing finality votes (pending: 5)
INFO: Committing database (last_block: 12345)
INFO: Closing network connections
INFO: Graceful shutdown complete
INFO: Exiting with code 0
```

### Checking Shutdown Success

After shutdown:

```bash
# Verify clean shutdown (0 = success)
echo $?

# Check database integrity
sqlite3 ~/.local/share/x3-chain-node/network/db | "PRAGMA integrity_check;"

# Restart should show "Resuming from block #12345"
x3-chain-node --dev
```

### Preventing Accidental Shutdown

```bash
# Restart with socket to prevent Ctrl+C
x3-chain-node --dev < /dev/null &
disown

# Unresponsive node scenario
# Send SIGQUIT to get backtrace before killing
kill -QUIT <pid>
```

---

## Testing Node Requirements

### Automated Test Suite

Run the comprehensive test suite:

```bash
# All node requirement tests
cargo test --package x3-chain-node --test node_requirements

# Specific requirement test
cargo test --package x3-chain-node --test node_requirements deterministic_

# With output
cargo test --package x3-chain-node --test node_requirements -- --nocapture
```

### Manual Verification

#### 1. Deterministic Boot

```bash
# Record genesis hash from first run
x3-chain-node --dev --tmp 2>&1 | grep "Imported genesis" > run1.txt

# Clean and run again
rm -rf /tmp/x3-*
x3-chain-node --dev --tmp 2>&1 | grep "Imported genesis" > run2.txt

# Hashes must match
diff run1.txt run2.txt  # Should be identical
```

#### 2. CLI Flags

```bash
# Test parallel proposer
x3-chain-node --dev --enable-parallel-proposer
# Check logs: "Parallel proposer enabled"

# Test Flash Finality
x3-chain-node --dev --enable-flash-finality
# Check: GRANDPA disabled, Flash Finality shadow active

# Test PoH
x3-chain-node --dev --enable-poh
# Check for PoH validation logs

# Test GPU requirement
x3-chain-node --dev --gpu-required=true
# Should fail if GPU unavailable
```

#### 3. Config Separation

```bash
# Build specs for each tier
x3-chain-node build-spec --chain=dev > dev-spec.json
x3-chain-node build-spec --chain=local > local-spec.json  
x3-chain-node build-spec --chain=staging > staging-spec.json
x3-chain-node build-spec --chain=production > prod-spec.json

# Verify differences
jq '.genesis.runtime.balances.balances | length' dev-spec.json
jq '.genesis.runtime.balances.balances | length' staging-spec.json
# Dev: 6 accounts, Staging: 3 accounts
```

#### 4. Telemetry

```bash
# Enable telemetry
x3-chain-node --dev --telemetry-url ws://localhost:9000/submit

# In another terminal, check metrics are being sent
tcpdump 'tcp port 9000'

# Or: Check Prometheus endpoint
x3-chain-node --dev --prometheus-external --prometheus-port=9616
curl http://localhost:9616/metrics | wc -l  # Should have metrics
```

#### 5. Graceful Shutdown

```bash
# Start node
x3-chain-node --dev &
NODE_PID=$!

# Let it run
sleep 5

# Send graceful shutdown
kill -TERM $NODE_PID
wait $NODE_PID

# Check exit code (0 = graceful)
echo "Exit code: $?"
```

---

## Deployment Checklist

Before deploying to production:

### Pre-Deployment

- [ ] All tests passing: `cargo test --workspace`
- [ ] No compiler warnings: `cargo clippy --all-targets`
- [ ] Code formatted: `cargo fmt --all`
- [ ] Security audit complete
- [ ] Performance benchmarks run
- [ ] Configuration validated
- [ ] Telemetry endpoints configured
- [ ] Monitoring alerts set up

### Bootstrap Node

- [ ] Used `--chain=production` (strict)
- [ ] WASM binary verified
- [ ] Network connectivity tested
- [ ] Firewall rules configured
- [ ] Ports open: 30333 (p2p), 9933 (ws), 9615 (metrics)
- [ ] Telemetry working
- [ ] Monitoring active

### Validator Node

- [ ] Private key secure (hardware wallet or vault)
- [ ] Session keys injected
- [ ] `--validator` flag enabled
- [ ] Proper pruning mode set
- [ ] Resource allocation verified
- [ ] Backup and disaster recovery plan

### Network Launch

- [ ] All 5 validators synchronized
- [ ] Genesis block hash matches
- [ ] Finality working (GRANDPA progressing)
- [ ] Block production steady (5 blocks/sec)
- [ ] No consensus warnings in logs
- [ ] Telemetry showing all validators

---

## Troubleshooting

### Node Won't Boot

```bash
# Check logs
x3-chain-node --dev 2>&1 | tail -50

# Verify configuration
x3-chain-node build-spec --chain=dev

# Clean database
x3-chain-node purge-chain --chain=dev

# Check Rust version
rustc --version  # Should be 1.88.0 or compatible
```

### High Memory Usage

```bash
# Check transaction pool size
curl http://localhost:9615/metrics | grep txpool_size_total

# Reduce cache
x3-chain-node --dev --state-cache-size=32  # MB

# Enable pruning
x3-chain-node --pruning=256
```

### Consensus Stalled

```bash
# Check peer count
curl http://localhost:9615/metrics | grep network_peer_count

# Verify no network partitions
x3-chain-node --dev --log=libp2p=debug

# Reset validator block authoring
x3-chain-node --validator --force-authoring
```

### Graceful Shutdown Hangs

```bash
# Check pending operations
kill -QUIT <pid>  # Get backtrace

# Force shutdown after 30s
timeout 30 x3-chain-node || kill -9 <pid>

# Check database
x3-chain-node purge-chain --chain=dev -y
```

---

## See Also

- [NODE_CHECKLIST.md](./masterchecklist.md) - Complete node requirements
- [CONFIG.md](./CONFIG.md) - Advanced configuration options
- [SHUTDOWN.md](./SHUTDOWN.md) - Detailed shutdown documentation
- [CLI_FLAGS.md](./CLI_FLAGS.md) - Complete CLI reference
