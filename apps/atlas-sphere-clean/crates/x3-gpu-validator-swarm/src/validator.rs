//! Validator module for X3 GPU Validator Swarm

use crate::config::SwarmConfig;
use crate::crypto::HashAlgorithm;
use crate::deterministic::{
    DeterministicEngine, DeterministicTask, ExecutionMode, ExecutionResult,
};
use crate::error::SwarmResult;
use crate::health::{HealthMonitor, ValidatorHealthTracker};
use crate::metrics::MetricsCollector;
use crate::quarantine::QuarantineManager;
use crate::telemetry::TelemetrySink;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Validator state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidatorState {
    /// Starting up
    Starting,
    /// Running normally
    Running,
    /// Running in degraded mode (CPU fallback)
    Degraded,
    /// Quarantined
    Quarantined,
    /// Stopped
    Stopped,
}

/// Validator event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorEvent {
    /// Event type
    pub event_type: String,
    /// Validator ID
    pub validator_id: String,
    /// Timestamp
    pub timestamp: i64,
    /// Data
    pub data: serde_json::Value,
}

/// X3 GPU Validator
pub struct Validator {
    /// Validator ID
    validator_id: String,
    /// Configuration
    config: SwarmConfig,
    /// State
    state: RwLock<ValidatorState>,
    /// Deterministic engine
    engine: DeterministicEngine,
    /// Metrics collector
    metrics: Arc<MetricsCollector>,
    /// Quarantine manager
    quarantine: Arc<QuarantineManager>,
    /// Health monitor
    health: HealthMonitor,
    /// Telemetry sink
    telemetry: Arc<TelemetrySink>,
    /// Health tracker
    health_tracker: RwLock<ValidatorHealthTracker>,
    /// Current mode
    current_mode: RwLock<ExecutionMode>,
    /// Start time
    start_time: Instant,
}

impl Validator {
    /// Create a new validator
    pub fn new(config: SwarmConfig, validator_id: String) -> Self {
        let metrics = Arc::new(MetricsCollector::new());
        let quarantine = Arc::new(QuarantineManager::new(
            config.quarantine.max_divergence_count,
            config.quarantine.quarantine_duration_secs,
            config.quarantine.auto_fallback_cpu,
        ));
        let telemetry = Arc::new(TelemetrySink::new(
            config.telemetry.clone(),
            validator_id.clone(),
        ));

        Self {
            validator_id,
            config,
            state: RwLock::new(ValidatorState::Starting),
            engine: DeterministicEngine::new(),
            metrics,
            quarantine,
            health: HealthMonitor::default(),
            telemetry,
            health_tracker: RwLock::new(ValidatorHealthTracker::new(String::new())),
            current_mode: RwLock::new(ExecutionMode::GpuWithCpuVerification),
            start_time: Instant::now(),
        }
    }

    /// Initialize the validator
    pub fn initialize(&self) -> SwarmResult<()> {
        // Configure engine
        self.engine.set_mode(ExecutionMode::GpuWithCpuVerification);
        self.engine
            .set_cpu_verification(self.config.verification.cpu_verification_enabled);
        self.engine
            .set_replay_mode(self.config.verification.replay_mode_enabled);
        self.engine.set_hash_algorithm(HashAlgorithm::Keccak256);

        // Register health checks
        self.health
            .register("engine".to_string(), || crate::metrics::HealthCheck {
                service: "engine".to_string(),
                status: crate::metrics::HealthStatus::Healthy,
                message: Some("Engine operational".to_string()),
                timestamp: chrono::Utc::now().timestamp(),
                details: HashMap::new(),
            });

        *self.state.write() = ValidatorState::Running;

        Ok(())
    }

