# 🚀 PHASE 5: PUBLIC TESTNET DEPLOYMENT EXECUTION PLAN

**Status:** Ready for Execution  
**Date:** April 26, 2026  
**Objective:** Deploy X3 blockchain to public testnet with production monitoring  

---

## 📋 DEPLOYMENT STRATEGY

This Phase 5 execution encompasses **5 parallel workstreams**:

### Workstream 1: Production Node Deployment 🖥️
- Launch 3+ public validator nodes
- Configure RPC gateways for public access
- Setup node monitoring and logging
- Verify peer connectivity and consensus

### Workstream 2: Monitoring Infrastructure 📊
- Deploy Prometheus server
- Configure Grafana dashboards
- Setup alert rules
- Enable log aggregation

### Workstream 3: Settlement/GPU/Consensus Deep Dives 🔬
- Review settlement timeout engine in detail
- Analyze GPU sidecar health monitoring
- Verify peer consensus and finalization
- Generate performance baselines

### Workstream 4: Docker/Kubernetes Deployment 🐳
- Build production Docker images
- Generate Kubernetes manifests
- Deploy to K8s cluster
- Verify auto-scaling and failover

### Workstream 5: Production Hardening 🔒
- Enable security monitoring
- Configure firewall rules
- Setup key management
- Run pre-launch validation suite

---

## 🎯 EXECUTION PHASES

### PHASE 5A: Infrastructure Preparation (Immediate)

**Duration:** 15-30 minutes  
**Tasks:**

1. **Review & Validate Deployment Configuration**
   ```bash
   ✓ OPTION_D_LAUNCH_GUIDE.md                 - Already reviewed
   ✓ TESTNET_DEPLOYMENT_GUIDE.md              - Already reviewed
   ✓ TESTNET_PRE_DEPLOYMENT_CHECKLIST.md      - Status verified
   ✓ Verify K8s manifests in /deployment/k8s
   ✓ Review Docker configs in /deployment/docker
   ```

2. **Verify Binary Status**
   ```bash
   # Check if x3-chain-node built
   ls -lh /home/lojak/Desktop/X3_ATOMIC_STAR/target/release/x3-chain-node
   
   # If not, trigger build
   cargo build --release -p x3-chain-node
   ```

3. **Prepare Node Configuration Files**
   ```bash
   # Chain spec for public testnet
   cp deployment/chain-specs/x3-testnet-raw.json deployment/chain-specs/x3-public-testnet.json
   
   # Generate validator keys (3 validators)
   python3 deployment/generate-keys-only.sh 3
   ```

4. **Setup Public RPC Endpoints**
   ```bash
   # Create RPC gateway config
   mkdir -p deployment/rpc-config
   cat > deployment/rpc-config/rpc-gateway.conf
   ```

---

### PHASE 5B: Deep Component Review (20 minutes)

Review and document each critical system:

#### 1. Settlement Timeout Engine Deep Dive
- **File:** `pallets/x3-settlement-engine/src/lib.rs`
- **Focus:** 
  - 28,800-block timeout configuration
  - Deadline index O(1) lookup mechanism
  - Auto-refund logic in on_idle hook
  - Event emission patterns

#### 2. GPU Sidecar Health Monitor Deep Dive
- **File:** `node/src/service.rs`
- **Focus:**
  - 5-block health check intervals
  - 3-failure restart threshold
  - Prometheus metrics export format
  - Logging configuration levels

#### 3. Peer Consensus & Finalization Deep Dive
- **File:** `runtime/src/lib.rs`
- **Focus:**
  - GRANDPA finalization configuration
  - BABE slot timing (12 seconds)
  - Epoch duration (14,400 blocks)
  - Validator set management

---

### PHASE 5C: Monitoring Stack Deployment (25 minutes)

1. **Deploy Prometheus Server**
   ```bash
   docker-compose -f deployment/docker-compose.monitoring.yml up -d
   
   # Verify Prometheus accessible
   curl http://localhost:9090/-/healthy
   ```

2. **Configure Grafana Dashboards**
   ```bash
   # Import dashboards from deployment/dashboards/
   # Setup data source: Prometheus (localhost:9090)
   # Create custom dashboards for:
   # - Settlement timeout tracking
   # - GPU health metrics
   # - Consensus finalization
   # - Network statistics
   ```

3. **Configure Alert Rules**
   ```bash
   # Update prometheus.yml with alert rules
   # Key alerts:
   # - Settlement timeout > 28,800 blocks
   # - GPU failures > 3 consecutive
   # - Validator consensus fork detected
   # - RPC endpoint down
   ```

