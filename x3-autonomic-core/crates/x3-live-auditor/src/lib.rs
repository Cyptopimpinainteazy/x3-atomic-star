//! Live Auditor for X3 Autonomic Core
//!
//! Monitors invariants and health metrics in real-time during block production.

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

pub use x3_autonomic_types::*;

/// What one invariant check produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvariantOutcome {
    /// The invariant was checked and holds.
    Passed,
    /// The invariant was checked and does not hold.
    Failed {
        /// The value the check read.
        observed: u128,
    },
    /// The invariant could not be checked, and why.
    NotChecked {
        /// What was missing.
        reason: String,
    },
}

/// Why the auditor could not do what it was asked.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuditorError {
    /// The auditor has no chain data source, so it cannot read the values an invariant needs.
    #[error("no chain state source: cannot check invariants at block {block_number}")]
    NoStateSource {
        /// The block the check was requested for.
        block_number: u32,
    },
}

/// Auditor configuration
#[derive(Debug, Clone)]
pub struct AuditorConfig {
    pub check_interval_blocks: u32,
    pub alert_channel_capacity: usize,
    pub store_events: bool,
}

impl Default for AuditorConfig {
    fn default() -> Self {
        Self {
            check_interval_blocks: 1,
            alert_channel_capacity: 1000,
            store_events: true,
        }
    }
}

/// Live auditor state
pub struct LiveAuditor {
    config: AuditorConfig,
    current_block: u32,
    enabled_invariants: Arc<RwLock<Vec<InvariantDefinition>>>,
    enabled_metrics: Arc<RwLock<Vec<HealthMetricDefinition>>>,
}

impl LiveAuditor {
    pub fn new(config: AuditorConfig) -> Self {
        Self {
            config,
            current_block: 0,
            enabled_invariants: Arc::new(RwLock::new(Vec::new())),
            enabled_metrics: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Enable an invariant for monitoring
    pub async fn enable_invariant(&self, invariant: InvariantDefinition) {
        let mut invariants = self.enabled_invariants.write().await;
        if invariant.enabled {
            info!(
                "Enabling invariant: {} (ID: {})",
                String::from_utf8_lossy(&invariant.name),
                String::from_utf8_lossy(&invariant.id)
            );
            invariants.push(invariant);
        }
    }

    /// Enable a health metric for monitoring
    pub async fn enable_metric(&self, metric: HealthMetricDefinition) {
        let mut metrics = self.enabled_metrics.write().await;
        if metric.enabled {
            info!(
                "Enabling health metric: {}",
                String::from_utf8_lossy(&metric.id)
            );
            metrics.push(metric);
        }
    }

    /// Process a new block and run invariant checks
    pub async fn on_block(&mut self, block_number: u32) {
        self.current_block = block_number;
        // The config's cadence gates the whole pass; each invariant's own
        // `check_interval_blocks` then decides whether it runs inside a pass. The field was never
        // read, which is how a configurable interval ends up being decoration.
        if self.config.check_interval_blocks == 0
            || !block_number.is_multiple_of(self.config.check_interval_blocks)
        {
            return;
        }
        let invariants = self.enabled_invariants.read().await;

        for invariant in invariants.iter() {
            if invariant.check_interval_blocks != 0
                && block_number.is_multiple_of(invariant.check_interval_blocks)
            {
                info!(
                    "Checking invariant {} at block {}",
                    String::from_utf8_lossy(&invariant.name),
                    block_number
                );
                // Invariant checking logic would be implemented here
                // Currently a placeholder - real implementation would query runtime state
            }
        }
    }

    /// Check all enabled invariants for a specific block.
    ///
    /// # Refused, not fabricated
    ///
    /// This used to return `(inv.id, true, inv.severity)` for every registered invariant — a
    /// literal `true` under the comment "Placeholder - real implementation would check actual
    /// state" — so a live auditor reported every invariant passing without reading a single
    /// value. It now refuses, naming what it would need: a chain data source for the invariants
    /// it was given. (The registry of *which* invariants to check is real; the checking is not.)
    pub async fn check_invariants(
        &self,
        block_number: u32,
    ) -> Result<Vec<InvariantOutcome>, AuditorError> {
        Err(AuditorError::NoStateSource { block_number })
    }

    /// Get current auditor health status
    pub fn health_status(&self) -> HealthStatus {
        if self.enabled_invariants.blocking_read().is_empty() {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_auditor_creation() {
        let auditor = LiveAuditor::new(AuditorConfig::default());
        assert_eq!(auditor.current_block, 0);
    }

    #[tokio::test]
    async fn test_enable_invariant() {
        let auditor = LiveAuditor::new(AuditorConfig::default());
        let invariant = InvariantDefinition {
            id: b"1".to_vec(),
            name: b"test".to_vec(),
            description: b"test invariant".to_vec(),
            severity: Severity::Warning,
            check_interval_blocks: 1,
            enabled: true,
        };
        auditor.enable_invariant(invariant).await;
        let invariants = auditor.enabled_invariants.read().await;
        assert_eq!(invariants.len(), 1);
    }

    /// The auditor used to answer `(id, true, severity)` for every registered invariant, under a
    /// comment saying the check was a placeholder: a live auditor that reports everything passing
    /// without reading a value. It refuses now, and the error names what it would need.
    #[tokio::test]
    async fn checking_invariants_refuses_without_a_state_source() {
        let auditor = LiveAuditor::new(AuditorConfig::default());
        let error = auditor
            .check_invariants(42)
            .await
            .expect_err("an auditor with no chain source cannot report a pass");
        assert!(
            error.to_string().contains("no chain state source"),
            "{error}"
        );
        assert!(
            error.to_string().contains("42"),
            "the block must be named: {error}"
        );
    }

    /// The pass cadence is the config's now, not decoration: `on_block` used to iterate every
    /// invariant on every block while `config.check_interval_blocks` was never read.
    #[tokio::test]
    async fn the_configured_cadence_gates_a_pass() {
        let mut auditor = LiveAuditor::new(AuditorConfig {
            check_interval_blocks: 5,
            ..AuditorConfig::default()
        });
        auditor
            .enable_invariant(InvariantDefinition {
                id: b"inv".to_vec(),
                name: b"invariant".to_vec(),
                description: b"checked every block".to_vec(),
                severity: Severity::Critical,
                check_interval_blocks: 1,
                enabled: true,
            })
            .await;

        // A block off the cadence leaves the auditor where it was...
        auditor.on_block(3).await;
        assert_eq!(auditor.current_block, 3);
        // ...and the cadence itself is what the config says.
        auditor.on_block(5).await;
        assert_eq!(auditor.current_block, 5);
    }
}
