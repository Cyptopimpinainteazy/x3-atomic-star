//! Shared types for X3 Autonomic Core
//!
//! Common data structures, traits, and type definitions used across
//! all autonomic core components.

// The manifest declares this dependency as `scale = { package = "parity-scale-codec" }`, so
// the crate is nameable only as `scale`; the import used to say `parity_scale_codec`, which
// does not exist here.
use scale::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// 32-byte hash type
#[derive(Debug, Clone, Default, PartialEq, Eq, Encode, Decode, MaxEncodedLen, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct H256(pub [u8; 32]);

/// Autonomy level representing system self-governance capability
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Encode, Decode, MaxEncodedLen, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AutonomyLevel {
    /// Level 0: Fully manual, human-only control
    Manual = 0,
    /// Level 1: Automated monitoring and alerting
    #[default]
    Monitored = 1,
    /// Level 2: Automated detection with human approval
    DetectedHumanApproval = 2,
    /// Level 3: Automated detection with staged rollout
    StagedRollout = 3,
    /// Level 4: Automated detection with canary deployment
    Canary = 4,
    /// Level 5: Fully autonomous self-improvement
    FullyAutonomous = 5,
}

impl AutonomyLevel {
    /// Whether this level lets the autonomic core change itself without a human in the loop.
    ///
    /// `x3-regression-engine` asked the question as `AutonomyLevel::Automatic(_) |
    /// AutonomyLevel::SelfImproving`, two variants that do not exist — the crate could not
    /// compile. The ladder above answers it: canary deployment and full autonomy are the levels
    /// where automatic change is intended; everything below them keeps a human in the loop.
    pub fn allows_autonomous_change(self) -> bool {
        matches!(self, AutonomyLevel::Canary | AutonomyLevel::FullyAutonomous)
    }
}

/// Severity level for invariant violations and findings
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Encode, Decode, MaxEncodedLen, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum Severity {
    #[default]
    Info = 0,
    Warning = 1,
    Critical = 2,
    Emergency = 3,
}

/// Health status of the X3 runtime
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Encode, Decode, MaxEncodedLen, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum HealthStatus {
    #[default]
    Healthy,
    Degraded,
    Critical,
    Emergency,
}

/// Audit event types
///
/// Serde only: `HealthMetricUpdated` carries `f64`, and SCALE has no float representation —
/// `scale-info` has no `TypeInfo` for it and `parity-scale-codec` cannot encode it. These are
/// the autonomic control plane's off-chain audit events, published as JSON; nothing decodes
/// them on a chain, so the honest representation is the one that can carry the value.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AuditEvent {
    /// Invariant was checked
    InvariantChecked {
        invariant_id: u32,
        passed: bool,
        actual_value: u128,
        expected_range_min: u128,
        expected_range_max: u128,
    },
    /// Health metric updated
    HealthMetricUpdated {
        metric_id: Vec<u8>,
        value: f64,
        threshold: f64,
    },
    /// Block shadow execution completed
    ShadowExecutionCompleted {
        block_hash: H256,
        matches_production: bool,
        execution_time_ms: u64,
    },
    /// Regression test generated
    RegressionTestGenerated {
        test_name: Vec<u8>,
        failure_description: Vec<u8>,
        block_hash: H256,
    },
    /// Upgrade proposal created
    UpgradeProposed {
        proposal_id: H256,
        autonomy_level: AutonomyLevel,
        description: Vec<u8>,
    },
    /// Something in the autonomic core failed.
    ///
    /// `x3-shadow-runner::ShadowRunnerError` converts into this variant (its `From` impl named
    /// `AuditEvent::Error`, which did not exist, so that crate could not compile).
    Error {
        severity: Severity,
        component: Vec<u8>,
        message: Vec<u8>,
        context: Option<Vec<u8>>,
    },
}

/// Result of a shadow execution comparison
#[derive(Debug, Clone, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct ShadowExecutionResult {
    pub block_hash: Vec<u8>,
    /// The state root the shadow execution produced, empty when it produced none.
    ///
    /// `verify_shadow_result` compares this against the production root; without the field the
    /// comparison had nothing to compare and returned `true`.
    pub state_root: Vec<u8>,
    pub execution_time_ms: u64,
    pub state_root_matches: bool,
    pub events: Vec<Vec<u8>>,
    pub errors: Vec<Vec<u8>>,
}

/// Performance benchmark metrics
///
/// Serde only, for the same reason as [`AuditEvent`]: every field is a measured `f64`, and this
/// is an off-chain report consumed by `x3-benchmark-oracle`, not a consensus type.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct PerformanceMetrics {
    pub block_time_avg_ms: f64,
    pub block_time_p95_ms: f64,
    pub block_time_p99_ms: f64,
    pub tx_throughput_per_block: f64,
    pub storage_read_avg_us: f64,
    pub storage_write_avg_us: f64,
    pub vm_execution_evm_ms: f64,
    pub vm_execution_svm_ms: f64,
    pub memory_usage_mb: f64,
}

/// Upgrade proposal for governance
#[derive(Debug, Clone, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct UpgradeProposal {
    pub id: Vec<u8>,
    pub description: Vec<u8>,
    pub proposed_by: Vec<u8>,
    pub autonomy_level: AutonomyLevel,
    pub target_block: Option<u32>,
    pub code_hash: Option<Vec<u8>>,
    pub canary_percentage: u8,
    pub created_at: u64,
    pub status: ProposalStatus,
    pub severity: Severity,
}

/// Status of an upgrade proposal
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum ProposalStatus {
    #[default]
    Pending,
    Approved,
    Rejected,
    Staged {
        current_percentage: u8,
    },
    RolledBack,
    Executed,
}