---

### PHASE 5D: Node Deployment (30 minutes)

#### Option 1: Bare Metal Deployment

```bash
# Terminal 1: Validator-1 (Public RPC enabled)
./target/release/x3-chain-node \
  --chain deployment/chain-specs/x3-public-testnet.json \
  --validator --name "Validator-1-Public" \
  --base-path /mnt/validator1 \
  --port 30333 --rpc-port 9944 \
  --rpc-external --rpc-cors all \
  --prometheus-external \
  --pruning archive \
  --state-cache-size 64 \
  2>&1 | tee /var/log/x3-validator1.log

# Terminal 2: Validator-2
./target/release/x3-chain-node \
  --chain deployment/chain-specs/x3-public-testnet.json \
  --validator --name "Validator-2-Public" \
  --base-path /mnt/validator2 \
  --port 30334 --rpc-port 9945 \
  --rpc-external --rpc-cors all \
  --prometheus-external \
  --pruning archive \
  --bootnodes "/ip4/<VALIDATOR1_IP>/tcp/30333/p2p/<VALIDATOR1_PEER_ID>" \
  2>&1 | tee /var/log/x3-validator2.log

# Terminal 3: Validator-3
./target/release/x3-chain-node \
  --chain deployment/chain-specs/x3-public-testnet.json \
  --validator --name "Validator-3-Public" \
  --base-path /mnt/validator3 \
  --port 30335 --rpc-port 9946 \
  --rpc-external --rpc-cors all \
  --prometheus-external \
  --pruning archive \
  --bootnodes "/ip4/<VALIDATOR1_IP>/tcp/30333/p2p/<VALIDATOR1_PEER_ID>" \
  2>&1 | tee /var/log/x3-validator3.log
```

#### Option 2: Kubernetes Deployment

```bash
# Create namespace
kubectl create namespace x3-testnet

# Apply K8s manifests in order
kubectl apply -f deployment/k8s/01-namespace.yaml
kubectl apply -f deployment/k8s/02-configmaps.yaml
kubectl apply -f deployment/k8s/03-secrets.yaml
kubectl apply -f deployment/k8s/04-pvcs.yaml
kubectl apply -f deployment/k8s/05-validators-statefulset.yaml
kubectl apply -f deployment/k8s/06-indexer-deployment.yaml
kubectl apply -f deployment/k8s/07-postgres-statefulset.yaml

# Verify deployment
kubectl get pods -n x3-testnet
kubectl logs -n x3-testnet -f x3-validator-0
```

#### Option 3: Docker Compose Deployment

```bash
cd deployment/docker
docker-compose -f docker-compose.production.yml up -d

# Verify services
docker-compose ps
docker logs -f x3-validator-1
```

---

### PHASE 5E: Validation & Testing (20 minutes)

1. **Verify RPC Connectivity**
   ```bash
   # Query node info
   curl -H "Content-Type: application/json" \
     -d '{"id":1,"jsonrpc":"2.0","method":"system_nodeInfo"}' \
     http://localhost:9944
   
   # Get chain state
   curl -H "Content-Type: application/json" \
     -d '{"id":1,"jsonrpc":"2.0","method":"chain_getHeader"}' \
     http://localhost:9944
   ```

2. **Verify Validator Consensus**
   ```bash
   # Check peer connectivity
   curl -H "Content-Type: application/json" \
     -d '{"id":1,"jsonrpc":"2.0","method":"system_peers"}' \
     http://localhost:9944
   
   # Monitor blocks being produced
   curl -H "Content-Type: application/json" \
     -d '{"id":1,"jsonrpc":"2.0","method":"chain_subscribeNewHeads"}' \
     http://localhost:9944
   ```

3. **Execute Option D Validation Suite on Production**
   ```bash
   bash /tmp/option_d_orchestrator.sh --production-mode
   ```

4. **Verify All 3 Systems Operational**
   ```bash
   # Settlement timeout tracking active
   # GPU health checks running
   # Consensus rounds progressing
   # Events being emitted
   # Metrics available on Prometheus
   ```

---

## 📊 MONITORING DASHBOARD CONFIGURATION

### Prometheus Metrics to Track

```yaml
# Settlement Engine Metrics
settlement_timeout_enforced_total
settlement_refunded_total
settlement_deadline_processed
settlement_pending_count

# GPU Sidecar Metrics
gpu_health_checks_total
gpu_failures_consecutive
gpu_restarts_triggered
gpu_uptime_blocks

# Consensus Metrics
consensus_rounds_completed
consensus_forks_detected
consensus_finalization_latency_ms
validator_block_produced_total
validator_block_attested_total
```

