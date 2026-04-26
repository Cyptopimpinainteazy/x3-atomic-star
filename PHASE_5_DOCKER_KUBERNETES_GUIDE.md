# 🐳 DOCKER & KUBERNETES DEPLOYMENT GUIDE

**Production-Ready Container Deployment for X3 Blockchain**  
**Date:** April 26, 2026  

---

## 📋 TABLE OF CONTENTS

1. Docker Image Building
2. Docker Compose Deployment
3. Kubernetes Architecture
4. Helm Chart Setup
5. Auto-Scaling Configuration
6. Failover & High Availability
7. Troubleshooting Guide

---

## 1️⃣ DOCKER IMAGE BUILDING

### Multi-Stage Build Dockerfile

Create `Dockerfile.x3-node`:

```dockerfile
# Stage 1: Builder
FROM rust:1.89.0-slim as builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    curl \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy source
COPY . /build/

# Build release binary
RUN cargo build --release -p x3-chain-node 2>&1 | tee /tmp/build.log

# Check if build succeeded
RUN if [ ! -f /build/target/release/x3-chain-node ]; then \
    echo "Build failed!"; \
    cat /tmp/build.log; \
    exit 1; \
fi

# Stage 2: Runtime
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    jq \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /build/target/release/x3-chain-node /app/x3-chain-node

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:9933/system_nodeInfo || exit 1

# Expose ports
EXPOSE 9933 9944 30333 9615

# Default command
ENTRYPOINT ["/app/x3-chain-node"]
CMD ["--chain", "dev", "--tmp", "--rpc-external", "--rpc-cors", "all"]
```

### Build Command

```bash
# Build Docker image
docker build -f Dockerfile.x3-node -t x3-blockchain:latest .

# Tag for registry
docker tag x3-blockchain:latest myregistry.azurecr.io/x3-blockchain:latest

# Push to registry
docker push myregistry.azurecr.io/x3-blockchain:latest

# Verify image
docker images x3-blockchain:latest
docker inspect x3-blockchain:latest
```

### GPU-Enabled Dockerfile

Create `Dockerfile.x3-node-gpu`:

```dockerfile
# Stage 1: Builder with GPU support
FROM nvidia/cuda:12.0-devel-ubuntu22.04 as builder

WORKDIR /build

# Install Rust and dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    curl \
    git \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

ENV PATH="/root/.cargo/bin:${PATH}"

COPY . /build/

# Build with GPU features
RUN cargo build --release -p x3-chain-node --features gpu-validator

# Stage 2: GPU Runtime
FROM nvidia/cuda:12.0-runtime-ubuntu22.04

WORKDIR /app

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/x3-chain-node /app/x3-chain-node

EXPOSE 9933 9944 30333 9615

ENTRYPOINT ["/app/x3-chain-node"]
CMD ["--chain", "dev", "--tmp", "--rpc-external"]
```

---

## 2️⃣ DOCKER COMPOSE DEPLOYMENT

### Production Docker Compose

Create `docker-compose.production.yml`:

