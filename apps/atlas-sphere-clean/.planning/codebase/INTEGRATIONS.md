# X3 Chain Integrations

**Document:** External Services & Integration Points  
**Date:** 2026-03-15  
**Scope:** Datastores, monitoring, networking, and external APIs

---

## Datastores

### PostgreSQL
Used by gateway/indexer services and analytics.
- `crates/x3-gateway/src/config.rs` (default DB URL)
- `crates/x3-indexer/indexer.toml` (indexer DB URL)
- `deployment/docker/docker-compose.production.yml` (postgres service)
- `deployment/kubernetes/production-deployment.yaml` (postgres deployment)

---

## Monitoring & Telemetry

### Prometheus + Grafana + Loki
- `docker-compose.yml` (prometheus + grafana)
- `deployment/monitoring/docker-compose.yml`
- `deployment/docker/config/prometheus.yml`
- `deployment/docker/config/grafana-datasources.yml`

### Telemetry Endpoint
- `CONFIG.md` references `wss://telemetry.x3-chain.io/submit`
- `DEVELOPMENT.md` shows telemetry configurations

---

## Networking & RPC

### Substrate JSON-RPC / WS
- `node/src/rpc.rs` and `node/src/rpc_frontier.rs`
- CLI flags and default ports in `CONFIG.md` and `CLI_FLAGS.md`

---

## Storage & Content

### IPFS
IPFS tooling and UI integrations are present:
- `START_IPFS_NODE.sh`
- `apps/x3-desktop/src/components/ipfsStorage/IpfsStoragePanel.test.tsx`

---

## Cloud / Tunnel / Infra

### Cloudflare Tunnel
Cloudflare tunnel stubs for connectivity:
- `infra/cloudflare-tunnel/placeholder/package.json`
- `infra-structure/services/cloudflare-tunnel/placeholder/package.json`

### Kubernetes & Docker
Multi-environment deployment definitions:
- `k8s-deployment.yaml`
- `deployment/kubernetes/production-deployment.yaml`
- `deployment/docker/docker-compose.production.yml`

---

## EVM / Solidity Tooling

- **Hardhat:** `contracts/core/package.json`, `botchain-tri-vm-genesis/hardhat/package.json`
- **Foundry references:** `apps/x3-desktop/src/components/panels/explorer/DevDocsPanel.tsx`

---

## API Specs

- OpenAPI spec: `packages/blockchain-connector/docs/openapi.yaml`

