//! X3 Judge Agent
//!
//! AI judge agent that evaluates upgrade proposals and makes autonomous
//! decisions about system improvements for the X3 Autonomic Core.

// Off-chain tooling: no `std` feature is declared here, so the `no_std` attribute this crate
// carried made it permanently no_std while the code uses `Vec`, `String` and `format!`. It
// never compiled. Off-chain tooling is std.

use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use x3_autonomic_types::{AutonomyLevel, HealthStatus, Severity, UpgradeProposal};

/// Configuration for the judge agent
///
/// `approval_threshold` was an `f64`, which SCALE cannot encode — none of these derives could be
/// satisfied, so the crate has never compiled. It is basis points now: the default `0.7` is
/// `7_000`.
#[derive(Debug, Clone, Encode, Decode, TypeInfo)]
pub struct JudgeConfig {
    /// Minimum approval threshold in basis points (0..=10_000).
    pub approval_threshold_bps: u32,
    /// Enable autonomous decision making
    pub autonomous_enabled: bool,
    /// Maximum proposal age in blocks
    pub max_proposal_age: u64,
    /// Require unanimous consent for critical upgrades
    pub require_unanimous_critical: bool,
}

impl Default for JudgeConfig {
    fn default() -> Self {
        Self {
            approval_threshold_bps: 7_000,
            autonomous_enabled: false,
            max_proposal_age: 10080, // ~1 week at 6s blocks
            require_unanimous_critical: true,
        }
    }
}

/// Decision made by the judge agent
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub enum JudgeDecision {
    /// Proposal approved for implementation
    Approved,
    /// Proposal rejected
    Rejected,
    /// More information needed
    NeedsMoreInfo,
    /// Deferred for later review
    Deferred,
    /// Proposal requires human review
    RequiresHumanReview,
}

/// Justification for a judge decision
#[derive(Debug, Clone, Encode, Decode, TypeInfo)]
pub struct DecisionJustification {
    /// Decision made
    pub decision: JudgeDecision,
    /// Confidence score in basis points (0..=10_000).
    pub confidence_bps: u32,
    /// Reasoning summary
    pub reasoning: Vec<u8>,
    /// Supporting evidence
    pub evidence: Vec<Vec<u8>>,
}

/// Judge agent for upgrade decisions
pub struct JudgeAgent {
    config: JudgeConfig,
    current_autonomy_level: AutonomyLevel,
}

impl JudgeAgent {
    /// Create a new judge agent
    pub fn new(config: JudgeConfig) -> Self {
        Self {
            config,
            current_autonomy_level: AutonomyLevel::Manual,
        }
    }

    /// Evaluate an upgrade proposal
    pub fn evaluate(&self, proposal: &UpgradeProposal) -> DecisionJustification {
        // The decision reads the proposal now. It used to ignore it entirely — the argument was
        // unused, and the answer came only from the autonomy gate, so the "judge" judged nothing
        // and its own reasoning string admitted it ("Simplified evaluation").
        //
        // Two rules, both from configured policy rather than from a model:
        //   * a critical proposal with `require_unanimous_critical` always goes to a human;
        //   * everything else follows the autonomy gate.
        let needs_human = (proposal.severity == Severity::Critical
            && self.config.require_unanimous_critical)
            || !self.can_auto_decide();
        let decision = if needs_human {
            JudgeDecision::RequiresHumanReview
        } else {
            JudgeDecision::Approved
        };

        let reasoning = match (proposal.severity, needs_human) {
            (Severity::Critical, true) => {
                b"critical proposal: unanimous human consent is required by policy".to_vec()
            }
            (_, true) => b"the autonomy level does not permit an automatic decision".to_vec(),
            _ => b"within the autonomy level and below the critical review threshold".to_vec(),
        };

        DecisionJustification {
            decision,
            confidence_bps: 8_500,
            reasoning,
            evidence: vec![],
        }
    }

    /// Set the autonomy level
    pub fn set_autonomy_level(&mut self, level: AutonomyLevel) {
        self.current_autonomy_level = level;
    }

    /// Get current autonomy level
    pub fn autonomy_level(&self) -> AutonomyLevel {
        self.current_autonomy_level
    }

    /// Check if autonomous decisions are enabled
    pub fn can_auto_decide(&self) -> bool {
        self.config.autonomous_enabled && self.current_autonomy_level.allows_autonomous_change()
    }

    /// Check if human review is required
    pub fn requires_human_review(&self, proposal: &UpgradeProposal) -> bool {
        proposal.severity == Severity::Critical && self.config.require_unanimous_critical
    }
}

/// Health check for judge agent
pub fn health_check() -> HealthStatus {
    HealthStatus::Healthy
}

#[cfg(test)]
mod tests {
    use super::*;
    use x3_autonomic_types::ProposalStatus;

    fn proposal(severity: Severity) -> UpgradeProposal {
        UpgradeProposal {
            id: b"upgrade-1".to_vec(),
            description: b"raise the block weight limit".to_vec(),
            proposed_by: b"council".to_vec(),
            autonomy_level: AutonomyLevel::Canary,
            target_block: Some(100),
            code_hash: None,
            canary_percentage: 10,
            created_at: 1,
            status: ProposalStatus::Pending,
            severity,
        }
    }

    /// The decision reads the proposal. It used to ignore it: the argument was unused and the
    /// answer came only from the autonomy gate, with a reasoning string that said "Simplified
    /// evaluation".
    #[test]
    fn a_critical_proposal_goes_to_a_human_even_at_full_autonomy() {
        let mut judge = JudgeAgent::new(JudgeConfig {
            autonomous_enabled: true,
            require_unanimous_critical: true,
            ..JudgeConfig::default()
        });
        judge.set_autonomy_level(AutonomyLevel::FullyAutonomous);

        let decision = judge.evaluate(&proposal(Severity::Critical));
        assert_eq!(decision.decision, JudgeDecision::RequiresHumanReview);
        assert!(String::from_utf8_lossy(&decision.reasoning).contains("critical"));

        // A warning-severity proposal at the same setting does not need a human.
        let decision = judge.evaluate(&proposal(Severity::Warning));
        assert_eq!(decision.decision, JudgeDecision::Approved);
        assert_eq!(decision.confidence_bps, 8_500);
    }

    #[test]
    fn a_manual_autonomy_level_always_needs_a_human() {
        let judge = JudgeAgent::new(JudgeConfig {
            autonomous_enabled: true,
            ..JudgeConfig::default()
        });
        assert_eq!(
            judge.evaluate(&proposal(Severity::Info)).decision,
            JudgeDecision::RequiresHumanReview
        );
        assert!(
            String::from_utf8_lossy(&judge.evaluate(&proposal(Severity::Info)).reasoning)
                .contains("autonomy level")
        );
    }
}
