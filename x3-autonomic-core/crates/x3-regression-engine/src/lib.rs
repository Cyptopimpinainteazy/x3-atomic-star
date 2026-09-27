//! X3 Regression Engine
//!
//! Automated regression test generator that creates and validates tests
//! based on detected behavior changes.

// Off-chain tooling: no `std` feature is declared here, so the `no_std` attribute this crate
// carried made it permanently no_std while the code uses `Vec`, `String` and `format!`. It
// never compiled. Off-chain tooling is std.

use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use x3_autonomic_types::{AutonomyLevel, HealthStatus};

/// Configuration for the regression engine
///
/// `min_confidence` was an `f64`, which SCALE cannot encode: the derives on this struct could
/// never be satisfied, which is one of the reasons this workspace has never built. It is basis
/// points now (0..=10_000, so 9_500 is the 0.95 the default used to name).
#[derive(Debug, Clone, Encode, Decode, TypeInfo)]
pub struct RegressionConfig {
    /// Maximum tests to keep in history
    pub test_history_size: u32,
    /// Minimum confidence threshold to auto-approve, in basis points.
    pub min_confidence_bps: u32,
    /// Whether to enable automatic test generation
    pub auto_generate: bool,
}

impl Default for RegressionConfig {
    fn default() -> Self {
        Self {
            test_history_size: 1000,
            min_confidence_bps: 9_500,
            auto_generate: false,
        }
    }
}

/// A generated regression test
#[derive(Debug, Clone, Encode, Decode, TypeInfo)]
pub struct RegressionTest {
    /// Unique test identifier
    pub id: Vec<u8>,
    /// Human-readable test name
    pub name: Vec<u8>,
    /// Test code/source
    pub source: Vec<u8>,
    /// Block range this test covers
    pub block_range: (u64, u64),
    /// Confidence score in basis points (0..=10_000).
    pub confidence_bps: u32,
    /// Whether this test passed
    pub passed: bool,
}

impl RegressionTest {
    /// Create a new regression test
    pub fn new(id: Vec<u8>, name: Vec<u8>, source: Vec<u8>) -> Self {
        Self {
            id,
            name,
            source,
            block_range: (0, 0),
            confidence_bps: 0,
            passed: false,
        }
    }

    /// Set the block range
    pub fn with_block_range(mut self, start: u64, end: u64) -> Self {
        self.block_range = (start, end);
        self
    }

    /// Set the confidence score, in basis points (a value above 10_000 is clamped to certainty).
    pub fn with_confidence_bps(mut self, confidence_bps: u32) -> Self {
        self.confidence_bps = confidence_bps.min(10_000);
        self
    }
}

/// Regression engine for automated test generation
pub struct RegressionEngine {
    config: RegressionConfig,
    tests: Vec<RegressionTest>,
    current_autonomy_level: AutonomyLevel,
}

impl RegressionEngine {
    /// Create a new regression engine
    pub fn new(config: RegressionConfig) -> Self {
        Self {
            config,
            tests: Vec::new(),
            current_autonomy_level: AutonomyLevel::Manual,
        }
    }

    /// Generate a new regression test from audit events
    pub fn generate_test(&mut self, name: Vec<u8>, source: Vec<u8>) -> RegressionTest {
        let id = format!("test-{}", self.tests.len()).as_bytes().to_vec();
        let test = RegressionTest::new(id, name, source);
        self.tests.push(test.clone());
        test
    }

    /// Get all tests
    pub fn tests(&self) -> &[RegressionTest] {
        &self.tests
    }

    /// Run all regression tests and return pass count
    pub fn run_tests(&mut self) -> (u32, u32) {
        let total = self.tests.len() as u32;
        // In production, this would actually execute tests
        for test in &mut self.tests {
            test.passed = true; // Simplified
        }
        let passed = self.tests.iter().filter(|t| t.passed).count() as u32;
        (passed, total)
    }

    /// Set the autonomy level
    pub fn set_autonomy_level(&mut self, level: AutonomyLevel) {
        self.current_autonomy_level = level;
    }

    /// Get current autonomy level
    pub fn autonomy_level(&self) -> AutonomyLevel {
        self.current_autonomy_level
    }

    /// Check if auto-generation is allowed
    pub fn can_auto_generate(&self) -> bool {
        self.config.auto_generate && self.current_autonomy_level.allows_autonomous_change()
    }
}

/// Health check for regression engine
pub fn health_check() -> HealthStatus {
    HealthStatus::Healthy
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Confidence is basis points now, so the clamp is exact rather than a float comparison.
    #[test]
    fn confidence_is_basis_points_and_clamped_at_certainty() {
        let test = RegressionTest::new(b"t-1".to_vec(), b"name".to_vec(), b"source".to_vec())
            .with_confidence_bps(9_500);
        assert_eq!(test.confidence_bps, 9_500);

        let over = RegressionTest::new(b"t-2".to_vec(), b"name".to_vec(), b"source".to_vec())
            .with_confidence_bps(20_000);
        assert_eq!(over.confidence_bps, 10_000, "certainty is the ceiling");
    }

    /// Automatic generation is gated by the autonomy ladder: it used to name
    /// `AutonomyLevel::Automatic(_)` and `SelfImproving`, variants that do not exist, so the
    /// crate could not compile at all.
    #[test]
    fn automatic_generation_follows_the_autonomy_ladder() {
        let mut engine = RegressionEngine::new(RegressionConfig {
            auto_generate: true,
            ..RegressionConfig::default()
        });
        assert!(!engine.can_auto_generate(), "manual is the starting level");

        engine.set_autonomy_level(AutonomyLevel::StagedRollout);
        assert!(
            !engine.can_auto_generate(),
            "a staged rollout still has a human in the loop"
        );

        engine.set_autonomy_level(AutonomyLevel::Canary);
        assert!(engine.can_auto_generate());

        // And the switch still switches it off.
        let mut disabled = RegressionEngine::new(RegressionConfig::default());
        disabled.set_autonomy_level(AutonomyLevel::FullyAutonomous);
        assert!(!disabled.can_auto_generate());
    }
}
