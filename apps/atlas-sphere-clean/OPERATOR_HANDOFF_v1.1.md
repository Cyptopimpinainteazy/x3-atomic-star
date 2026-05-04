# X3 Chain v1.1 Operator Handoff — Testnet Deployment Notice

**From:** X3 Chain Core Engineering  
**To:** Testnet Operators  
**Date:** March 24, 2026  
**Subject:** v1.1 Release Available for Deployment

---

## Overview

X3 Chain v1.1 is now available for testnet deployment. This release includes critical production-readiness improvements, comprehensive operator tooling, and atomic cross-VM swap functionality.

**Release Status:** ✅ SIGNED & VERIFIED  
**Target Deployment:** Testnet validators  
**Effort Estimate:** 30-45 minutes per node  

---

## What You're Receiving

### Release Package

- **File:** `x3-chain-v1.1.1.tar.gz`
- **Contents:**
  - Node binary (x3-chain-node, 54 MB)
  - Runtime WASM (824 KB)
  - Deployment scripts (health checks, launchers)
   - Config bootstrap (`config/.env.example`)
  - Complete operator documentation
  - Release notes and troubleshooting guide

### Cryptographic Verification

**CHECKSUMS.sha256** — Signed SHA-256 hash for the complete release package (`x3-chain-v1.1.1.tar.gz`)

**CHECKSUMS.bundle.sha256** — SHA-256 hashes for the extracted bundle contents:
1. Node binary
2. Runtime WASM

**CHECKSUMS.sha256.asc** — GPG detached signature (signed by X3 Chain Release key)

**Action Required:** Verify the tarball before extraction, then verify the extracted bundle contents
```bash
sha256sum -c CHECKSUMS.sha256
gpg --verify CHECKSUMS.sha256.asc CHECKSUMS.sha256
```

---

## Deployment Quick Start

### Prerequisites (5 min)

- [ ] Linux host (Ubuntu 22.04 LTS recommended)
- [ ] 16 GB RAM minimum
- [ ] 100 GB free disk space
- [ ] SSH access with sudo privileges
- [ ] `gpg` installed (for signature verification)

### Deployment Steps (30-45 min)

1. **Verify Release Integrity** (5 min)
   ```bash
   # In directory with release artifacts:
   sha256sum -c CHECKSUMS.sha256
   gpg --verify CHECKSUMS.sha256.asc CHECKSUMS.sha256

   rm -rf /tmp/x3-chain-release && mkdir -p /tmp/x3-chain-release
   tar -xzf x3-chain-v1.1.1.tar.gz -C /tmp/x3-chain-release
   (cd /tmp/x3-chain-release && sha256sum -c CHECKSUMS.bundle.sha256)
   ```

2. **Run Pre-Deployment Health Check** (5 min)
   ```bash
   NODE_NAME=my-validator \
   bash /tmp/x3-chain-release/scripts/x3_node_healthcheck.sh --mode prod
   ```
   
   **Success Criteria:**
   - ✓ Binary found and executable
   - ✓ All required ports available (9944, 30333, 9615)
   - ✓ No root privilege errors
   - ✓ Rust toolchain available

3. **Extract and Prepare** (5 min)
   ```bash
   sudo mkdir -p /opt/x3-chain
   sudo tar -xzf x3-chain-v1.1.1.tar.gz -C /opt/x3-chain
   sudo chmod +x /opt/x3-chain/x3-chain-node /opt/x3-chain/scripts/*.sh
   ```

4. **Configure Systemd Service** (5 min)
   
   Create `/etc/systemd/system/x3-chain-node.service`:
   ```
   [Unit]
   Description=X3 Chain Node
   After=network-online.target
   Wants=network-online.target

   [Service]
   Type=simple
   User=x3
   WorkingDirectory=/opt/x3-chain
   ExecStart=/opt/x3-chain/x3-chain-node \
     --chain=testnet \
     --base-path=/var/lib/x3-chain \
     --rpc-port=9944 \
     --rpc-external \
     --prometheus-external
   ExecStop=/bin/kill -SIGTERM $MAINPID
   Restart=on-failure
   RestartSec=5

   [Install]
   WantedBy=multi-user.target
   ```
   
   Then:
   ```bash
   sudo systemctl daemon-reload
   sudo systemctl enable x3-chain-node
   ```

5. **Start Node** (5 min)
   ```bash
   sudo systemctl start x3-chain-node
   sudo systemctl status x3-chain-node
   
   # Monitor startup:
   journalctl -u x3-chain-node -f
   ```

6. **Verify Operations** (5 min)
   ```bash
   # Check RPC responsiveness
   curl http://localhost:9944/health
   
   # Verify block production
   curl -X POST http://localhost:9944 \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","id":1,"method":"chain_getHeader","params":[]}'
   ```

---

## Multi-Validator Setup (Optional)

For testnet network with multiple validators:

1. **Node 1 (Bootnode):**
   ```bash
   NODE_NAME=validator-1 CHAIN=testnet ./x3-chain-node
   ```
   
   Capture from logs: `Local node identity is: 12D3KooWXXX...`

2. **Nodes 2-4:** (with bootnode)
   ```bash
   NODE_NAME=validator-2 CHAIN=testnet ./x3-chain-node \
     --bootnode "/ip4/validator-1-ip/tcp/30333/p2p/12D3KooWXXX..."
   ```

3. **Verify Connectivity:**
   ```bash
   # On each node, check peers
   curl http://localhost:9944 -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","id":1,"method":"system_peers","params":[]}'
   
   # Should show ≥3 peers (other validators)
   ```