```yaml
version: '3.8'

services:
  # PostgreSQL for indexing
  postgres:
    image: postgres:15-alpine
    container_name: x3-postgres
    environment:
      POSTGRES_USER: x3
      POSTGRES_PASSWORD: secure_password
      POSTGRES_DB: x3_indexer
    volumes:
      - postgres_data:/var/lib/postgresql/data
    ports:
      - "5432:5432"
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U x3"]
      interval: 10s
      timeout: 5s
      retries: 5

  # Prometheus for metrics
  prometheus:
    image: prom/prometheus:latest
    container_name: x3-prometheus
    volumes:
      - ./deployment/monitoring/prometheus.yml:/etc/prometheus/prometheus.yml
      - ./deployment/monitoring/rules:/etc/prometheus/rules
      - prometheus_data:/prometheus
    ports:
      - "9090:9090"
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--storage.tsdb.retention.time=30d'

  # Grafana for dashboards
  grafana:
    image: grafana/grafana:latest
    container_name: x3-grafana
    environment:
      GF_SECURITY_ADMIN_PASSWORD: admin
      GF_USERS_ALLOW_SIGN_UP: 'false'
    volumes:
      - grafana_data:/var/lib/grafana
      - ./deployment/dashboards:/etc/grafana/provisioning/dashboards
    ports:
      - "3000:3000"
    depends_on:
      - prometheus

  # Validator 1
  validator-1:
    image: x3-blockchain:latest
    container_name: x3-validator-1
    command: >
      /app/x3-chain-node
      --chain ./deployment/chain-specs/x3-public-testnet.json
      --validator
      --name "Validator-1"
      --base-path /data/validator-1
      --port 30333
      --rpc-port 9933
      --rpc-external
      --rpc-cors all
      --prometheus-external
      --prometheus-port 9615
      --pruning archive
    volumes:
      - ./deployment/chain-specs:/deployment/chain-specs
      - validator1_data:/data/validator-1
    ports:
      - "9933:9933"
      - "30333:30333"
      - "9615:9615"
    environment:
      RUST_LOG: info,x3_runtime=debug
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:9933/system_nodeInfo"]
      interval: 30s
      timeout: 5s
      retries: 3
    restart: unless-stopped

  # Validator 2
  validator-2:
    image: x3-blockchain:latest
    container_name: x3-validator-2
    command: >
      /app/x3-chain-node
      --chain ./deployment/chain-specs/x3-public-testnet.json
      --validator
      --name "Validator-2"
      --base-path /data/validator-2
      --port 30334
      --rpc-port 9934
      --rpc-external
      --rpc-cors all
      --prometheus-external
      --prometheus-port 9616
      --pruning archive
      --bootnodes "/dns4/validator-1/tcp/30333/p2p/12D3KooWNMSBgEWpJQhRv9MEhS3LgTRXt7FEUMhtYA8TaB1tdPsw"
    volumes:
      - ./deployment/chain-specs:/deployment/chain-specs
      - validator2_data:/data/validator-2
    ports:
      - "9934:9934"
      - "30334:30334"
      - "9616:9616"
    environment:
      RUST_LOG: info,x3_runtime=debug
    depends_on:
      - validator-1
    restart: unless-stopped

  # Validator 3
  validator-3:
    image: x3-blockchain:latest
    container_name: x3-validator-3
    command: >
      /app/x3-chain-node
      --chain ./deployment/chain-specs/x3-public-testnet.json
      --validator
      --name "Validator-3"
      --base-path /data/validator-3
      --port 30335
      --rpc-port 9935
      --rpc-external
      --rpc-cors all
      --prometheus-external
      --prometheus-port 9617
      --pruning archive
      --bootnodes "/dns4/validator-1/tcp/30333/p2p/12D3KooWNMSBgEWpJQhRv9MEhS3LgTRXt7FEUMhtYA8TaB1tdPsw"
    volumes:
      - ./deployment/chain-specs:/deployment/chain-specs
      - validator3_data:/data/validator-3
    ports:
      - "9935:9935"
      - "30335:30335"
      - "9617:9617"
    environment:
      RUST_LOG: info,x3_runtime=debug
    depends_on:
      - validator-1
    restart: unless-stopped

volumes:
  postgres_data:
  prometheus_data:
  grafana_data:
  validator1_data:
  validator2_data:
  validator3_data:

networks:
  default:
    name: x3-network
```

### Docker Compose Commands

```bash
# Start all services
docker-compose -f docker-compose.production.yml up -d

# View logs
docker-compose logs -f validator-1
docker-compose logs -f validator-2

# Check service status
docker-compose ps

# Stop all services
docker-compose down

# Remove all data (WARNING: destructive)
docker-compose down -v

# Restart specific service
docker-compose restart validator-1

# Scale services
docker-compose up -d --scale validator=5
```

