# Phase 6: Kubernetes Deployment Automation

**Status:** Planning Phase  
**Date:** April 25, 2026  
**Previous Phase:** Phase 5 (72/72 tests passing) ✅  
**Next Phase:** Phase 7 (Monitoring & Alerting)

---

## 📊 Phase 6 Objectives

### Primary Goals
1. **Containerize X3 Infrastructure**
   - Create Dockerfile for x3-chain-node validator
   - Create Dockerfile for x3-indexer service
   - Multi-stage builds for minimal image size

2. **Infrastructure as Code**
   - Kubernetes manifests (YAML)
   - 3-validator StatefulSet with persistent storage
   - Indexer Deployment with rollout strategy
   - Services (ClusterIP, NodePort, Ingress)

3. **Production Readiness**
   - ConfigMap for chain specs
   - Secrets for sensitive data (node keys, RPC endpoints)
   - PersistentVolumeClaims for validator data
   - Resource limits & requests

4. **Helm Chart**
   - Values for easy configuration
   - Templated deployments for multi-environment support
   - Release management & versioning

---

## 🐳 Dockerfile Specifications

### x3-chain-node Validator Dockerfile

**Build Strategy:**
- Base: `rust:1.80-slim` (build stage)
- Runtime: `ubuntu:24.04` (minimal runtime stage)
- Pre-built binary: Copy from `/target/release/x3-chain-node`

**Key Features:**
- Build from source OR copy pre-built binary
- Chain spec bundled in image
- Validator flags optimized for containerization
- Health check endpoint (:9933 RPC)

### x3-indexer Dockerfile

**Build Strategy:**
- Base: `rust:1.80-slim` (build stage)
- Runtime: `ubuntu:24.04` or `node:20-alpine` (if JavaScript/Node backend)
- Pre-built binary: Copy from `/target/release/x3-indexer`

**Key Features:**
- GraphQL endpoint (:4000)
- PostgreSQL client libraries
- Health check for GraphQL readiness

---

## ☸️ Kubernetes Architecture

### Namespace
```
kube-system (monitoring + logging)
x3-chain (workloads)
x3-indexer (indexer stack)
```

### Validator StatefulSet (x3-chain-node)

**Configuration:**
- **Replicas:** 3
- **Storage:** PersistentVolumeClaim (30GB per validator, keep-alive across restarts)
- **Update Strategy:** RollingUpdate (one pod at a time for consensus safety)
- **Resource Limits:**
  - CPU: 4 cores requested, 8 cores limit
  - Memory: 8GB requested, 16GB limit
- **Liveness Probe:** RPC endpoint check (TCP 9933 every 30s)
- **Readiness Probe:** Chain sync check (RPC call, every 10s)

**StatefulSet Identifier:**
- Pod names: `x3-validator-0`, `x3-validator-1`, `x3-validator-2`
- Persistent hostnames for peer discovery
- Node key stored in Secret, mounted as volume

### Indexer Deployment (x3-indexer)

**Configuration:**
- **Replicas:** 2 (for HA)
- **Update Strategy:** RollingUpdate
- **Resource Limits:**
  - CPU: 2 cores requested, 4 cores limit
  - Memory: 4GB requested, 8GB limit
- **Liveness Probe:** GraphQL query (HTTP 4000, every 30s)
- **Readiness Probe:** Database connectivity check (every 10s)

**Database:**
- PostgreSQL StatefulSet (1 replica for simplicity, or managed service)
- PersistentVolumeClaim (20GB)
- ConfigMap for init SQL schema

### Services

**1. Validator Peer Discovery Service (Headless)**
- Type: ClusterIP (headless, for StatefulSet peer discovery)
- Port: 30333 (P2P)
- DNS: `x3-validator-0.x3-validators.x3-chain.svc.cluster.local`

**2. Validator RPC Service**
- Type: LoadBalancer (external RPC access)
- Port: 9933 (RPC)
- Protocol: TCP

**3. Validator Metrics Service**
- Type: ClusterIP
- Port: 9616 (Prometheus metrics)
- Scrape target for monitoring

**4. Indexer GraphQL Service**
- Type: LoadBalancer (external GraphQL access)
- Port: 4000 (GraphQL)
- Protocol: HTTP

### ConfigMaps