### Grafana Dashboard Panels

1. **Settlement System Dashboard**
   - Timeline of settled intents
   - Timeout enforcement rate
   - Auto-refund volume

2. **GPU Health Dashboard**
   - Health check results over time
   - Failure rate and recovery time
   - Uptime percentage

3. **Consensus Dashboard**
   - Validator peer connectivity
   - Block finalization latency
   - Fork detection alerts

4. **Network Dashboard**
   - RPC request latency
   - Active connections
   - Data transfer rate

---

## 🔒 PRODUCTION HARDENING CHECKLIST

- [ ] Enable firewall rules (only required ports)
- [ ] Configure SSH key authentication
- [ ] Setup log rotation
- [ ] Enable backup procedures
- [ ] Configure rate limiting on RPC
- [ ] Setup DDoS protection
- [ ] Enable security monitoring
- [ ] Setup incident response procedures
- [ ] Configure certificate management
- [ ] Enable audit logging

---

## 📈 SUCCESS CRITERIA

✅ **Phase 5 Complete When:**

1. ✅ 3+ public validators running and synced
2. ✅ RPC endpoints publicly accessible and responsive
3. ✅ Prometheus metrics exporting for all systems
4. ✅ Grafana dashboards displaying real-time data
5. ✅ Settlement timeout engine tracking intents
6. ✅ GPU health monitor reporting health status
7. ✅ Consensus rounds progressing normally
8. ✅ All events being emitted correctly
9. ✅ Option D validation suite passing in production mode
10. ✅ No critical errors in logs

---

## 🚨 EMERGENCY PROCEDURES

### If Consensus Breaks
```bash
# Check validator logs
tail -f /var/log/x3-validator*.log | grep -i "grandpa\|consensus"

# Verify peer connectivity
curl http://localhost:9944/system_peers

# Check block production rate
curl http://localhost:9944/chain_getHeader
```

### If GPU Monitor Fails
```bash
# Restart GPU monitoring service
systemctl restart x3-gpu-monitor

# Check GPU status
nvidia-smi

# Review logs
journalctl -u x3-gpu-monitor -f
```

### If Settlement Timeout Stalls
```bash
# Check settlement pallet state
curl -X POST http://localhost:9944 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"state_getStorage","params":["0x...deadline_index"]}'

# Verify on_idle hook executing
grep -i "on_idle" /var/log/x3-validator*.log
```

---

## 📞 QUICK REFERENCE

### Key Endpoints (After Deployment)

```
Validator 1 RPC:    http://<PUBLIC_IP>:9944
Validator 2 RPC:    http://<PUBLIC_IP>:9945
Validator 3 RPC:    http://<PUBLIC_IP>:9946

Prometheus:         http://<PUBLIC_IP>:9090
Grafana:            http://<PUBLIC_IP>:3000
Indexer (SubQuery): http://<PUBLIC_IP>:3001
```

### Key Log Files

```
Validator 1:        /var/log/x3-validator1.log
Validator 2:        /var/log/x3-validator2.log
Validator 3:        /var/log/x3-validator3.log
GPU Monitor:        /var/log/x3-gpu-monitor.log
Consensus Events:   /var/log/x3-consensus.log
```

### Key Configuration Files

```
Chain Spec:         deployment/chain-specs/x3-public-testnet.json
Prometheus Config:  deployment/monitoring/prometheus.yml
Grafana Dashboards: deployment/dashboards/*.json
K8s Manifests:      deployment/k8s/*.yaml
Docker Compose:     deployment/docker/docker-compose.production.yml
```

---

## ⏱️ ESTIMATED TIMELINE

| Phase | Duration | Status |
|-------|----------|--------|
| **5A: Infrastructure Prep** | 15-30 min | ⏳ Ready |
| **5B: Component Review** | 20 min | ⏳ Ready |
| **5C: Monitoring Setup** | 25 min | ⏳ Ready |
| **5D: Node Deployment** | 30 min | ⏳ Ready |
| **5E: Validation** | 20 min | ⏳ Ready |
| **TOTAL** | **~2 hours** | 🚀 **Ready to Launch** |

---

## 🎯 NEXT COMMAND

```bash
# Start Phase 5A: Infrastructure Preparation
bash deployment/DEPLOYMENT_READY.sh
```

**Ready to deploy? Let's execute Phase 5!** 🚀