---

## 3️⃣ KUBERNETES ARCHITECTURE

### K8s Manifest Overview

```
deployment/k8s/
├── 01-namespace.yaml              # Create x3-testnet namespace
├── 02-configmaps.yaml             # Chain spec, node config
├── 03-secrets.yaml                # Validator keys
├── 04-pvcs.yaml                   # Persistent volumes
├── 05-validators-statefulset.yaml # Validator nodes (StatefulSet)
├── 06-indexer-deployment.yaml     # Indexer service
└── 07-postgres-statefulset.yaml   # Database
```

### Namespace & ConfigMaps

Create `01-namespace.yaml`:

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: x3-testnet
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: x3-chain-spec
  namespace: x3-testnet
data:
  x3-public-testnet.json: |
    {
      "name": "X3 Public Testnet",
      "id": "x3_public_testnet",
      "chainType": "Live",
      "bootNodes": [
        "/dns4/x3-validator-0.x3-validators/tcp/30333/p2p/12D3KooWNMSBgEWpJQhRv9MEhS3LgTRXt7FEUMhtYA8TaB1tdPsw"
      ],
      "telemetryEndpoints": [],
      "protocol_id": "x3",
      "properties": {
        "ss58Format": 42,
        "tokenDecimals": 12,
        "tokenSymbol": "X3"
      },
      "genesis": { ... }
    }
```

### Secrets for Validator Keys

Create `03-secrets.yaml`:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: validator-keys
  namespace: x3-testnet
type: Opaque
data:
  validator-1-seed: <base64-encoded-seed>
  validator-2-seed: <base64-encoded-seed>
  validator-3-seed: <base64-encoded-seed>
---
apiVersion: v1
kind: Secret
metadata:
  name: postgres-credentials
  namespace: x3-testnet
type: Opaque
stringData:
  username: x3
  password: secure_password
```

### Persistent Volumes

Create `04-pvcs.yaml`:

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: validator-0-pvc
  namespace: x3-testnet
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 100Gi
  storageClassName: fast-ssd
---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: validator-1-pvc
  namespace: x3-testnet
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 100Gi
  storageClassName: fast-ssd
---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: validator-2-pvc
  namespace: x3-testnet
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 100Gi
  storageClassName: fast-ssd
```

### Validator StatefulSet

Create `05-validators-statefulset.yaml`:

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: x3-validator
  namespace: x3-testnet
spec:
  serviceName: x3-validators
  replicas: 3
  selector:
    matchLabels:
      app: x3-validator
  template:
    metadata:
      labels:
        app: x3-validator
    spec:
      serviceAccountName: x3-validator
      
      # Init container for key setup
      initContainers:
      - name: key-setup
        image: x3-blockchain:latest
        command:
          - /app/x3-chain-node
          - key
          - insert
          - --base-path=/data/validator
          - --keystore-path=/data/validator/chains/x3_public_testnet/keystore
          - --suri=$(VALIDATOR_SEED)
        env:
        - name: VALIDATOR_SEED
          valueFrom:
            secretKeyRef:
              name: validator-keys
              key: validator-$(HOSTNAME_SUFFIX)-seed
        volumeMounts:
        - name: validator-storage
          mountPath: /data/validator

      containers:
      - name: x3-node
        image: x3-blockchain:latest
        imagePullPolicy: IfNotPresent
        command:
          - /app/x3-chain-node
          - --chain=/config/x3-public-testnet.json
          - --validator
          - --name=$(HOSTNAME)
          - --base-path=/data/validator
          - --port=30333
          - --rpc-port=9933
          - --rpc-external
          - --rpc-cors=all
          - --prometheus-external
          - --prometheus-port=9615
          - --pruning=archive
        
        env:
        - name: HOSTNAME
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: RUST_LOG
          value: "info,x3_runtime=debug"

        ports:
        - containerPort: 30333
          name: p2p
          protocol: TCP
        - containerPort: 9933
          name: rpc
          protocol: TCP
        - containerPort: 9615
          name: prometheus
          protocol: TCP

        resources:
          requests:
            cpu: "2000m"
            memory: "4Gi"
          limits:
            cpu: "4000m"
            memory: "8Gi"

        livenessProbe:
          httpGet:
            path: /system_nodeInfo
            port: 9933
          initialDelaySeconds: 30
          periodSeconds: 30
          timeoutSeconds: 5
          failureThreshold: 3

        readinessProbe:
          httpGet:
            path: /system_nodeInfo
            port: 9933
          initialDelaySeconds: 10
          periodSeconds: 10
          timeoutSeconds: 5

        volumeMounts:
        - name: validator-storage
          mountPath: /data/validator
        - name: chain-spec
          mountPath: /config

      volumes:
      - name: chain-spec
        configMap:
          name: x3-chain-spec

  volumeClaimTemplates:
  - metadata:
      name: validator-storage
    spec:
      accessModes: [ "ReadWriteOnce" ]
      storageClassName: fast-ssd
      resources:
        requests:
          storage: 100Gi

---
apiVersion: v1
kind: Service
metadata:
  name: x3-validators
  namespace: x3-testnet
spec:
  clusterIP: None
  selector:
    app: x3-validator
  ports:
  - port: 30333
    name: p2p
  - port: 9933
    name: rpc
  - port: 9615
    name: prometheus

---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: x3-validator
  namespace: x3-testnet
```

