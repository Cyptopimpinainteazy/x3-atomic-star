//! X3 Shadow Runner
//!
//! Shadow execution engine that replays blocks in an isolated environment
//! to verify correctness without affecting the main chain state.

// Off-chain tooling: no `std` feature is declared here, so the `no_std` attribute this crate
// carried made it permanently no_std while the code uses `Vec`, `String` and `format!`. It
// never compiled. Off-chain tooling is std.

use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use x3_autonomic_types::{
    AuditEvent, AutonomyLevel, HealthStatus, Severity, ShadowExecutionResult,
};

/// Configuration for the shadow runner
#[derive(Debug, Clone, Encode, Decode, TypeInfo)]
pub struct ShadowRunnerConfig {
    /// Maximum blocks to keep in memory for replay
    pub block_cache_size: u32,
    /// Whether to enable state root verification
    pub verify_state_roots: bool,
    /// Timeout for shadow execution per block (milliseconds)
    pub execution_timeout_ms: u64,
    /// Whether to run in full verification mode
    pub full_verification: bool,
}

impl Default for ShadowRunnerConfig {
    fn default() -> Self {
        Self {
            block_cache_size: 100,
            verify_state_roots: true,
            execution_timeout_ms: 5000,
            full_verification: false,
        }
    }
}

/// Shadow execution engine for block verification
pub struct ShadowRunner {
    config: ShadowRunnerConfig,
    current_autonomy_level: AutonomyLevel,
}

impl ShadowRunner {
    /// Create a new shadow runner with the given configuration
    pub fn new(config: ShadowRunnerConfig) -> Self {
        Self {
            config,
            current_autonomy_level: AutonomyLevel::Manual,
        }
    }

    /// Get the current configuration
    pub fn config(&self) -> &ShadowRunnerConfig {
        &self.config
    }

    /// Set the autonomy level for shadow execution
    pub fn set_autonomy_level(&mut self, level: AutonomyLevel) {
        self.current_autonomy_level = level;
    }

    /// Get the current autonomy level
    pub fn autonomy_level(&self) -> AutonomyLevel {
        self.current_autonomy_level
    }

    /// Execute a block in shadow mode and return the result
    ///
    /// # Refused, not fabricated
    ///
    /// This returned `Ok` with `state_root_matches: true`, `execution_time_ms: 0` and no events
    /// for any block, without executing anything (`extrinsics` was unused). A shadow runner that
    /// always reports a match is worse than one that reports nothing: the whole point of the
    /// comparison is to catch a divergence. There is no isolated runtime wired here to replay a
    /// block in, so it refuses and says so.
    pub fn execute_shadow_block(
        &self,
        block_hash: &[u8],
        extrinsics: &[Vec<u8>],
    ) -> Result<ShadowExecutionResult, ShadowRunnerError> {
        let _ = (block_hash, extrinsics);
        Err(ShadowRunnerError::IsolatedRuntimeUnavailable)
    }

    /// Verify that shadow execution results match expected state
    ///
    /// This returned `true` for every input ("Simplified verification"): a verifier that cannot
    /// fail verifies nothing. It now compares the state root the shadow execution produced
    /// against the production root, and refuses when there is no root to compare.
    pub fn verify_shadow_result(
        &self,
        result: &ShadowExecutionResult,
        expected_root: &[u8],
    ) -> Result<bool, ShadowRunnerError> {
        if !self.config.verify_state_roots {
            return Ok(true); // Verification is switched off in this configuration.
        }
        if result.state_root.is_empty() {
            return Err(ShadowRunnerError::StateRootMissing);
        }
        Ok(result.state_root == expected_root)
    }

    /// Check if autonomy level allows automatic action
    pub fn can_auto_act(&self) -> bool {
        self.current_autonomy_level.allows_autonomous_change()
    }
}

