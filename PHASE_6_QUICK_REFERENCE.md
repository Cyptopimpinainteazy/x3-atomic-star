# Phase 6 Quick Reference - Kubernetes Deployment

## 📋 Pre-Deployment Checklist

- [ ] Kubernetes cluster available (`kubectl cluster-info`)
- [ ] Docker images built:
  - [ ] `docker.io/x3-chain/x3-chain-node:latest` (52MB)
  - [ ] `docker.io/x3-chain/x3-indexer:latest` (8.7MB)
- [ ] Container registry accessible
- [ ] Storage class available for PersistentVolumes
- [ ] kubectl configured and authenticated
- [ ] Cluster has minimum 12 CPU cores and 24GB RAM (3 validators @ 4CPU/8GB each)

## 🚀 Quick Deployment Commands

### Build Docker Images (Phase 6a)

```bash
# Build validator image
cd /home/lojak/Desktop/X3_ATOMIC_STAR
docker build -f Dockerfile.validator -t x3-chain/x3-chain-node:latest .

# Build indexer image
docker build -f Dockerfile.indexer -t x3-chain/x3-indexer:latest .

# Tag for registry (if using external registry)
docker tag x3-chain/x3-chain-node:latest docker.io/x3-chain/x3-chain-node:latest
docker tag x3-chain/x3-indexer:latest docker.io/x3-chain/x3-indexer:latest

# Push to registry (optional)
docker push docker.io/x3-chain/x3-chain-node:latest
docker push docker.io/x3-chain/x3-indexer:latest
```

### Deploy to Kubernetes (Phase 6b/6c)

```bash
# Automated deployment (uses all manifests in order)
chmod +x /home/lojak/Desktop/X3_ATOMIC_STAR/scripts/k8s-deploy.sh
./scripts/k8s-deploy.sh apply

# Manual step-by-step deployment
cd /home/lojak/Desktop/X3_ATOMIC_STAR/k8s

# 1. Create namespace and basic resources
kubectl apply -f 01-namespace.yaml
kubectl apply -f 02-configmaps.yaml
kubectl apply -f 03-secrets.yaml
kubectl apply -f 04-pvcs.yaml

# 2. Deploy database
kubectl apply -f 07-postgres-statefulset.yaml

# 3. Deploy validators
kubectl apply -f 05-validators-statefulset.yaml

# 4. Deploy indexer
kubectl apply -f 06-indexer-deployment.yaml
```

## 📊 Monitoring & Verification

### Check Pod Status

```bash
# Watch pods starting up
kubectl get pods -n x3-chain -w

# Check specific pod status
kubectl describe pod x3-validator-0 -n x3-chain

# View pod logs
kubectl logs -f x3-validator-0 -n x3-chain
kubectl logs -f x3-indexer-0 -n x3-chain
```

### Verify Validator Consensus

```bash
# Check validator health
kubectl exec -it x3-validator-0 -n x3-chain -- \
  curl -s http://localhost:9933 -X POST \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"system_health","params":[],"id":1}' | jq

# Check connected peers
kubectl exec -it x3-validator-0 -n x3-chain -- \
  curl -s http://localhost:9933 -X POST \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"system_peers","params":[],"id":1}' | jq

# Get block number
kubectl exec -it x3-validator-0 -n x3-chain -- \
  curl -s http://localhost:9933 -X POST \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"chain_getHeader","params":[],"id":1}' | jq
```

### Verify Indexer

```bash
# Test GraphQL endpoint (internal)
kubectl run -it --rm debug --image=curlimages/curl -n x3-chain --restart=Never -- \
  curl -X POST http://x3-indexer-internal:4000/graphql \
  -H "Content-Type: application/json" \
  -d '{"query":"{ __typename }"}'

# Get external IP of Indexer LoadBalancer
kubectl get svc x3-indexer -n x3-chain

# Test GraphQL endpoint (external, once LoadBalancer has IP)
INDEXER_IP=$(kubectl get svc x3-indexer -n x3-chain -o jsonpath='{.status.loadBalancer.ingress[0].ip}')
curl -X POST http://$INDEXER_IP:4000/graphql \
  -H "Content-Type: application/json" \
  -d '{"query":"{ __typename }"}'
```

## 🔧 Common Operations

### Scale Validators

```bash
# Scale to 5 validators
kubectl scale statefulset x3-validator --replicas=5 -n x3-chain

# Scale back to 3
kubectl scale statefulset x3-validator --replicas=3 -n x3-chain
```

### Scale Indexer

```bash
# Scale to 3 replicas
kubectl scale deployment x3-indexer --replicas=3 -n x3-chain

# Scale to 1 (single instance)
kubectl scale deployment x3-indexer --replicas=1 -n x3-chain
```

### Update Images

```bash
# Update validator image
kubectl set image statefulset/x3-validator \
  validator=x3-chain/x3-chain-node:v1.1.0 -n x3-chain

# Update indexer image
kubectl set image deployment/x3-indexer \
  indexer=x3-chain/x3-indexer:v1.1.0 -n x3-chain

# Rollback if needed
kubectl rollout undo deployment/x3-indexer -n x3-chain
```

### Port Forwarding (Local Testing)

```bash
# Forward validator RPC to localhost
kubectl port-forward -n x3-chain svc/x3-validator-rpc 9933:9933

# Forward indexer GraphQL to localhost
kubectl port-forward -n x3-chain svc/x3-indexer 4000:4000

# Now access locally:
# RPC: http://localhost:9933
# GraphQL: http://localhost:4000/graphql
```

## 🗑️ Cleanup

```bash
# Delete all X3 Chain resources
./scripts/k8s-deploy.sh delete

# Or manually delete in reverse order
kubectl delete -f k8s/06-indexer-deployment.yaml -n x3-chain
kubectl delete -f k8s/05-validators-statefulset.yaml -n x3-chain
kubectl delete -f k8s/07-postgres-statefulset.yaml -n x3-chain
kubectl delete -f k8s/04-pvcs.yaml -n x3-chain
kubectl delete -f k8s/03-secrets.yaml -n x3-chain
kubectl delete -f k8s/02-configmaps.yaml -n x3-chain
kubectl delete namespace x3-chain
```

## 📝 Notes

- **Persistence:** Validators use PersistentVolumes to maintain state across restarts
- **Database:** PostgreSQL is deployed in the cluster; for production, consider managed database services
- **LoadBalancer:** If using LoadBalancer services, the external IP may take a few minutes to assign
- **Network:** Validators communicate internally via service DNS (x3-validators)
- **Security:** Update secrets before production deployment

## 🎯 Success Criteria

After deployment:
1. ✅ All 3 validators reach "Ready" state
2. ✅ Validators report 2+ connected peers
3. ✅ Block height advancing on all validators
4. ✅ PostgreSQL database healthy
5. ✅ Indexer responding to GraphQL queries
6. ✅ External access working via LoadBalancer IPs
