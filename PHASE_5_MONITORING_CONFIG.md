# 📊 PRODUCTION MONITORING CONFIGURATION GUIDE

**Phase 5 Monitoring Infrastructure Setup**  
**Date:** April 26, 2026  
**Status:** Ready for Deployment  

---

## 🎯 MONITORING ARCHITECTURE

```
┌─────────────────────────────────────────────────────────────────┐
│                  PUBLIC TESTNET ARCHITECTURE                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │  Validator-1 │  │  Validator-2 │  │  Validator-3 │          │
│  │  (Metrics)   │  │  (Metrics)   │  │  (Metrics)   │          │
│  └────┬─────────┘  └────┬─────────┘  └────┬─────────┘          │
│       │                 │                 │                    │
│       └─────────────────┼─────────────────┘                    │
│                         │ (Scrape /metrics)                    │
│                    ┌────▼──────────┐                           │
│                    │  PROMETHEUS   │◄───── Queries             │
│                    │  (9090)       │                           │
│                    └────┬──────────┘                           │
│                         │ (Queries)                            │
│                    ┌────▼──────────┐                           │
│                    │   GRAFANA     │◄───── Dashboards          │
│                    │   (3000)      │                           │
│                    └───────────────┘                           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 1️⃣ PROMETHEUS CONFIGURATION

### Setup Prometheus Server

```bash
# Create directory structure
mkdir -p /opt/prometheus/{config,data,rules}

# Download Prometheus
cd /tmp
wget https://github.com/prometheus/prometheus/releases/download/v2.51.0/prometheus-2.51.0.linux-amd64.tar.gz
tar xzf prometheus-2.51.0.linux-amd64.tar.gz
sudo cp prometheus-2.51.0.linux-amd64/prometheus /usr/local/bin/
sudo cp prometheus-2.51.0.linux-amd64/promtool /usr/local/bin/
```

### Prometheus Configuration File

Create `/opt/prometheus/config/prometheus.yml`:

```yaml
# Global configuration
global:
  scrape_interval: 15s
  evaluation_interval: 15s
  external_labels:
    cluster: 'x3-public-testnet'
    environment: 'production'

# Alerting configuration
alerting:
  alertmanagers:
    - static_configs:
        - targets:
            - localhost:9093

# Load rules
rule_files:
  - '/opt/prometheus/rules/x3-settlement-rules.yml'
  - '/opt/prometheus/rules/x3-gpu-rules.yml'
  - '/opt/prometheus/rules/x3-consensus-rules.yml'
  - '/opt/prometheus/rules/x3-network-rules.yml'

# Scrape configurations
scrape_configs:
  # Validator 1 Prometheus Metrics
  - job_name: 'x3-validator-1'
    static_configs:
      - targets: ['localhost:9615']
        labels:
          validator: 'Validator-1'
          instance: 'validator-1'
    scrape_interval: 15s

  # Validator 2 Prometheus Metrics
  - job_name: 'x3-validator-2'
    static_configs:
      - targets: ['localhost:9616']
        labels:
          validator: 'Validator-2'
          instance: 'validator-2'
    scrape_interval: 15s

  # Validator 3 Prometheus Metrics
  - job_name: 'x3-validator-3'
    static_configs:
      - targets: ['localhost:9617']
        labels:
          validator: 'Validator-3'
          instance: 'validator-3'
    scrape_interval: 15s

  # Node Exporter (System Metrics)
  - job_name: 'node-exporter'
    static_configs:
      - targets: ['localhost:9100']

  # Prometheus Self-Monitoring
  - job_name: 'prometheus'
    static_configs:
      - targets: ['localhost:9090']
```

### Start Prometheus

```bash
sudo /usr/local/bin/prometheus \
  --config.file=/opt/prometheus/config/prometheus.yml \
  --storage.tsdb.path=/opt/prometheus/data \
  --storage.tsdb.retention.time=30d \
  --web.console.libraries=/usr/local/share/prometheus/console_libraries \
  --web.console.templates=/usr/local/share/prometheus/consoles \
  --web.listen-address=0.0.0.0:9090 &