    /// Process a task
    pub fn process_task(&self, task: DeterministicTask) -> ExecutionResult {
        // Check if quarantined
        if self.quarantine.is_quarantined(&self.validator_id) {
            return ExecutionResult::error(task.task_id, "Validator is quarantined".to_string());
        }

        // Execute task
        let task_id = task.task_id.clone();
        let start = Instant::now();
        let result = self.engine.execute(task.clone());
        let latency_ms = start.elapsed().as_millis() as u64;

        // Record metrics
        let success = result.verification == crate::crypto::VerificationResult::Valid;
        let divergent = result.divergence_detected;

        self.metrics
            .record_task(&self.validator_id, latency_ms, success, divergent);

        // Update health tracker
        {
            let mut tracker = self.health_tracker.write();
            tracker.record_task(success);
        }

        // Handle divergence
        if divergent {
            // Record divergence
            let mut record = crate::quarantine::DivergenceRecord::new(
                self.validator_id.clone(),
                task_id.clone(),
                result.outputs.iter().flat_map(|h| h.0.to_vec()).collect(),
                vec![], // CPU output would be here in real impl
            );
            record.add_details(format!("Execution mode: {:?}", result.execution_mode));
            self.quarantine.record_divergence(record);

            // Quarantine if too many divergences
            if self.quarantine.should_auto_fallback() {
                // Auto fallback to CPU
                *self.current_mode.write() = ExecutionMode::CpuFallback;
                self.engine.set_mode(ExecutionMode::CpuFallback);
                self.metrics.record_cpu_fallback();

                // Notify telemetry
                self.telemetry.record_divergence(
                    self.validator_id.clone(),
                    &task_id,
                    "Auto-fallback to CPU enabled",
                );
            }
        }

        // Record telemetry
        self.telemetry
            .record_task(self.validator_id.clone(), &task_id, latency_ms, success);

        result
    }

    /// Get current state
    pub fn state(&self) -> ValidatorState {
        *self.state.read()
    }

    /// Get validator ID
    pub fn id(&self) -> &str {
        &self.validator_id
    }

    /// Get metrics
    pub fn get_metrics(&self) -> crate::metrics::SwarmMetrics {
        self.metrics.get_swarm_metrics()
    }

    /// Get health status
    pub fn health_status(&self) -> crate::metrics::HealthStatus {
        self.health.get_overall_status()
    }

    /// Record heartbeat
    pub fn record_heartbeat(&self) {
        let mut tracker = self.health_tracker.write();
        tracker.record_heartbeat();
    }

    /// Get uptime
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Export metrics as JSON
    pub fn export_metrics_json(&self) -> SwarmResult<String> {
        self.metrics.export_json()
    }

    /// Get quarantine status
    pub fn get_quarantine_status(&self) -> Option<crate::quarantine::QuarantineStatus> {
        self.quarantine.get_status(&self.validator_id)
    }

    /// Enable CPU mode
    pub fn enable_cpu_mode(&self) {
        *self.current_mode.write() = ExecutionMode::CpuFallback;
        self.engine.set_mode(ExecutionMode::CpuFallback);
    }

    /// Enable GPU mode
    pub fn enable_gpu_mode(&self) {
        *self.current_mode.write() = ExecutionMode::GpuWithCpuVerification;
        self.engine.set_mode(ExecutionMode::GpuWithCpuVerification);
    }

    /// Get current execution mode
    pub fn current_mode(&self) -> ExecutionMode {
        *self.current_mode.read()
    }

    /// Shutdown
    pub fn shutdown(&self) {
        *self.state.write() = ValidatorState::Stopped;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_creation() {
        let config = SwarmConfig::default();
        let validator = Validator::new(config, "test-validator".to_string());

        assert_eq!(validator.id(), "test-validator");
        assert_eq!(validator.state(), ValidatorState::Starting);
    }

    #[test]
    fn test_validator_task() {
        let config = SwarmConfig::default();
        let validator = Validator::new(config, "test-validator".to_string());

        validator.initialize().unwrap();

        let task = DeterministicTask::new(
            crate::deterministic::TaskType::BatchHash,
            vec![b"hello".to_vec(), b"world".to_vec()],
            HashAlgorithm::Keccak256,
        );

        let result = validator.process_task(task);
        assert!(result.outputs.len() == 2);
    }
}