### K8s Deployment Commands

```bash
# Apply K8s manifests
kubectl apply -f deployment/k8s/01-namespace.yaml
kubectl apply -f deployment/k8s/02-configmaps.yaml
kubectl apply -f deployment/k8s/03-secrets.yaml
kubectl apply -f deployment/k8s/04-pvcs.yaml
kubectl apply -f deployment/k8s/05-validators-statefulset.yaml
kubectl apply -f deployment/k8s/06-indexer-deployment.yaml
kubectl apply -f deployment/k8s/07-postgres-statefulset.yaml

# Check deployment status
kubectl get pods -n x3-testnet
kubectl get statefulset -n x3-testnet
kubectl get service -n x3-testnet

# View validator logs
kubectl logs -n x3-testnet x3-validator-0 -f
kubectl logs -n x3-testnet x3-validator-1 -f

# Get RPC endpoint IP
kubectl get svc -n x3-testnet x3-validators -o jsonpath='{.status.loadBalancer.ingress[0].ip}'
```

---

## 4️⃣ HELM CHART SETUP

### Helm Chart Values

Create `helm/values.yaml`:

```yaml
replicaCount: 3

image:
  repository: myregistry.azurecr.io/x3-blockchain
  tag: latest
  pullPolicy: IfNotPresent

validator:
  resources:
    requests:
      cpu: 2000m
      memory: 4Gi
    limits:
      cpu: 4000m
      memory: 8Gi
  
  storage:
    size: 100Gi
    storageClass: fast-ssd

postgres:
  enabled: true
  resources:
    requests:
      memory: 1Gi
    limits:
      memory: 2Gi
  storage:
    size: 50Gi

prometheus:
  enabled: true
  retention: 30d

grafana:
  enabled: true
  adminPassword: admin

service:
  type: LoadBalancer
  rpc:
    port: 9933
  p2p:
    port: 30333
  prometheus:
    port: 9615
```

### Install Helm Chart

```bash
# Add X3 Helm repo
helm repo add x3 https://charts.x3blockchain.com
helm repo update

# Install X3 testnet
helm install x3-testnet x3/x3-blockchain \
  -n x3-testnet --create-namespace \
  -f helm/values.yaml

# Upgrade deployment
helm upgrade x3-testnet x3/x3-blockchain \
  -n x3-testnet \
  -f helm/values-prod.yaml

# Check Helm release
helm status x3-testnet -n x3-testnet
helm history x3-testnet -n x3-testnet
```