/// Errors that can occur during shadow execution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShadowRunnerError {
    /// Block not found in cache
    BlockNotFound,
    /// Execution timeout
    ExecutionTimeout,
    /// State root mismatch
    StateRootMismatch,
    /// Invalid extrinsics
    InvalidExtrinsics,
    /// Runtime error during shadow execution
    RuntimeError(String),
    /// No isolated runtime is wired in to replay the block.
    IsolatedRuntimeUnavailable,
    /// The result carries no state root, so there is nothing to compare.
    StateRootMissing,
}

impl core::fmt::Display for ShadowRunnerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BlockNotFound => write!(f, "Block not found in cache"),
            Self::ExecutionTimeout => write!(f, "Shadow execution timed out"),
            Self::StateRootMismatch => write!(f, "State root mismatch"),
            Self::InvalidExtrinsics => write!(f, "Invalid extrinsics provided"),
            Self::RuntimeError(msg) => write!(f, "Runtime error: {}", msg),
            Self::IsolatedRuntimeUnavailable => write!(
                f,
                "shadow execution is not wired into this node: no isolated runtime to replay a block"
            ),
            Self::StateRootMissing => write!(
                f,
                "the shadow result carries no state root, so there is nothing to verify"
            ),
        }
    }
}

impl From<ShadowRunnerError> for AuditEvent {
    fn from(err: ShadowRunnerError) -> Self {
        AuditEvent::Error {
            severity: Severity::Critical,
            component: "x3-shadow-runner".into(),
            message: err.to_string().into_bytes(),
            context: None,
        }
    }
}

impl std::error::Error for ShadowRunnerError {}

/// Health check for the shadow runner
pub fn health_check() -> HealthStatus {
    // Shadow runner is healthy if it can be instantiated
    HealthStatus::Healthy
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result_with_root(root: &[u8]) -> ShadowExecutionResult {
        ShadowExecutionResult {
            block_hash: b"block".to_vec(),
            state_root: root.to_vec(),
            execution_time_ms: 1,
            state_root_matches: false,
            events: vec![],
            errors: vec![],
        }
    }

    /// `execute_shadow_block` returned `Ok` with `state_root_matches: true` and no events for any
    /// block, without executing anything. A shadow runner that always reports a match cannot do
    /// the one thing it exists for. It refuses now.
    #[test]
    fn shadow_execution_refuses_without_an_isolated_runtime() {
        let runner = ShadowRunner::new(ShadowRunnerConfig::default());
        let error = runner
            .execute_shadow_block(b"block", &[vec![1, 2, 3]])
            .expect_err("there is no isolated runtime to replay a block in");
        assert!(
            error.to_string().contains("not wired into this node"),
            "{error}"
        );
    }

    /// `verify_shadow_result` returned `true` for every input. It compares the roots now, and
    /// refuses when there is nothing to compare.
    #[test]
    fn verification_compares_roots_and_refuses_when_there_are_none() {
        let runner = ShadowRunner::new(ShadowRunnerConfig::default());

        assert_eq!(
            runner.verify_shadow_result(&result_with_root(b"root-a"), b"root-a"),
            Ok(true)
        );
        assert_eq!(
            runner.verify_shadow_result(&result_with_root(b"root-a"), b"root-b"),
            Ok(false)
        );
        assert_eq!(
            runner.verify_shadow_result(&result_with_root(b""), b"root-a"),
            Err(ShadowRunnerError::StateRootMissing)
        );

        // With verification switched off, the runner says so instead of pretending to compare.
        let unverified = ShadowRunner::new(ShadowRunnerConfig {
            verify_state_roots: false,
            ..ShadowRunnerConfig::default()
        });
        assert_eq!(
            unverified.verify_shadow_result(&result_with_root(b""), b"root-a"),
            Ok(true)
        );
    }

    /// The autonomy gate is the ladder's answer, not a list of variants that do not exist.
    #[test]
    fn autonomous_action_follows_the_autonomy_ladder() {
        let mut runner = ShadowRunner::new(ShadowRunnerConfig::default());
        assert!(!runner.can_auto_act());
        runner.set_autonomy_level(AutonomyLevel::Canary);
        assert!(runner.can_auto_act());
        runner.set_autonomy_level(AutonomyLevel::Manual);
        assert!(!runner.can_auto_act());
    }
}