# Verify Prometheus running
curl http://localhost:9090/-/healthy
```

---

## 2️⃣ ALERT RULES CONFIGURATION

### Settlement Engine Alert Rules

Create `/opt/prometheus/rules/x3-settlement-rules.yml`:

```yaml
groups:
  - name: x3_settlement_alerts
    interval: 30s
    rules:
      # Settlement Timeout Enforcement
      - alert: SettlementTimeoutNotEnforced
        expr: rate(settlement_timeout_enforced_total[5m]) == 0
        for: 5m
        labels:
          severity: warning
          component: settlement
        annotations:
          summary: "Settlement timeout enforcement stalled"
          description: "Settlement timeout enforcement has not processed intents in 5 minutes"

      # Settlement Auto-Refund Not Working
      - alert: SettlementAutoRefundFailing
        expr: rate(settlement_refunded_total[10m]) < 0.5
        for: 10m
        labels:
          severity: critical
          component: settlement
        annotations:
          summary: "Settlement auto-refund rate abnormally low"
          description: "Expected at least 0.5 refunds/min but got {{ $value }}"

      # High Pending Settlement Count
      - alert: HighPendingSettlements
        expr: settlement_pending_count > 1000
        for: 5m
        labels:
          severity: warning
          component: settlement
        annotations:
          summary: "{{ $value }} pending settlements"
          description: "Settlement engine has {{ $value }} pending intents (threshold: 1000)"

      # Settlement Deadline Backlog
      - alert: SettlementDeadlineBacklog
        expr: (settlement_pending_count / rate(settlement_deadline_processed[5m])) > 60
        for: 5m
        labels:
          severity: critical
          component: settlement
        annotations:
          summary: "Settlement deadline processing backlog > 60 minutes"
          description: "Current backlog would take {{ $value }} minutes to process"
```

### GPU Health Alert Rules

Create `/opt/prometheus/rules/x3-gpu-rules.yml`:

```yaml
groups:
  - name: x3_gpu_alerts
    interval: 30s
    rules:
      # GPU Consecutive Failures Threshold
      - alert: GPUFailureThresholdReached
        expr: gpu_failures_consecutive >= 3
        for: 1m
        labels:
          severity: critical
          component: gpu
        annotations:
          summary: "GPU sidecar {{ $labels.instance }} restart triggered"
          description: "{{ $value }} consecutive GPU failures detected"

      # GPU Health Checks Stalled
      - alert: GPUHealthCheckStalled
        expr: rate(gpu_health_checks_total[5m]) == 0
        for: 5m
        labels:
          severity: warning
          component: gpu
        annotations:
          summary: "GPU health checks not executing"
          description: "GPU health monitoring appears to be stuck on {{ $labels.instance }}"

      # High GPU Restart Rate
      - alert: HighGPURestartRate
        expr: rate(gpu_restarts_triggered[1h]) > 1
        for: 5m
        labels:
          severity: warning
          component: gpu
        annotations:
          summary: "GPU restart rate abnormally high"
          description: "{{ $value }} restarts/hour on {{ $labels.instance }} (healthy: < 1/hour)"

      # GPU Uptime Degrading
      - alert: GPUUptimeDegrading
        expr: gpu_uptime_blocks < 86400
        for: 10m
        labels:
          severity: warning
          component: gpu
        annotations:
          summary: "GPU uptime less than 24 hours"
          description: "{{ $labels.instance }} uptime: {{ $value }} blocks (< 86400)"
