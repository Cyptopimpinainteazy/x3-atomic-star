# X3 Chain Graceful Shutdown Guide

Complete reference for safe and graceful node shutdown procedures.

## Table of Contents

1. [Shutdown Overview](#shutdown-overview)
2. [Graceful Shutdown Procedure](#graceful-shutdown-procedure)
3. [Signal Handling](#signal-handling)
4. [Timeout Behavior](#timeout-behavior)
5. [State Verification](#state-verification)
6. [Troubleshooting](#troubleshooting)
7. [Systemd Integration](#systemd-integration)
8. [Monitoring Shutdown](#monitoring-shutdown)

---

## Shutdown Overview

### Why Graceful Shutdown?

Graceful shutdown ensures:
- ✅ Finality votes are flushed to disk
- ✅ Database is cleanly closed
- ✅ Network peers are notified
- ✅ No data corruption on restart
- ✅ State can resume from last finalized block

### Shutdown Phases

The graceful shutdown process:

```
Signal Reception (SIGTERM/SIGINT)
        ↓
Finality Flush (GRANDPA/Flash votes)
        ↓
Database Commit (all pending writes)
        ↓
Connection Closure (peer disconnection)
        ↓
Exit (return code 0 = success)
```

---

## Graceful Shutdown Procedure

### Manual Shutdown (Interactive)

**Easiest method for development:**

```bash
# Terminal 1: Start node
x3-chain-node --dev

# Terminal 2: Send shutdown signal
# Option A: SIGTERM (preferred)
kill -TERM <pid>

# Option B: SIGINT (Ctrl+C in terminal)
# Just press Ctrl+C in Terminal 1

# Option C: SIGQUIT (with backtrace)
# For debugging, get stack trace before exit
kill -QUIT <pid>
```

**Expected Log Output**:
```
INFO: Shutdown signal received (SIGTERM)
INFO: Flushing finality component (pending: 5 votes)
INFO: Committing block #12345 to database
INFO: Closing network: disconnecting 8 peers
INFO: Closing RPC API endpoint
INFO: Graceful shutdown complete
INFO: Exiting with code 0
```

**Verification**:
```bash
# Check exit code (0 = success)
echo $?

# Next startup should show:
# INFO: Re-entering consensus from block #12345
```

### Programmatic Shutdown (Background Process)

**For unattended nodes:**

```bash
# Start in background
x3-chain-node --dev > logs/node.log 2>&1 &
NODE_PID=$!
echo $NODE_PID > node.pid

# Run some operations...
sleep 600

# Graceful shutdown
kill -TERM $NODE_PID
TIMEOUT=30
WAITED=0

while kill -0 $NODE_PID 2>/dev/null; do
  if [ $WAITED -ge $TIMEOUT ]; then
    echo "Timeout: force killing process"
    kill -9 $NODE_PID
    break
  fi
  sleep 1
  WAITED=$((WAITED + 1))
done

# Check result
EXIT_CODE=$?
if [ $EXIT_CODE -eq 0 ]; then
  echo "Clean shutdown"
else
  echo "Force shutdown (exit code: $EXIT_CODE)"
fi
```

### Production Shutdown Script

**Safe shutdown for critical infrastructure:**

```bash
#!/bin/bash
set -euo pipefail

NODE_PID="$1"
SHUTDOWN_TIMEOUT=30
FORCE_TIMEOUT=60

if [ -z "$NODE_PID" ]; then
  echo "Usage: $0 <pid>"
  exit 1
fi

# Verify process exists
if ! kill -0 "$NODE_PID" 2>/dev/null; then
  echo "Process not found: $NODE_PID"
  exit 1
fi

echo "Initiating graceful shutdown for process $NODE_PID..."

# Send SIGTERM
echo "Sending SIGTERM..."
kill -TERM "$NODE_PID"

# Wait for graceful exit
ELAPSED=0
while [ $ELAPSED -lt $SHUTDOWN_TIMEOUT ]; do
  if ! kill -0 "$NODE_PID" 2>/dev/null; then
    echo "Process exited cleanly after $ELAPSED seconds"
    exit 0
  fi
  sleep 1
  ELAPSED=$((ELAPSED + 1))
  echo "Waiting... ($ELAPSED/$SHUTDOWN_TIMEOUT)"
done

# Still running? Try SIGQUIT for backtrace
echo "Timeout waiting for graceful exit. Sending SIGQUIT for backtrace..."
kill -QUIT "$NODE_PID" 2>/dev/null || true
sleep 2

# Still running? Force kill
if kill -0 "$NODE_PID" 2>/dev/null; then
  echo "Force killing process..."
  kill -9 "$NODE_PID"
  sleep 1
fi

echo "Shutdown complete"
exit 0
```

---

## Signal Handling

### Supported Signals

| Signal | Behavior | Use Case |
|--------|----------|----------|
| SIGTERM | Graceful shutdown | Deployment/scaling |
| SIGINT | Graceful shutdown | Ctrl+C in terminal |
| SIGQUIT | Graceful + backtrace | Debugging hanging nodes |
| SIGKILL | Force exit (no cleanup) | Last resort only |
| SIGSTOP | Pause (no shutdown) | Temporary suspension |

### SIGTERM (Preferred)

```bash
# Initiates graceful shutdown
kill -TERM <pid>

# Default behavior of systemctl stop
systemctl stop x3-chain-node
```

**Guaranteed cleanup**:
- Finality votes flushed
- Database committed
- Peers disconnected
- Exit code 0 on success

### SIGINT (Ctrl+C)

```bash
# Same as SIGTERM
# Triggered by Ctrl+C in terminal
kill -INT <pid>   # Alternative notation
```

**Behavior**: Identical to SIGTERM

### SIGQUIT (Backtrace)

```bash
# Graceful shutdown with debug backtrace
kill -QUIT <pid>

# Output:
# Thread 'Finality' backtrace...
# Signal: received SIGQUIT, starting shutdown
# ...graceful cleanup...
```

**Use When**:
- Node appears to be hanging
- Need to debug what code is running
- Investigating performance issues

### SIGKILL (Force)

```bash
# LAST RESORT ONLY - No cleanup!
kill -9 <pid>

# WARNING: Can cause:
# - Data corruption
# - Inconsistent state
# - Longer restart time
```

**When to use**: Only if graceful shutdown completely unresponsive for 60+ seconds

---

## Timeout Behavior

### Default Timeout: 30 Seconds

If graceful shutdown exceeds 30 seconds:

```
T=0s:  SIGTERM received
T=30s: Timeout exceeded
       → All tasks cancelled
       → Database force-closed
       → Process exits with code 1
```

### Timeout Extension

For nodes with large state:

```bash
# Edit systemd service
sudo systemctl edit x3-chain-node

# Add:
[Service]
TimeoutStopSec=120  # 2-minute timeout
```

Or via script:

```bash
# Wait up to 60 seconds
SHUTDOWN_TIMEOUT=60
kill -TERM $PID
sleep $SHUTDOWN_TIMEOUT
if kill -0 $PID 2>/dev/null; then
  kill -9 $PID  # Force if still running
fi
```

### Logs During Timeout

```
INFO: Shutdown signal received
INFO: Flushing finality (32 pending votes)...
WARN: Still waiting for finality flush (10s elapsed)
WARN: Still waiting for finality flush (20s elapsed)
ERROR: Shutdown timeout exceeded (30s)
ERROR: Force-closing database
ERROR: Exiting with code 1
```

---

## State Verification

### Post-Shutdown Checks

**Verify clean shutdown:**

```bash
# 1. Check exit code (0 = graceful)
echo $?

# 2. Check last block commited
tail -20 logs/node.log | grep "Block finalized"

# 3. Check database integrity
x3-chain-node build-spec --chain=dev > /dev/null && echo "DB OK"

# 4. Verify RocksDB not locked
lsof +D ~/.local/share/x3-chain-node/ 2>/dev/null || echo "Not in use"
```

### Restart Verification

**Confirm successful recovery:**

```bash
# Start node
x3-chain-node --dev &
NODE_PID=$!
sleep 5

# Verify resumption from correct block
tail logs/node.log | grep "Resuming from block"

# Should show: "Resuming from block #12345"
# NOT: "Starting new chain at genesis"
```

### Database Integrity Check

```bash
# Check RocksDB state
x3-chain-node build-spec --chain=dev --raw > /tmp/test.json

# Verify no corruption
if jq empty /tmp/test.json; then
  echo "Database state valid"
else
  echo "Database corrupted"
fi
```

---

## Troubleshooting

### Hang on Shutdown

**Symptoms**: Process doesn't exit after SIGTERM

**Diagnosis**:
```bash
# Get full backtrace
kill -QUIT <pid>
sleep 2

# If still running, check logs
tail -100 logs/node.log | grep -i "finality\|block\|error"
```

**Solutions**:
```bash
# Option 1: Wait longer (increase timeout)
TIMEOUT=60; kill -TERM $PID; sleep $TIMEOUT

# Option 2: Check for network hang
# See "Stalled Network" below

# Option 3: Force shutdown
kill -9 $PID
```

### Stalled Network

**Symptoms**: Shutdown hangs waiting for peer disconnection

**Diagnosis**:
```bash
# Check peer connections
netstat -tln | grep 30333

# Monitor in background
watch 'netstat -tln | grep 30333 | wc -l'
```

**Solution**:
```bash
# Close connections manually
sudo ss -K dst 127.0.0.1 dport 30333

# Proceed with forced shutdown
kill -9 $PID
```

### Database Lock

**Symptoms**: Can't restart after shutdown - "database already in use"

**Diagnosis**:
```bash
# Find process holding lock
lsof ~/.local/share/x3-chain-node/.

# View lock files
ls -la ~/.local/share/x3-chain-node/
```

**Solution**:
```bash
# Remove lock file (if no process holding it)
rm -f ~/.local/share/x3-chain-node/LOCK

# Or forcefully kill holder
pkill -9 rust  # Only if safe!

# Verify clean before restart
x3-chain-node purge-chain --chain=dev -y
x3-chain-node --dev
```

### Data Corruption After Shutdown

**Symptoms**: Can't restart - "corrupled database" or state errors

**Recovery**:
```bash
# Option 1: Purge and resync
x3-chain-node purge-chain --chain=staging -y
x3-chain-node --chain=staging  # Will resync from network

# Option 2: Restore from backup
rsync -av backups/x3-chain-node/ ~/.local/share/x3-chain-node/
x3-chain-node --chain=staging
```

---

## Systemd Integration

### Service File

**Create `/etc/systemd/system/x3-chain-node.service`:**

```ini
[Unit]
Description=X3 Chain Node
Documentation=https://x3-chain.io/docs
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=x3-node
Group=x3-node
WorkingDirectory=/opt/x3-chain-node
ExecStart=/opt/x3-chain-node/x3-chain-node \
  --chain=staging \
  --name=validator-1 \
  --validator \
  --pruning=1000 \
  --db=rocksdb

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=/var/lib/x3-chain-node

# Shutdown behavior
KillSignal=SIGTERM
KillMode=mixed
TimeoutStopSec=120

# Auto-restart on crash
Restart=on-failure
RestartSec=30

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=x3-chain-node

[Install]
WantedBy=multi-user.target
```

### Managing Systemd Service

```bash
# Install
sudo systemctl daemon-reload
sudo systemctl enable x3-chain-node

# Start
sudo systemctl start x3-chain-node

# Graceful stop (respects TimeoutStopSec)
sudo systemctl stop x3-chain-node

# Check status
sudo systemctl status x3-chain-node

# View logs
journalctl -u x3-chain-node -f

# Force stop (if hangs)
sudo systemctl kill -s KILL x3-chain-node
```

---

## Monitoring Shutdown

### Real-time Monitoring

```bash
#!/bin/bash

PID=$1
echo "Monitoring shutdown of PID $PID"

while kill -0 $PID 2>/dev/null; do
  # Show memory
  ps aux | grep $PID | grep -v grep
  
  # Show open files
  echo "Open connections:"
  lsof -p $PID 2>/dev/null | grep -E "TCP|UDP" | wc -l
  
  sleep 1
done

echo "Process exited"
```

### Log Monitoring During Shutdown

```bash
# In one terminal
tail -f logs/node.log | grep -E "Shutdown|Finality|Database|Closing"

# In another terminal
kill -TERM <pid>

# Watch logs for shutdown sequence
```

### Metrics Before Shutdown

```bash
#!/bin/bash

# Record metrics before shutdown
curl http://localhost:9615/metrics > /tmp/metrics-before.txt 2>/dev/null

# Do shutdown
kill -TERM $PID
wait $PID

# Record metrics after restart
sleep 10  # Wait for restart
curl http://localhost:9615/metrics > /tmp/metrics-after.txt 2>/dev/null

# Compare
diff /tmp/metrics-before.txt /tmp/metrics-after.txt
```

---

## Shutdown Checklist

Before critical shutdown:

- [x] Record current block height
- [x] Verify finality is recent (< 1 min)
- [x] Check no pending authority changes
- [x] Notify other infrastructure nodes
- [x] Set adequate timeout (>30s)
- [x] Have rollback plan if restart fails
- [x] Verify database backups exist
- [x] Test graceful shutdown on staging first

---

## Appendix: Signal Reference

```bash
# Send signals
kill -TERM <pid>   # SIGTERM (graceful)
kill -INT <pid>    # SIGINT (same as SIGTERM)
kill -QUIT <pid>   # SIGQUIT (graceful + backtrace)
kill -9 <pid>      # SIGKILL (force, no cleanup!)

# Monitor running process
ps aux | grep x3-chain-node
pgrep -a x3-chain-node
pidof x3-chain-node
```

---

## See Also

- [DEVELOPMENT.md](./DEVELOPMENT.md) - Node development guide
- [CONFIG.md](./CONFIG.md) - Configuration reference
- [Node Checklist](./masterchecklist.md#21-node-requirements)
- [Systemd Documentation](https://www.freedesktop.org/software/systemd/man/systemd.service.html)
