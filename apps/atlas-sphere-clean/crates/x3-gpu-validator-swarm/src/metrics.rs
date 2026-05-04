//! Metrics and telemetry for X3 GPU Validator Swarm

use crate::error::SwarmResult;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Swarm-level metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SwarmMetrics {
    /// Total validators
    pub total_validators: u64,
    /// Active validators
    pub active_validators: u64,
    /// Quarantined validators
    pub quarantined_validators: u64,
    /// Total tasks processed
    pub total_tasks: u64,
    /// Successful tasks
    pub successful_tasks: u64,
    /// Failed tasks
    pub failed_tasks: u64,
    /// Total tasks with divergence
    pub divergent_tasks: u64,
    /// CPU fallback count
    pub cpu_fallbacks: u64,
    /// Average task latency (ms)
    pub avg_task_latency_ms: f64,
    /// Tasks per second
    pub tasks_per_second: f64,
}

/// Validator-level metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidatorMetrics {
    /// Validator ID
    pub validator_id: String,
    /// Tasks completed
    pub tasks_completed: u64,
    /// Tasks failed
    pub tasks_failed: u64,
    /// Divergences detected
    pub divergences: u64,
    /// Last task timestamp
    pub last_task_at: Option<i64>,
    /// Average latency (ms)
    pub avg_latency_ms: f64,
    /// Current stake
    pub stake: u64,
}