**1. x3-chain-spec ConfigMap**
- Chain spec JSON: `x3-testnet-raw.json`
- Mounted as: `/etc/x3/chain-spec.json`

**2. x3-indexer-config ConfigMap**
- Indexer settings (RPC endpoints, DB connection)
- Mounted as: `/etc/x3-indexer/config.toml`

### Secrets

**1. Validator Node Keys**
- ed25519 private keys (one per validator)
- Mounted as: `/etc/x3/node-key-{0,1,2}`
- Generated from deterministic seed in Phase 5

**2. Database Credentials**
- PostgreSQL username/password
- Key: `pg-user`, `pg-password`

**3. RPC Credentials (Optional)**
- JWT tokens for secured RPC access
- Key: `rpc-jwt-secret`

---

## 📦 Helm Chart Structure

```
x3-helm-chart/
├── Chart.yaml                    # Chart metadata
├── values.yaml                   # Default values
├── values-testnet.yaml          # Testnet overrides
├── values-mainnet.yaml          # Mainnet overrides (future)
│
├── templates/
│   ├── namespace.yaml           # x3-chain namespace
│   ├── configmap-chain-spec.yaml
│   ├── configmap-indexer.yaml
│   ├── secret-node-keys.yaml
│   ├── secret-db-creds.yaml
│   │
│   ├── validator-statefulset.yaml
│   ├── validator-service-headless.yaml
│   ├── validator-service-rpc.yaml
│   ├── validator-service-metrics.yaml
│   │
│   ├── indexer-deployment.yaml
│   ├── indexer-service.yaml
│   ├── indexer-ingress.yaml
│   │
│   ├── postgres-statefulset.yaml  # Optional: managed DB
│   ├── postgres-service.yaml
│   ├── postgres-pvc.yaml
│   │
│   └── _helpers.tpl             # Template helpers
│
└── charts/                       # Dependency charts (optional)
    └── postgresql/               # If using Bitnami PostgreSQL chart
```

---

## 🚀 Phase 6 Implementation Plan

### Task 1: Create Dockerfiles
**Deliverables:**
- `Dockerfile.validator` (x3-chain-node)
- `Dockerfile.indexer` (x3-indexer)
- `.dockerignore` files
- Build scripts with registry push

**Success Criteria:**
- Images build without errors
- Binary size: validator <100MB, indexer <50MB
- RUN with non-root user
- Health checks functional

### Task 2: Kubernetes Manifests (Manual YAML)
**Deliverables:**
- `k8s/namespace.yaml`
- `k8s/configmap-*.yaml` (chain spec, indexer config)
- `k8s/secret-*.yaml` (node keys, DB creds)
- `k8s/statefulset-validators.yaml` (3 replicas)
- `k8s/deployment-indexer.yaml` (2 replicas)
- `k8s/service-*.yaml` (5 services: headless, RPC, metrics, indexer, DB)
- `k8s/pvc-*.yaml` (validator storage, indexer storage, DB storage)

**Success Criteria:**
- All manifests valid YAML syntax
- Pod scheduling works: `kubectl apply -f k8s/` succeeds
- Validators boot and form consensus
- Indexer connects to all 3 RPC endpoints

### Task 3: Helm Chart Creation
**Deliverables:**
- Complete helm chart with templating
- `values.yaml`, `values-testnet.yaml`, `values-mainnet.yaml`
- Template helpers for image tags, replica counts, resource limits
- Documentation: `README.md` with usage examples

**Success Criteria:**
- `helm lint x3-helm-chart/` passes
- `helm install x3-testnet x3-helm-chart/ -f values-testnet.yaml` works
- `helm upgrade x3-testnet x3-helm-chart/` performs rolling updates
- All pods reach Running state within 5 minutes

### Task 4: Storage & Persistence
**Deliverables:**
- PersistentVolumeClaim templates for validator state
- Database initialization scripts
- StorageClass configuration (if cluster-specific)
- Backup/restore procedures

**Success Criteria:**
- Validators recover their state after pod restart
- Indexer schema initialized on first deployment
- Storage persists across helm upgrades

### Task 5: Networking & Ingress
**Deliverables:**
- Ingress for external GraphQL access (indexer)
- Network policies for validator-to-validator communication
- Service DNS for internal RPC calls
- Load balancer configuration