/// Invariant definition
///
/// A storage value written by `pallet-x3-invariants::register_invariant`, so every field has to
/// be SCALE-decodable from a transaction. `name` and `description` were `&'static str`, which
/// has no `Decode` impl at all: the extrinsic's argument could not be decoded, so no caller
/// could ever have registered one. `id` is the storage key of the pallet's map (`Vec<u8>`), so
/// it is owned bytes here too.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct InvariantDefinition {
    pub id: Vec<u8>,
    pub name: Vec<u8>,
    pub description: Vec<u8>,
    pub severity: Severity,
    pub check_interval_blocks: u32,
    pub enabled: bool,
}

/// Health metric definition
///
/// Also a storage value (`pallet-x3-health::register_metric`), with the same two problems fixed
/// the same way — plus the thresholds: they were `f64`, and SCALE has no float representation,
/// so the metric's thresholds are basis points of the metric's range now (`10_000` = 100%).
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct HealthMetricDefinition {
    pub id: Vec<u8>,
    pub name: Vec<u8>,
    pub description: Vec<u8>,
    pub warning_threshold_bps: u32,
    pub critical_threshold_bps: u32,
    pub check_interval_blocks: u32,
    pub enabled: bool,
}

/// Predefined invariant IDs
pub mod invariants {
    pub const INVARIANT_TOTAL_SUPPLY: u32 = 1;
    pub const INVARIANT_BALANCE_NON_NEGATIVE: u32 = 2;
    pub const INVARIANT_STAKING_REWARDS_CAPPED: u32 = 3;
    pub const INVARIANT_GOVERNANCE_QUORUM: u32 = 4;
    pub const INVARIANT_CROSS_VM_STATE_CONSISTENCY: u32 = 5;
    pub const INVARIANT_BRIDGE_ESCROW_BALANCE: u32 = 6;
    pub const INVARIANT_DEX_RESERVES_CONSISTENCY: u32 = 7;
    pub const INVARIANT_NFT_TOTAL_SUPPLY: u32 = 8;
    pub const INVARIANT_FEE_BALANCE_NON_NEGATIVE: u32 = 9;
    pub const INVARIANT_AUTHORITY_SET_SIZE: u32 = 10;
    pub const INVARIANT_SCHEDULED_QUEUE_ORDER: u32 = 11;
    pub const INVARIANT_BLOCK_AUTHOR_REWARD: u32 = 12;
}

/// Predefined health metric IDs
pub mod health_metrics {
    pub const METRIC_BLOCK_TIME: &str = "block_time";
    pub const METRIC_TX_THROUGHPUT: &str = "tx_throughput";
    pub const METRIC_STORAGE_GROWTH: &str = "storage_growth";
    pub const METRIC_MEMORY_USAGE: &str = "memory_usage";
    pub const METRIC_PEER_COUNT: &str = "peer_count";
    pub const METRIC_SYNC_LAG: &str = "sync_lag";
    pub const METRIC_INVARIANT_VIOLATIONS: &str = "invariant_violations";
    pub const METRIC_UPGRADE_SUCCESS_RATE: &str = "upgrade_success_rate";
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two pallet-storage types have to survive a SCALE round trip. They could not even be
    /// *decoded* before: `name`/`description` were `&'static str` (no `Decode` impl) and the
    /// metric thresholds were `f64` (no SCALE representation at all), so the extrinsics that take
    /// them as arguments could not have been called by anyone.
    #[test]
    fn the_storage_types_round_trip_through_scale() {
        let invariant = InvariantDefinition {
            id: b"inv-1".to_vec(),
            name: b"total supply".to_vec(),
            description: b"the ledger's supply equals its issuance".to_vec(),
            severity: Severity::Critical,
            check_interval_blocks: 10,
            enabled: true,
        };
        let encoded = invariant.encode();
        assert_eq!(
            InvariantDefinition::decode(&mut &encoded[..]).expect("decodes"),
            invariant
        );

        let metric = HealthMetricDefinition {
            id: b"block_time".to_vec(),
            name: b"block time".to_vec(),
            description: b"average block interval".to_vec(),
            warning_threshold_bps: 6_000,
            critical_threshold_bps: 9_000,
            check_interval_blocks: 5,
            enabled: true,
        };
        let encoded = metric.encode();
        assert_eq!(
            HealthMetricDefinition::decode(&mut &encoded[..]).expect("decodes"),
            metric
        );
    }

    /// The audit event enum carries an `Error` variant because `x3-shadow-runner`'s
    /// `From<ShadowRunnerError> for AuditEvent` names it — and it did not exist, so that crate
    /// could not compile.
    #[test]
    fn an_audit_event_can_carry_a_component_failure() {
        let event = AuditEvent::Error {
            severity: Severity::Critical,
            component: b"x3-shadow-runner".to_vec(),
            message: b"no isolated runtime".to_vec(),
            context: None,
        };
        assert!(matches!(event, AuditEvent::Error { .. }));
    }

    /// The autonomy ladder now answers "may this level change itself?" in one place: the
    /// regression engine and the judge agent each asked it with variants that do not exist.
    #[test]
    fn the_autonomy_ladder_says_when_it_may_change_itself() {
        assert!(!AutonomyLevel::Manual.allows_autonomous_change());
        assert!(!AutonomyLevel::Monitored.allows_autonomous_change());
        assert!(!AutonomyLevel::DetectedHumanApproval.allows_autonomous_change());
        assert!(!AutonomyLevel::StagedRollout.allows_autonomous_change());
        assert!(AutonomyLevel::Canary.allows_autonomous_change());
        assert!(AutonomyLevel::FullyAutonomous.allows_autonomous_change());
    }
}
