//! Core court types.

use serde::{Deserialize, Serialize};
use x3_proof::types::{AgentIdentity, BlockHeight, Hash256, IntentId};

/// Dispute identifier.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DisputeId(pub u64);

/// Dispute lifecycle state.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DisputeState {
    /// Dispute filed, pending replay.
    Filed,
    /// Replay in progress.
    Replaying,
    /// Verdict rendered.
    Resolved,
    /// Dispute dismissed (invalid filing).
    Dismissed,
}

/// The type of dispute being filed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DisputeType {
    /// Execution diverged from deterministic expectation.
    ExecutionDivergence {
        /// The proof chain being disputed.
        proof_chain_hash: Hash256,
    },
    /// Agent submitted invalid proof.
    InvalidProof {
        /// The specific proof hash being contested.
        proof_hash: Hash256,
    },
    /// Agent executed the same intent twice.
    DoubleExecution {
        intent_id: IntentId,
        first_proof: Hash256,
        second_proof: Hash256,
    },
    /// Execution result doesn't match the proof chain.
    ResultMismatch {
        intent_id: IntentId,
        claimed_result: Hash256,
        actual_result: Hash256,
    },
}

/// A dispute filed with the court.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dispute {
    /// Unique dispute ID.
    pub id: DisputeId,
    /// Type of dispute.
    pub dispute_type: DisputeType,
    /// Agent being disputed.
    pub respondent: AgentIdentity,
    /// Block at which the dispute was filed.
    pub filed_at: BlockHeight,
    /// Deadline for resolution (finality window).
    pub deadline: BlockHeight,
    /// Current state.
    pub state: DisputeState,
    /// The verdict, once resolved.
    pub verdict: Option<VerdictRecord>,
}

/// A court verdict — immutable and final.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerdictRecord {
    /// The dispute this verdict resolves.
    pub dispute_id: DisputeId,
    /// The outcome.
    pub outcome: VerdictOutcome,
    /// Block at which the verdict was rendered.
    pub rendered_at: BlockHeight,
    /// Hash of the replay proof (if applicable).
    pub replay_proof_hash: Option<Hash256>,
    /// Amount ordered slashed (0 if acquitted).
    pub slash_amount: u128,
    /// Hash of the verdict record.
    pub verdict_hash: Hash256,
}

/// Verdict outcome.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum VerdictOutcome {
    /// Agent is guilty — slashing enforced.
    Guilty,
    /// Agent is not guilty — dispute dismissed.
    NotGuilty,
    /// Dispute is invalid (malformed, wrong target, etc.).
    InvalidDispute,
}

/// Court configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CourtConfig {
    /// Maximum blocks allowed for dispute resolution.
    pub finality_window: u64,
    /// Minimum bond required to file a dispute (anti-spam).
    pub dispute_bond: u128,
    /// Whether auto-slashing is enabled on guilty verdicts.
    pub auto_slash: bool,
}

impl Default for CourtConfig {
    fn default() -> Self {
        Self {
            finality_window: 100,
            dispute_bond: 100_000,
            auto_slash: true,
        }
    }
}