**Success Criteria:**
- External: `curl https://indexer.x3.local/graphql` works
- Internal: Validators discover peers via DNS (`x3-validator-0.x3-validators.svc`)
- RPC accessible from outside cluster

### Task 6: Testing & Validation
**Deliverables:**
- Deployment checklist
- Pod startup verification script
- Consensus verification (all 3 validators connected)
- GraphQL query test
- Chaos testing scenarios (pod failure, network partitions)

**Success Criteria:**
- All 3 validators reach consensus in <2 minutes
- Indexer processes blocks from all 3 RPC endpoints
- GraphQL queries return data
- Pod restarts don't break consensus

---

## 📋 Execution Order

```
Phase 6a: Docker Containerization
  ├─ Create Dockerfiles for validator & indexer
  ├─ Build & test images locally
  ├─ Push to container registry (Docker Hub, ECR, etc.)
  └─ Verify image sizes & runtime

Phase 6b: Kubernetes Manifests (Manual)
  ├─ Create namespace, ConfigMaps, Secrets
  ├─ Create validator StatefulSet + Services
  ├─ Create indexer Deployment + Service
  ├─ Create PostgreSQL (if needed) + PVCs
  └─ Test: `kubectl apply -f k8s/ && watch kubectl get pods`

Phase 6c: Helm Chart
  ├─ Convert manifests to Helm templates
  ├─ Create values files (testnet, mainnet)
  ├─ Document chart usage
  ├─ Test: `helm install x3-testnet ./chart`
  └─ Validate multi-environment support

Phase 6d: Testing & Hardening
  ├─ Test pod lifecycle (create, scale, delete, restart)
  ├─ Verify consensus recovery after failures
  ├─ Test indexer recovery
  ├─ Chaos testing (network partition, pod kill, disk full)
  └─ Load testing (RPC request throughput)

Phase 6e: Documentation & Runbooks
  ├─ Deployment guide (fresh cluster setup)
  ├─ Upgrade procedures
  ├─ Emergency recovery procedures
  ├─ Monitoring setup (Prometheus scrape config)
  └─ Troubleshooting guide
```

---

## 🎯 Phase 6 Success Criteria

### Go/No-Go Gates

| Criterion | Target | Verification |
|-----------|--------|--------------|
| Dockerfiles | Both validator & indexer build | `docker build` exit code 0 |
| K8s Manifests | All YAML valid | `kubectl apply --dry-run=client` succeeds |
| Helm Chart | Lints without errors | `helm lint x3-helm-chart/` passes |
| Pod Startup | All 3 validators + indexer Running | `kubectl get pods` shows all Running |
| Consensus | All validators synced | RPC call: `system_peers` returns 3 peers |
| Indexer GraphQL | Responding to queries | `curl graphql-endpoint` returns data |
| Persistence | Validators recover state | Kill pod, verify state restored on restart |
| Helm Upgrade | Rolling update without downtime | `helm upgrade` completes, consensus maintained |
| Scaling | Can increase/decrease replicas | `kubectl scale statefulset x3-validator --replicas=5` works |
| Network | External access to RPC & GraphQL | `curl` from outside cluster succeeds |

---

## 📈 Post-Phase 6 Roadmap

### Phase 7: Monitoring & Alerting
- Prometheus scrape config for validator metrics
- Grafana dashboards (validator health, block production, indexer lag)
- AlertManager rules (consensus failure, RPC errors, storage full)

### Phase 8: Load Testing & Performance
- Sustained RPC load test (1000 req/s)
- Settlement transaction throughput benchmarks
- Indexer query latency measurements

### Phase 9: Mainnet Preparation
- Multi-region deployment (HA across availability zones)
- Backup & disaster recovery procedures
- Security hardening (pod security policies, RBAC)
- Mainnet genesis configuration

---

## 🔗 Resources & References

- **Kubernetes StatefulSet:** https://kubernetes.io/docs/concepts/workloads/controllers/statefulset/
- **Helm Chart Best Practices:** https://helm.sh/docs/chart_best_practices/
- **Polkadot Node Deployment:** https://github.com/paritytech/polkadot/tree/master/scripts/ci/docker
- **Container Image Optimization:** https://www.cisecurity.org/benchmark/docker

---

**Next Step:** Begin Phase 6a with Dockerfile creation for x3-chain-node validator.