4. **Monitor Finality:**
   ```bash
   # Watch block heights and finalized blocks
   watch 'curl -s http://localhost:9944 \
     -H "Content-Type: application/json" \
     -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"chain_getFinalizedHead\"}" \
     | jq'
   ```

---

## Key Commands

| Task | Command |
|------|---------|
| **Start** | `systemctl start x3-chain-node` |
| **Stop** | `systemctl stop x3-chain-node` |
| **Status** | `systemctl status x3-chain-node` |
| **Logs** | `journalctl -u x3-chain-node -f` |
| **Health** | `curl http://localhost:9944/health` |
| **Restart** | `systemctl restart x3-chain-node` |
| **Check Config** | View `/opt/x3-chain/config/chain-spec-testnet.json` |

---

## Troubleshooting

### Node Won't Start

**Symptom:** Service fails with "Cannot bind RPC port"

**Solution:**
```bash
# Check what's using port 9944
lsof -i :9944

# Kill conflicting process or change port in systemd service
systemctl edit x3-chain-node
# Add to [Service]: --rpc-port=9945
systemctl daemon-reload && systemctl restart x3-chain-node
```

### High Memory Usage

**Symptom:** Node memory exceeds available RAM

**Solution:**
```bash
# Check actual usage
ps aux | grep x3-chain-node

# Restart node (clears state cache)
systemctl restart x3-chain-node

# If persists, check disk space
df -h /var/lib/x3-chain
```

### Consensus Not Progressing

**Symptom:** Block height stays at genesis (#0)

**Solution:**
1. Verify other nodes are running
2. Check network connectivity: `ss -tln | grep :30333`
3. Verify peers are connecting: see health check output
4. Check logs for "authority index not found" errors

*Reference: Full troubleshooting guide in `docs/X3_OPERATOR_SOP.md`*

---

## Monitoring Setup

### Prometheus Metrics (Recommended)

Node exports metrics on port **9615**:

```bash
# Add to Prometheus scrape config:
scrape_configs:
  - job_name: 'x3-chain'
    static_configs:
      - targets: ['localhost:9615']
    scrape_interval: 15s
```

### Key Metrics to Track

- `substrate_block_height` — Current block number
- `substrate_finality_finalized_blocks_total` — Finalized block count
- `substrate_network_peer_count` — Connected peers
- `substrate_rpc_calls_total` — RPC request volume

### Log Monitoring

```bash
# Track consensus status
journalctl -u x3-chain-node -f | grep "consensus\|finality\|import"

# Track errors
journalctl -u x3-chain-node -p error -f

# Rotate logs
journalctl -u x3-chain-node --rotate
```

---

## Rollback Plan

If critical issues arise:

### Database Rollback
```bash
systemctl stop x3-chain-node
rm -rf /var/lib/x3-chain
systemctl start x3-chain-node  # Will resync from genesis or peers
```

### Binary Rollback
```bash
# Keep backup of v1.0 binary
cp /opt/x3-chain/x3-chain-node /opt/x3-chain/x3-chain-node.v1.1

# Restore v1.0
cp /opt/x3-chain/x3-chain-node.v1.0 /opt/x3-chain/x3-chain-node
systemctl restart x3-chain-node
```

### Configuration Rollback
```bash
# Restore previous chain spec
cp /opt/x3-chain/config/chain-spec-testnet.json.backup \
   /opt/x3-chain/config/chain-spec-testnet.json
systemctl restart x3-chain-node
```

---

## Success Criteria

After deployment, verify:

- [ ] Node starts without panics
- [ ] RPC endpoint responds to requests
- [ ] Block height increases (every ~6 seconds)
- [ ] Finalized block height increases (within 2 minutes)
- [ ] No "consensus stalled" or "authority index not found" errors
- [ ] Peer count shows ≥3 connected validators (multi-validator setup)
- [ ] Prometheus metrics scrape successfully

---

## Version Information

**Release:** v1.1  
**Build Date:** 2026-03-24  
**Rust Version:** 1.81 (stable)  
**Substrate:** Latest (Polkadot crate versions)  

---

## Support

### Documentation Included

Detailed operator runbook: `docs/X3_OPERATOR_SOP.md` (777 lines)

Topics covered:
- Complete pre-deployment checklist
- Single and multi-validator startup
- Monitoring and alert configuration
- 10+ troubleshooting scenarios with root causes
- Database, binary, and config rollback procedures
- Emergency incident response

### For Issues

1. **Check SOP first:** Most issues covered in troubleshooting section
2. **Run health check:** `bash scripts/x3_node_healthcheck.sh --mode prod`
3. **Capture logs:** `journalctl -u x3-chain-node -n 1000 > node-logs.txt`
4. **Report with:** Host OS, logs, reproduction steps, and output of health check

---

## Next Steps

1. **Extract and verify** the release package
2. **Read operator SOP** (`docs/X3_OPERATOR_SOP.md`)
3. **Run health check** on your deployment host
4. **Deploy and monitor** the node startup
5. **Report back** with success confirmation

---

## Acknowledgments

X3 Chain v1.1 represents the completion of Phase 8 (Testnet Proving and Go/No-Go). The release includes contributions from:

- Core Engineering (consensus, bridge, pallet layer)
- QA (test harness, integration validation)
- Operations (operator tooling, health checks, runbooks)

Thank you for supporting the X3 Chain testnet!

---

**Release Manager Sign-Off:** ✅ Ready for operator deployment  
**Date:** 2026-03-24  
**Status:** All gates passed — GO FOR DEPLOYMENT