---

## 5️⃣ AUTO-SCALING CONFIGURATION

### Horizontal Pod Autoscaler

Create `hpa.yaml`:

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: x3-validator-hpa
  namespace: x3-testnet
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: StatefulSet
    name: x3-validator
  minReplicas: 3
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
  behavior:
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Percent
        value: 50
        periodSeconds: 60
    scaleUp:
      stabilizationWindowSeconds: 0
      policies:
      - type: Percent
        value: 100
        periodSeconds: 15
```

---

## 6️⃣ FAILOVER & HIGH AVAILABILITY

### Network Policy

Create `network-policy.yaml`:

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: x3-network-policy
  namespace: x3-testnet
spec:
  podSelector:
    matchLabels:
      app: x3-validator
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - podSelector:
        matchLabels:
          app: x3-validator
    ports:
    - protocol: TCP
      port: 30333
    - protocol: TCP
      port: 9933
    - protocol: TCP
      port: 9615
  egress:
  - to:
    - podSelector:
        matchLabels:
          app: x3-validator
    ports:
    - protocol: TCP
      port: 30333
  - to:
    - podSelector:
        matchLabels:
          app: postgres
    ports:
    - protocol: TCP
      port: 5432
  - ports:
    - protocol: TCP
      port: 53
    - protocol: UDP
      port: 53
```

### Pod Disruption Budget

Create `pdb.yaml`:

```yaml
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: x3-validator-pdb
  namespace: x3-testnet
spec:
  minAvailable: 2
  selector:
    matchLabels:
      app: x3-validator
```

---

## 7️⃣ TROUBLESHOOTING GUIDE

### Common Issues & Solutions

**Issue 1: Pods not starting**
```bash
# Check pod events
kubectl describe pod x3-validator-0 -n x3-testnet

# Check logs
kubectl logs x3-validator-0 -n x3-testnet --previous

# Check resource availability
kubectl top nodes
kubectl top pods -n x3-testnet
```

**Issue 2: Consensus not progressing**
```bash
# Check peer connectivity
kubectl exec -it x3-validator-0 -n x3-testnet -- \
  curl -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"system_peers"}' \
  http://localhost:9933

# Check block production
kubectl logs x3-validator-0 -n x3-testnet | grep -i "block"
```

**Issue 3: Storage full**
```bash
# Check PVC usage
kubectl get pvc -n x3-testnet
kubectl exec x3-validator-0 -n x3-testnet -- df -h /data/validator

# Resize PVC (if supported by storage class)
kubectl patch pvc validator-0-pvc -n x3-testnet \
  -p '{"spec":{"resources":{"requests":{"storage":"200Gi"}}}}'
```

---

## ✅ DEPLOYMENT CHECKLIST

- [ ] Docker images built and pushed to registry
- [ ] Docker Compose tested locally
- [ ] K8s namespace created
- [ ] ConfigMaps and Secrets applied
- [ ] PersistentVolumes provisioned
- [ ] StatefulSet deployed
- [ ] Service endpoints verified
- [ ] Health checks passing
- [ ] Prometheus metrics scraping
- [ ] Grafana dashboards displaying
- [ ] Auto-scaling policies active
- [ ] Pod disruption budgets configured
- [ ] Network policies applied
- [ ] Backup procedures tested

---

## 🚀 DEPLOYMENT COMMAND SUMMARY

```bash
# Docker Compose (Single machine)
docker-compose -f docker-compose.production.yml up -d

# Kubernetes (Multi-node cluster)
kubectl apply -f deployment/k8s/
helm install x3-testnet x3/x3-blockchain -n x3-testnet

# Verify deployment
docker-compose ps                          # Docker Compose
kubectl get pods -n x3-testnet             # Kubernetes
```

**Ready to deploy!** 🚀