```

### Consensus Alert Rules

Create `/opt/prometheus/rules/x3-consensus-rules.yml`:

```yaml
groups:
  - name: x3_consensus_alerts
    interval: 30s
    rules:
      # Validator Consensus Fork Detected
      - alert: ConsensusForksDetected
        expr: rate(consensus_forks_detected[5m]) > 0
        labels:
          severity: critical
          component: consensus
        annotations:
          summary: "CONSENSUS FORK DETECTED!"
          description: "{{ $value }} forks detected in last 5 minutes on {{ $labels.instance }}"

      # Block Finalization Latency High
      - alert: HighBlockFinalizationLatency
        expr: consensus_finalization_latency_ms > 120000
        for: 5m
        labels:
          severity: warning
          component: consensus
        annotations:
          summary: "Block finalization latency > 2 minutes"
          description: "{{ $labels.instance }} finalization latency: {{ $value }}ms (target: < 120s)"

      # Validator Block Production Stopped
      - alert: ValidatorBlockProductionStopped
        expr: rate(validator_block_produced_total[5m]) == 0
        for: 2m
        labels:
          severity: critical
          component: consensus
        annotations:
          summary: "Block production stopped"
          description: "{{ $labels.validator }} has not produced blocks in 2 minutes"

      # Validator Block Attestation Failing
      - alert: ValidatorAttestationFailing
        expr: (rate(validator_block_attested_total[5m]) / rate(validator_block_produced_total[5m])) < 0.8
        for: 5m
        labels:
          severity: warning
          component: consensus
        annotations:
          summary: "Validator attestation rate below 80%"
          description: "{{ $labels.validator }} attestation rate: {{ $value | humanizePercentage }}"

      # Validator Peer Connectivity Lost
      - alert: ValidatorPeerConnectivityLost
        expr: validator_connected_peers == 0
        for: 1m
        labels:
          severity: critical
          component: consensus
        annotations:
          summary: "Validator disconnected from all peers"
          description: "{{ $labels.instance }} has 0 connected peers"
```

### Network Alert Rules

Create `/opt/prometheus/rules/x3-network-rules.yml`:

```yaml
groups:
  - name: x3_network_alerts
    interval: 30s
    rules:
      # RPC Endpoint Down
      - alert: RPCEndpointDown
        expr: up{job="x3-validator-1"} == 0
        for: 1m
        labels:
          severity: critical
          component: rpc
        annotations:
          summary: "RPC endpoint down"
          description: "{{ $labels.instance }} RPC endpoint is unreachable"

      # High RPC Request Latency
      - alert: HighRPCLatency
        expr: rpc_request_latency_ms > 5000
        for: 5m
        labels:
          severity: warning
          component: rpc
        annotations:
          summary: "RPC request latency > 5 seconds"
          description: "{{ $labels.endpoint }} latency: {{ $value }}ms"

      # Validator Node Sync Lagging
      - alert: ValidatorSyncLagging
        expr: (max(chain_block_number) - chain_block_number) > 10
        for: 5m
        labels:
          severity: warning
          component: network
        annotations:
          summary: "Validator block sync lagging behind network"
          description: "{{ $labels.instance }} is {{ $value }} blocks behind the network tip"
```

---

## 3️⃣ GRAFANA DASHBOARDS

### Dashboard 1: Settlement System

```json
{
  "title": "Settlement Engine - Production Monitoring",
  "panels": [
    {
      "title": "Settlement Timeout Enforcements (per minute)",
      "targets": [
        {
          "expr": "rate(settlement_timeout_enforced_total[1m])"
        }
      ]
    },
    {
      "title": "Auto-Refunds Processed (cumulative)",
      "targets": [
        {
          "expr": "settlement_refunded_total"
        }
      ]
    },
    {
      "title": "Pending Settlement Intents",
      "targets": [
        {
          "expr": "settlement_pending_count"
        }
      ]
    },
    {
      "title": "Deadline Processing Rate (blocks/min)",
      "targets": [
        {
          "expr": "rate(settlement_deadline_processed[1m])"
        }
      ]
    }
  ]
}
```

### Dashboard 2: GPU Health

```json
{
  "title": "GPU Sidecar Health - Production Monitoring",
  "panels": [
    {
      "title": "Health Checks Executed (per minute)",
      "targets": [
        {
          "expr": "rate(gpu_health_checks_total[1m])"
        }
      ]
    },
    {
      "title": "Consecutive Failures",
      "targets": [
        {
          "expr": "gpu_failures_consecutive",
          "legendFormat": "{{ instance }}"
        }
      ]
    },
    {
      "title": "Total Restarts Triggered",
      "targets": [
        {
          "expr": "gpu_restarts_triggered",
          "legendFormat": "{{ instance }}"
        }
      ]
    },
    {
      "title": "Uptime (blocks)",
      "targets": [
        {
          "expr": "gpu_uptime_blocks",
          "legendFormat": "{{ instance }}"
        }
      ]
    }
  ]
}
```

### Dashboard 3: Consensus & Finalization

```json
{
  "title": "Consensus & Finalization - Production Monitoring",
  "panels": [
    {
      "title": "Consensus Rounds Completed",
      "targets": [
        {
          "expr": "rate(consensus_rounds_completed[1m])"
        }
      ]
    },
    {
      "title": "Forks Detected",
      "targets": [
        {
          "expr": "rate(consensus_forks_detected[5m])"
        }
      ]
    },
    {
      "title": "Block Finalization Latency (ms)",
      "targets": [
        {
          "expr": "consensus_finalization_latency_ms",
          "legendFormat": "{{ instance }}"
        }
      ]
    },
    {
      "title": "Validator Block Production Rate",
      "targets": [
        {
          "expr": "rate(validator_block_produced_total[1m])",
          "legendFormat": "{{ validator }}"
        }
      ]
    }
  ]
}
```

---

## 4️⃣ ALERTMANAGER CONFIGURATION

Create `/etc/alertmanager/config.yml`:

```yaml
global:
  resolve_timeout: 5m
  slack_api_url: 'https://hooks.slack.com/services/YOUR/WEBHOOK/URL'

