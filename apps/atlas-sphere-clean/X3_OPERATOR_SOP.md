# X3 Chain Operator Standard Operating Procedures (SOP)

**Version:** 1.0  
**Date:** March 22, 2026  
**Phase:** 08-02 (Deployment SOP, rollback, and operator runbooks)

---

## Table of Contents

1. [Overview](#overview)
2. [Pre-Deployment Validation](#pre-deployment-validation)
3. [Single-Validator Startup](#single-validator-startup)
4. [Multi-Validator Network Setup](#multi-validator-network-setup)
5. [Monitoring and Health Checks](#monitoring-and-health-checks)
6. [Troubleshooting](#troubleshooting)
7. [Rollback Procedures](#rollback-procedures)
8. [Emergency Procedures](#emergency-procedures)

---

## Overview

This SOP provides step-by-step procedures for deploying and operating X3 Chain nodes across development, staging, and production environments.

**Key Principles:**
- Always run pre-deployment health checks
- Validate environment prerequisites before launch
- Monitor consensus progression and block finality
- Have rollback procedures tested and ready
- Document all environment-specific configurations

---

## Pre-Deployment Validation

### Step 1: Run Environment Health Check

Before launching any node, run the dedicated health check script to validate your local environment:

```bash
# Development mode (default)
bash scripts/x3_node_healthcheck.sh

# Production mode (requires NODE_NAME)
NODE_NAME=my-validator bash scripts/x3_node_healthcheck.sh --mode prod

# Strict mode (exits on warnings)
# Note: in a fresh checkout this can return 2 if app `.env.local` files are not generated yet.
# Run `./setup-app-env.sh` first if you need a fully green strict preflight.
bash scripts/x3_node_healthcheck.sh --mode dev --strict
```

**Expected Output:**
```
╔════════════════════════════════════════════╗
║   X3 CHAIN NODE STARTUP HEALTH CHECK      ║
╚════════════════════════════════════════════╝

Mode: dev
...
═══ SUMMARY ═══

  ✓ Passed: 11
  ⚠ Warnings: 5
  ✗ Failed: 0

Health check PASSED
```

**Exit Codes:**
- `0` = All checks passed, safe to proceed
- `1` = One or more failures detected, DO NOT proceed
- `2` = Warnings present in strict mode, review before proceeding

### Step 2: Verify Prerequisites

The health check validates:

- ✓ **Required Commands**: cargo, bash, curl
- ✓ **Binary Availability**: target/release/x3-chain-node exists
- ✓ **Node Requirements**: CPU, RAM, disk space
- ✓ **Port Availability**: 9944 (JSON-RPC / WS), 9945 (reserved legacy WS/dev tooling), 30333 (P2P), 9615 (Prometheus)
- ✓ **Environment Files**: .env.local for all integrations (explorer, wallet, dex, x3-intelligence)
- ✓ **Production Requirements** (prod mode only):
  - NODE_NAME environment variable is set
  - Not running as root

**If Health Check Fails:**

| Issue | Fix |
|-------|-----|
| Missing binary | `cargo build --release` |
| Missing env files | `./setup-app-env.sh` |
| Port in use | `lsof -ti:PORT \| xargs kill -9` (dev only) |
| NODE_NAME not set (prod) | `export NODE_NAME=validator-1` |
| Running as root (prod) | Switch to non-root user |

### Step 3: Pre-Launch Sanity Checks

Before starting the node:

```bash
# Verify WASM binary is built
ls -lh target/release/x3-chain-node
ls -lh runtime/wasm_binary.rs

# Check recent git status (ensure clean working tree)
git status

# Verify database directory doesn't exist (for clean start)
rm -rf /tmp/x3-dev  # dev mode only

# Verify JSON-RPC / P2P / Prometheus ports are truly free
netstat -talp | grep -E "9944|9945|30333|9615"
```

---

## Single-Validator Startup

### Development Mode

**Purpose:** Local testing, block authoring verification, RPC endpoint testing

**Command:**

```bash
# Step 1: Validate environment
bash scripts/x3_node_healthcheck.sh --mode dev

# Step 2: Start dev node (auto-kills port processes + builds if needed)
./run-dev-node.sh

# Node will output:
# 2026-03-22 20:45:00 X3 Chain Development Node Launcher
# 2026-03-22 20:45:30 Imported genesis (#0) ...
# 2026-03-22 20:45:31 Preparing block at height 1 ...
# 2026-03-22 20:46:00 finalized #1 (0xabc123...) as 'Best'
```

**Expected Progression:**

1. **0-5 seconds**: Binary check + build (if needed)
2. **5-10 seconds**: Port cleanup (auto-kills existing processes)
3. **10-20 seconds**: JSON-RPC server starts (9944) and Prometheus becomes responsive (9615)
4. **20-30 seconds**: Node starts authoring, imports genesis
5. **30+ seconds**: Block progression visible in logs
   - `Preparing block at height N`
   - `finalized #N (hash) as 'Best'`

**Health Indicators (Healthy):**
- RPC endpoint responds to `system_health`
- WebSocket clients connect through the same JSON-RPC port (`ws://127.0.0.1:9944`)
- Prometheus metrics available on 9615/metrics
- Block height increases every ~12 seconds
- Finalized head advances after ~2 minutes

**Unhealthy Indicators (Stop Here):**
- `thread 'tokio-runtime-worker' panicked` → catastrophic failure
- Port binding errors (9944 / 30333 / 9615 still in use)
- No block progression after 2 minutes
- `authority index not found` → validator key issue

**Graceful Shutdown:**

```bash
# Ctrl+C in terminal
# OR
pkill -f "x3-chain-node"
```

---

### Production Mode

**Purpose:** Validator node in testnet/mainnet, consensus participation required

**Prerequisites:**

```bash
# Must set NODE_NAME
export NODE_NAME=validator-$(hostname)

# Validator mode must be enabled for consensus participation
export VALIDATOR=true

# Must be non-root user
id
# output: uid=1000(validator) gid=1000(validator)...
```

**Command:**

```bash
# Step 1: Validate environment
NODE_NAME=validator-1 bash scripts/x3_node_healthcheck.sh --mode prod

# Step 2a: Isolated single-node smoke test (authors blocks immediately)
NODE_NAME=validator-1 VALIDATOR=true BASE_PATH=/tmp/x3-prod-smoke CHAIN=dev ./run-production-node.sh

# Step 2b: Peered validator / local-testnet startup
NODE_NAME=validator-1 VALIDATOR=true BASE_PATH=/tmp/x3-prod-local CHAIN=local ./run-production-node.sh

# Expected output:
# 2026-03-22 20:45:00 X3 Chain Production Node Validator
# 2026-03-22 20:45:15 Starting validator: validator-1
# 2026-03-22 20:45:30 Connected to peer: /ip4/192.168.1.50/tcp/30333/p2p/Qm...
# 2026-03-22 20:46:00 ⚙️  Syncing... block #1 (0xabc...)
# 2026-03-22 20:47:00 ✨ Imported #2 [state_root: 0xdef456...]
```

**Important:**

- Use `CHAIN=dev` when validating a single production-style validator in isolation.
- Use `CHAIN=local`, `CHAIN=testnet`, or an explicit chainspec path when joining a peered network.
- `VALIDATOR=true` is required for authority participation when using `run-production-node.sh`.
- On peered chains, a lone validator can remain idle at genesis until at least one peer joins; this is expected behavior, not a startup failure.

**Consensus Participation Checks:**

```bash
# 1. RPC: Check peer connections
curl -s http://localhost:9944 \
  -H "content-type:application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"system_peers","params":[]}' | jq .

# Expected: peers list with connected validators

# 2. RPC: Check authority set
curl -s http://localhost:9944 \
  -H "content-type:application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"state_call","params":["AuraApi_slot_duration"]}' | jq .

# 3. Metrics: Check consensus participation
curl -s http://localhost:9615/metrics | grep -E "substrate_block|finality"

# Expected:
# substrate_block_height{chain="x3",status="best"} 10
# substrate_block_height{chain="x3",status="finalized"} 8
```

**Production Health Thresholds:**

| Metric | Healthy | Warning | Critical |
|--------|---------|---------|----------|
| Peers | ≥2 | 1 | 0 |
| Block height | +1 per slot (6s) | stalls | no progression |
| Finalized lag | ≤2 slots | >4 slots | >10 slots |
| Authority status | active | inactive | removed |

---

## Multi-Validator Network Setup

**Goal:** Validate consensus protocol under realistic multi-validator conditions

### Prerequisites

```bash
# Prepare 4 validator machines (physical or VMs)
# Each has:
# - X3 node binary built
# - Unique NODE_NAME and keys
# - Network connectivity (firewall allows P2P port 30333)

# Create a test network:
local_ip_1="192.168.1.100"
local_ip_2="192.168.1.101"
local_ip_3="192.168.1.102"
local_ip_4="192.168.1.103"
```

### Setup Phase 1: Generate Keys

**On each validator machine:**

```bash
# Generate Aura (block prod) and GRANDPA (finality) keys
cargo run --release -- key generate --scheme sr25519 --password "x3-test"

# Output:
# Secret seed: 0x1234...
# Public key: 5D1...
# Account ID: 5D1...
```

**Collect key material:**

```
Validator 1:
  Secret: 0xabc...
  Public: 5D1aaa...
  NodeName: validator-1

Validator 2:
  Secret: 0xdef...
  Public: 5D2bbb..
  NodeName: validator-2

Validator 3:
  Secret: 0x123...
  Public: 5D3ccc...
  NodeName: validator-3

Validator 4:
  Secret: 0x456...
  Public: 5D4ddd...
  NodeName: validator-4
```

### Setup Phase 2: Configure Bootnodes

**Designate validator-1 as bootnode:**

```bash
# On validator-1, run and capture P2P address:
NODE_NAME=validator-1 ./run-production-node.sh 2>&1 | grep "Local node identity"

# Output:
# Local node identity is: 12D3KooWAB...xyz

# Use this for other validators:
BOOTNODE="/ip4/192.168.1.100/tcp/30333/p2p/12D3KooWAB...xyz"
```

### Setup Phase 3: Start Multi-Validator Cluster

**Terminal 1 (Validator 1 - Bootnode):**

```bash
# Make port 30333 publicly accessible (firewall rule)
NODE_NAME=validator-1 ./run-production-node.sh
```

**Terminal 2 (Validator 2):**

```bash
export NODE_NAME=validator-2
export BOOTNODE="/ip4/192.168.1.100/tcp/30333/p2p/12D3KooWAB...xyz"
./run-production-node.sh --bootnode "$BOOTNODE"
```

**Terminal 3 (Validator 3):**

```bash
export NODE_NAME=validator-3
export BOOTNODE="/ip4/192.168.1.100/tcp/30333/p2p/12D3KooWAB...xyz"
./run-production-node.sh --bootnode "$BOOTNODE"
```

**Terminal 4 (Validator 4):**

```bash
export NODE_NAME=validator-4
export BOOTNODE="/ip4/192.168.1.100/tcp/30333/p2p/12D3KooWAB...xyz"
./run-production-node.sh --bootnode "$BOOTNODE"
```

`run-production-node.sh` accepts both `--bootnode` and `--bootnodes` for operator convenience.

### Validation Phase 1: Peer Connectivity

**On any validator, check peer count:**

```bash
watch -n 2 'curl -s http://localhost:9944 \
  -H "content-type:application/json" \
  -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"system_peers\",\"params\":[]}" | jq ".result | length"'

# Expected progression:
# 0 → 1 → 2 → 3 (peers connected)
```

**Expected Timeline:**

| Time | Event |
|------|-------|
| T+0s | Validator 1 starts (no peers) |
| T+10s | Validator 2 connects (1 peer on all) |
| T+25s | Validator 3 connects (2 peers on most) |
| T+40s | Validator 4 connects (3 peers on all) |
| T+45s | Full mesh (all have ≥3 peers) |

### Validation Phase 2: Consensus Progression

**Monitor block height across all 4 validators:**

```bash
# Terminal A: Watch all 4 block heights
for ip in 100 101 102 103; do
  echo "=== Validator $((ip-99)) ==="
  watch -n 3 "curl -s http://192.168.1.$ip:9944 \
    -H 'content-type:application/json' \
    -d '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"chain_getHeader\",\"params\":[]}' | jq '.result.number'"
done
```

**Healthy Consensus Indicators:**

```
Time 0:00   Height: 0 (genesis imported)
Time 0:15   Height: 2 (slot 1 & 2 authored)
Time 0:30   Height: 4 (slot 3 & 4 authored)
Time 1:00   Height: 8 (slots 5-8 authored)
Time 2:00   Finalized: 4-6 (consistent across all validators)
Time 3:00   Finalized: 10+ (finality advancing)
```

**Unhealthy Consensus Indicators (Blocker):**

```
Symptom: Height stalled at 0-1 for >2 minutes
→ Cause: Authority set misconfiguration
→ Fix: Verify NODE_NAME matches genesis.json

Symptom: Finalized height not advancing
→ Cause: GRANDPA finality stalled
→ Fix: Check logs for "invalid signature" or "authority index not found"

Symptom: Peer count drops to 0
→ Cause: Network partition or bootnode unreachable
→ Fix: Verify firewall allows P2P (port 30333)
```

---

## Monitoring and Health Checks

### Real-Time Monitoring

**Metrics Dashboard** (when running):

```bash
# Open in browser (local-only for dev)
http://localhost:9615/metrics

# Key metrics to watch:
- substrate_block_height (best vs finalized)
- substrate_import_queue_len (≤100 healthy)
- substrate_network_peer_count (≥1 healthy)
- timer_perf_* (authoring/finality latency)
```

**RPC Health Endpoint:**

```bash
# System health (should respond within 1s)
curl -s http://localhost:9944 \
  -H "content-type:application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"system_health","params":[]}' | jq .

# Expected response:
{
  "result": {
    "peers": 3,
    "isSyncing": false,
    "shouldHavePeers": true
  }
}
```

**Log Monitoring:**

```bash
# Follow in real-time
journalctl -u x3-chain-node -f

# Or pipe stdout in terminal:
./run-production-node.sh 2>&1 | tee node.log
```

**Health Check Script (Automated):**

```bash
# Run periodically (every 30 seconds recommended)
bash scripts/x3_node_healthcheck.sh --mode dev --strict

# Exit code 0 = all healthy
# Exit code 1 = critical issue (alarm!)
# Exit code 2 = warnings present (review)
```

---

## Troubleshooting

### Issue: Node Crashes with Panic

**Symptom:**

```
thread 'tokio-runtime-worker' panicked at '...'
```

**Steps:**

1. **Capture full logs:**
   ```bash
   ./run-production-node.sh 2>&1 | tee crash.log
   # Reproduce the issue...
   ```

2. **Analyze panic location:**
   ```bash
   grep "panicked\|at src/" crash.log
   ```

3. **Check recent commits:**
   ```bash
   git log --oneline -10
   git diff HEAD~1
   ```

4. **Common causes & fixes:**

| Panic | Fix |
|-------|-----|
| `index out of bounds` | Check input validation in recent changes |
| `unable to lock database` | Another node process is using it |
| `invalid genesis` | Ensure chain-spec matches WASM |

### Issue: No Block Progression

**Symptom:** Block height stuck at 0 or 1, no new blocks every 6 seconds

**Diagnosis:**

```bash
# Step 1: Check authority set
curl -s http://localhost:9944 \
  -H "content-type:application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"state_call","params":["AuraApi_authorities"]}' | jq .

# Expected: contains your NODE_NAME/keys

# Step 2: Check peer status
curl -s http://localhost:9944 \
  -H "content-type:application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"system_peers","params":[]}' | jq '.result | length'

# Expected: ≥1 peer in multi-node, 0 OK for single-node dev

# Step 3: Check logs for errors
grep -i "error\|failed\|invalid" node.log | head -20
```

**Root Causes:**

| Cause | Indicator | Fix |
|-------|-----------|-----|
| Authority not in set | Log: "authority index not found" | Regenerate keys with correct seed |
| Chain spec mismatch | Log: "genesis hash does not match" | Use `--chain local` (dev) |
| Database locked | Log: "unable to open database" | `pkill x3-chain-node && rm -rf /tmp/x3-*` |
| Time synchronization | No pattern in slot timing | `systemctl stop chronyd` (test env only) |

### Issue: Finality Not Advancing

**Symptom:** Best block height increases, but finalized height stays stuck

**Steps:**

```bash
# Monitor convergence (should happen within 2 min)
watch -n 5 'curl -s http://localhost:9944 -d "{...chain_getHeader...}" | jq .result.number'

# Check GRANDPA status
grep -i "grandpa\|finality" node.log | tail -20

# Peer consensus status
curl http://localhost:9944 -d '{"...system_peers...}' | jq '.result[].roles'
# Expected: all should have "full" or "authority" role
```

**Fix (most common: insufficient peers):**

```bash
# Ensure ≥1 peer is connected
# If single-node dev: acceptable (finality waits for 2/3+ signoff)
# If multi-node: check P2P connectivity:

netstat -talp | grep 30333
# Should show established connections

# If not: check firewall:
sudo ufw allow 30333
```

---

## Rollback Procedures

### Database Rollback

**Scenario:** Chain state is corrupted after a bad update

**Procedure:**

```bash
# Preserve the same launch parameters you used originally
export NODE_NAME=${NODE_NAME:-validator-1}
export VALIDATOR=${VALIDATOR:-true}
export BASE_PATH=${BASE_PATH:-/tmp/x3-dev}
export CHAIN=${CHAIN:-dev}

# 1. Stop the node
pkill -SIGTERM x3-chain-node
sleep 5

# 2. Backup current database
cp -r "$BASE_PATH" "$BASE_PATH.bad.$(date +%s)"

# 3. Delete the database
rm -rf "$BASE_PATH"

# 4. Restart from genesis
NODE_NAME="$NODE_NAME" VALIDATOR="$VALIDATOR" BASE_PATH="$BASE_PATH" CHAIN="$CHAIN" ./run-production-node.sh

# 5. Verify consensus catches up
watch 'curl -s http://localhost:9944 -d "{...chain_getHeader...}" | jq .result.number'
```

**Validated Result (March 22):**
- Recovery procedure was exercised with `BASE_PATH=/tmp/x3-rollback-test`
- Backup directory created successfully
- Restart re-initialized genesis and resumed block production from `#1`

**Expected Recovery Time:**
- Dev node: 30 seconds
- Prod node: 5-10 minutes (depends on block height)

### Binary Rollback

**Scenario:** Latest build has a critical bug

**Procedure:**

```bash
# 1. Stop currently running node
pkill x3-chain-node

# 2. Checkout previous working version
git log --oneline | head -10
git checkout <hash-of-last-good-commit>

# 3. Rebuild binary
cargo build --release

# 4. Restart with same database
./run-production-node.sh

# Node will replay blocks to current height
```

### Configuration Rollback

**Scenario:** Environment variables or launch flags were changed incorrectly

**Procedure:**

```bash
# 1. Review recent environment changes
git diff HEAD~1 .env ~/.x3/config.toml

# 2. Revert the config file
git checkout HEAD~1 -- .env ~/.x3/config.toml

# 3. Stop and restart
pkill x3-chain-node
./run-production-node.sh
```

---

## Emergency Procedures

### Node Hangs (Unresponsive)

```bash
# 1. Try graceful shutdown (wait 10 seconds)
pkill -SIGTERM x3-chain-node
sleep 10

# 2. If still running, force kill
pkill -9 x3-chain-node

# 3. Check for zombie processes
ps aux | grep x3-chain
pgrep -f lsof

# 4. Check if ports are still bound
lsof -ti:9944
lsof -ti:30333

# 5. If ports still bound, use fuser to reclaim
fuser -k 9944/tcp
fuser -k 30333/tcp

# 6. Check mount points (if on NFS)
mount | grep /tmp

# 7. Restart from clean state
cargo build --release
./run-production-node.sh
```

### Network Partition (Isolated Validator)

```bash
# Detect: `system_peers` returns 0, no progress

# 1. Check network connectivity
ping 192.168.1.100  # bootnode
traceroute 192.168.1.100

# 2. Check firewall rules
sudo ufw status
sudo iptables -L -n | grep 30333

# 3. Re-add bootnode (may need to restart)
export BOOTNODE="/ip4/X.X.X.X/tcp/30333/p2p/12D3Koo..."
pkill x3-chain-node
NODE_NAME=validator-2 ./run-production-node.sh --bootnode "$BOOTNODE"

# 4. Monitor peer reconnection
watch 'curl -s http://localhost:9944 -d "{...system_peers...}" | jq ".result | length"'
```

### Catastrophic State Corruption

```bash
# Symptom: Multiple panics, inconsistent state, cannot start

# 1. Backup logs and database for forensics
tar -czf debug.tar.gz /tmp/x3-dev node.log* crash.log*

# 2. Nuke the database completely
rm -rf /tmp/x3-dev* ~/.x3/chains/x3-*

# 3. Verify binary integrity
cargo build --release --locked
cargo test -p x3-chain-runtime --lib

# 4. Start completely fresh
./run-production-node.sh

# 5. If issue persists, report with debug.tar.gz
```

---

## Appendix: Health Check Script Reference

The `scripts/x3_node_healthcheck.sh` script is your first line of defense for any node operation.

**Usage:**

```bash
# Development
bash scripts/x3_node_healthcheck.sh

# Production (enforces NODE_NAME)
NODE_NAME=validator-1 bash scripts/x3_node_healthcheck.sh --mode prod

# Strict (fail on warnings)
bash scripts/x3_node_healthcheck.sh --mode dev --strict

# Help
bash scripts/x3_node_healthcheck.sh --help
```

**Exit Codes:**
- `0` = Ready to proceed
- `1` = Critical failure (DO NOT proceed)
- `2` = Warnings in strict mode (review before proceeding)

**Checked Conditions:**
- Required commands available (cargo, bash, curl)
- Node binary exists (target/release/x3-chain-node)
- Environment variables set (NODE_NAME in prod)
- Ports are free (9944, 9945, 30333, 9615)
- App config files present (.env.local files)
- System resources sufficient (RAM, disk, CPU)

---

## Conclusion

This SOP provides procedures for safe, repeatable node operation. Always:

1. ✓ Run health check before any operation
2. ✓ Validate environment matches expectations
3. ✓ Monitor consensus progression in real-time
4. ✓ Have rollback procedures tested
5. ✓ Document all changes and issues

For questions or issues, consult the X3 Chain documentation or reach out to the core team.