/// Metrics collector
pub struct MetricsCollector {
    /// Swarm metrics
    swarm: RwLock<SwarmMetrics>,
    /// Validator metrics
    validators: RwLock<HashMap<String, ValidatorMetrics>>,
    /// Counters
    counters: RwLock<HashMap<String, AtomicU64>>,
    /// Gauge values
    gauges: RwLock<HashMap<String, f64>>,
    /// Latency tracking
    latencies: RwLock<Vec<u64>>,
    /// Start time
    start_time: Instant,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            swarm: RwLock::new(SwarmMetrics::default()),
            validators: RwLock::new(HashMap::new()),
            counters: RwLock::new(HashMap::new()),
            gauges: RwLock::new(HashMap::new()),
            latencies: RwLock::new(Vec::new()),
            start_time: Instant::now(),
        }
    }

    /// Increment a counter
    pub fn increment(&self, name: &str, value: u64) {
        let mut counters = self.counters.write();
        let counter = counters
            .entry(name.to_string())
            .or_insert_with(|| AtomicU64::new(0));
        counter.fetch_add(value, Ordering::SeqCst);
    }

    /// Set a gauge value
    pub fn set_gauge(&self, name: &str, value: f64) {
        let mut gauges = self.gauges.write();
        gauges.insert(name.to_string(), value);
    }

    /// Get a counter value
    pub fn get_counter(&self, name: &str) -> u64 {
        let counters = self.counters.read();
        counters
            .get(name)
            .map(|c| c.load(Ordering::SeqCst))
            .unwrap_or(0)
    }

    /// Get a gauge value
    pub fn get_gauge(&self, name: &str) -> f64 {
        let gauges = self.gauges.read();
        *gauges.get(name).unwrap_or(&0.0)
    }

    /// Record task completion
    pub fn record_task(&self, validator_id: &str, latency_ms: u64, success: bool, divergent: bool) {
        // Update swarm metrics
        {
            let mut swarm = self.swarm.write();
            swarm.total_tasks += 1;
            if success {
                swarm.successful_tasks += 1;
            } else {
                swarm.failed_tasks += 1;
            }
            if divergent {
                swarm.divergent_tasks += 1;
            }
        }

        // Update validator metrics
        {
            let mut validators = self.validators.write();
            let metrics = validators
                .entry(validator_id.to_string())
                .or_insert_with(|| ValidatorMetrics {
                    validator_id: validator_id.to_string(),
                    ..Default::default()
                });
            metrics.tasks_completed += 1;
            if !success {
                metrics.tasks_failed += 1;
            }
            if divergent {
                metrics.divergences += 1;
            }
            metrics.last_task_at = Some(chrono::Utc::now().timestamp());

            // Update average latency
            let total_latency = metrics.avg_latency_ms * (metrics.tasks_completed - 1) as f64;
            metrics.avg_latency_ms =
                (total_latency + latency_ms as f64) / metrics.tasks_completed as f64;
        }

        // Record latency for swarm average
        {
            let mut latencies = self.latencies.write();
            latencies.push(latency_ms);
            if latencies.len() > 10000 {
                latencies.drain(0..1000);
            }
        }

        // Increment counters
        self.increment("tasks_total", 1);
        if success {
            self.increment("tasks_success", 1);
        } else {
            self.increment("tasks_failed", 1);
        }
        if divergent {
            self.increment("tasks_divergent", 1);
        }
    }

    /// Record CPU fallback
    pub fn record_cpu_fallback(&self) {
        self.increment("cpu_fallbacks", 1);
        let mut swarm = self.swarm.write();
        swarm.cpu_fallbacks += 1;
    }

    /// Get swarm metrics
    pub fn get_swarm_metrics(&self) -> SwarmMetrics {
        let mut swarm = self.swarm.write();

        // Calculate average latency
        let latencies = self.latencies.read();
        swarm.avg_task_latency_ms = if latencies.is_empty() {
            0.0
        } else {
            latencies.iter().sum::<u64>() as f64 / latencies.len() as f64
        };

        // Calculate TPS
        let elapsed = self.start_time.elapsed().as_secs_f64();
        swarm.tasks_per_second = if elapsed > 0.0 {
            swarm.total_tasks as f64 / elapsed
        } else {
            0.0
        };

        swarm.clone()
    }

    /// Get validator metrics
    pub fn get_validator_metrics(&self, validator_id: &str) -> Option<ValidatorMetrics> {
        let validators = self.validators.read();
        validators.get(validator_id).cloned()
    }

    /// Get all validator metrics
    pub fn get_all_validator_metrics(&self) -> Vec<ValidatorMetrics> {
        let validators = self.validators.read();
        validators.values().cloned().collect()
    }

    /// Update validator count
    pub fn update_validator_count(&self, total: u64, active: u64, quarantined: u64) {
        let mut swarm = self.swarm.write();
        swarm.total_validators = total;
        swarm.active_validators = active;
        swarm.quarantined_validators = quarantined;
        self.set_gauge("validators_total", total as f64);
        self.set_gauge("validators_active", active as f64);
        self.set_gauge("validators_quarantined", quarantined as f64);
    }

    /// Reset metrics
    pub fn reset(&self) {
        *self.swarm.write() = SwarmMetrics::default();
        self.validators.write().clear();
        self.counters.write().clear();
        self.gauges.write().clear();
        self.latencies.write().clear();
    }

    /// Export all metrics as JSON
    pub fn export_json(&self) -> SwarmResult<String> {
        #[derive(Serialize)]
        struct ExportData {
            swarm: SwarmMetrics,
            validators: Vec<ValidatorMetrics>,
            counters: HashMap<String, u64>,
            gauges: HashMap<String, f64>,
            uptime_seconds: f64,
        }

        let data = ExportData {
            swarm: self.get_swarm_metrics(),
            validators: self.get_all_validator_metrics(),
            counters: self
                .counters
                .read()
                .iter()
                .map(|(k, v)| (k.clone(), v.load(Ordering::SeqCst)))
                .collect(),
            gauges: self.gauges.read().clone(),
            uptime_seconds: self.start_time.elapsed().as_secs_f64(),
        };

        serde_json::to_string_pretty(&data).map_err(|e| e.into())
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    /// Service name
    pub service: String,
    /// Health status
    pub status: HealthStatus,
    /// Message
    pub message: Option<String>,
    /// Timestamp
    pub timestamp: i64,
    /// Details
    pub details: HashMap<String, String>,
}

/// Health status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Healthy
    Healthy,
    /// Degraded
    Degraded,
    /// Unhealthy
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

/// Validator health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorHealth {
    pub validator_id: String,
    pub status: HealthStatus,
    pub last_heartbeat: Option<i64>,
    pub tasks_recent: u64,
    pub error_rate: f64,
    pub divergence_rate: f64,
}