route:
  receiver: 'critical-alerts'
  group_by: ['alertname', 'cluster', 'service']
  group_wait: 10s
  group_interval: 10s
  repeat_interval: 12h

  routes:
    - match:
        severity: critical
      receiver: 'critical-alerts'
      repeat_interval: 5m

    - match:
        severity: warning
      receiver: 'warning-alerts'
      repeat_interval: 1h

receivers:
  - name: 'critical-alerts'
    slack_configs:
      - channel: '#x3-critical-alerts'
        title: '🚨 CRITICAL ALERT'
        text: '{{ .GroupLabels.alertname }}: {{ .CommonAnnotations.description }}'

  - name: 'warning-alerts'
    slack_configs:
      - channel: '#x3-warnings'
        title: '⚠️ WARNING'
        text: '{{ .GroupLabels.alertname }}: {{ .CommonAnnotations.description }}'
```

---

## 5️⃣ LOG AGGREGATION SETUP

### Setup ELK Stack (Optional but Recommended)

```bash
# Install Elasticsearch
docker run -d --name elasticsearch \
  -e "discovery.type=single-node" \
  -e "xpack.security.enabled=false" \
  -p 9200:9200 \
  docker.elastic.co/elasticsearch/elasticsearch:8.0.0

# Install Kibana
docker run -d --name kibana \
  -e "ELASTICSEARCH_HOSTS=http://elasticsearch:9200" \
  -p 5601:5601 \
  docker.elastic.co/kibana/kibana:8.0.0

# Install Filebeat for log shipping
wget https://artifacts.elastic.co/downloads/beats/filebeat/filebeat-8.0.0-linux-x86_64.tar.gz
tar xzf filebeat-8.0.0-linux-x86_64.tar.gz
```

### Filebeat Configuration

Create `filebeat.yml`:

```yaml
filebeat.inputs:
  - type: log
    enabled: true
    paths:
      - /var/log/x3-validator*.log
    fields:
      component: validator

  - type: log
    enabled: true
    paths:
      - /var/log/x3-gpu-monitor.log
    fields:
      component: gpu

  - type: log
    enabled: true
    paths:
      - /var/log/x3-consensus.log
    fields:
      component: consensus

output.elasticsearch:
  hosts: ["localhost:9200"]
  index: "x3-logs-%{+yyyy.MM.dd}"

logging.level: info
logging.to_files: true
logging.files:
  path: /var/log/filebeat
```

---

## ✅ VERIFICATION CHECKLIST

After deploying monitoring infrastructure:

- [ ] Prometheus scraping all 3 validators
- [ ] All metrics visible in Prometheus UI
- [ ] Grafana connected to Prometheus data source
- [ ] Settlement dashboard showing live data
- [ ] GPU health dashboard showing live data
- [ ] Consensus dashboard showing live data
- [ ] Alert rules loading without errors
- [ ] Test alert: `ALERTS_TOTAL > 0`
- [ ] Alertmanager webhook receiving notifications
- [ ] Log aggregation collecting validator logs
- [ ] Kibana indexing X3 logs

---

## 🚀 DEPLOYMENT COMMAND

```bash
# Start full monitoring stack
bash deployment/monitoring/deploy-monitoring.sh

# Or manually:
docker-compose -f deployment/docker-compose.monitoring.yml up -d
```

---

**Monitoring infrastructure ready for Phase 5 production deployment!** 📊
