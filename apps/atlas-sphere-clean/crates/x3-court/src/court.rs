//! The Court — deterministic dispute resolution engine.

use crate::docket::CourtDocket;
use crate::error::CourtError;
use crate::types::*;
use sha2::{Digest, Sha256};
use x3_proof::chain::ProofChain;
use x3_proof::types::{AgentIdentity, BlockHeight, Hash256};
use x3_proof::verifier::{ComparisonResult, ProofVerifier};

/// The X3 Court. No humans. No voting. No mercy.
///
/// Disputes are resolved by deterministic replay.
/// Verdicts are final. Slashing is automatic.
pub struct Court {
    /// Court docket (registry of all disputes).
    docket: CourtDocket,
    /// Configuration.
    config: CourtConfig,
    /// Next dispute ID.
    next_id: u64,
}

impl Court {
    /// Create a new court.
    pub fn new(config: CourtConfig) -> Self {
        Self {
            docket: CourtDocket::new(),
            config,
            next_id: 0,
        }
    }

    /// File a new dispute. Anyone can file — but they must bond.
    pub fn file_dispute(
        &mut self,
        dispute_type: DisputeType,
        respondent: AgentIdentity,
        current_block: BlockHeight,
    ) -> Result<DisputeId, CourtError> {
        let id = DisputeId(self.next_id);
        self.next_id += 1;

        let dispute = Dispute {
            id,
            dispute_type,
            respondent,
            filed_at: current_block,
            deadline: current_block + self.config.finality_window,
            state: DisputeState::Filed,
            verdict: None,
        };

        self.docket.register(dispute)?;
        Ok(id)
    }

    /// Adjudicate a dispute by replaying the execution.
    ///
    /// This is the core function. It takes the original proof chain
    /// and a replay proof chain, compares them, and renders a verdict.
    pub fn adjudicate(
        &mut self,
        dispute_id: DisputeId,
        original_chain: &ProofChain,
        replay_chain: &ProofChain,
        current_block: BlockHeight,
    ) -> Result<VerdictRecord, CourtError> {
        let dispute = self
            .docket
            .get_mut(dispute_id)
            .ok_or(CourtError::DisputeNotFound(dispute_id))?;

        if dispute.state != DisputeState::Filed {
            return Err(CourtError::DisputeNotFileable(dispute_id));
        }

        if current_block > dispute.deadline {
            dispute.state = DisputeState::Dismissed;
            return Err(CourtError::DeadlineExceeded(dispute_id));
        }

        dispute.state = DisputeState::Replaying;

        // Verify original chain integrity
        if let Err(_) = ProofVerifier::verify_chain(original_chain) {
            let verdict = self.render_verdict(
                dispute_id,
                VerdictOutcome::Guilty,
                None,
                0, // Slash amount determined by slashing engine
                current_block,
            );
            return Ok(verdict);
        }

        // Compare original vs replay
        let comparison = ProofVerifier::compare_chains(original_chain, replay_chain);

        let (outcome, replay_hash) = match comparison {
            ComparisonResult::Matched { chain_hash } => {
                // Execution was deterministic — agent is not guilty
                (VerdictOutcome::NotGuilty, Some(chain_hash))
            }
            ComparisonResult::Diverged { replay_hash, .. } => {
                // Execution diverged — agent is guilty
                (VerdictOutcome::Guilty, Some(replay_hash))
            }
        };

        let verdict = self.render_verdict(
            dispute_id,
            outcome,
            replay_hash,
            0, // Slash amount determined externally by the slashing engine
            current_block,
        );

        Ok(verdict)
    }

    /// Adjudicate a double-execution dispute.
    pub fn adjudicate_double_execution(
        &mut self,
        dispute_id: DisputeId,
        first_chain: &ProofChain,
        second_chain: &ProofChain,
        current_block: BlockHeight,
    ) -> Result<VerdictRecord, CourtError> {
        let _dispute = self
            .docket
            .get_mut(dispute_id)
            .ok_or(CourtError::DisputeNotFound(dispute_id))?;

        // If both chains exist and reference the same intent, it's double execution
        if first_chain.len() > 0 && second_chain.len() > 0 {
            let verdict =
                self.render_verdict(dispute_id, VerdictOutcome::Guilty, None, 0, current_block);
            return Ok(verdict);
        }

        let verdict = self.render_verdict(
            dispute_id,
            VerdictOutcome::NotGuilty,
            None,
            0,
            current_block,
        );
        Ok(verdict)
    }

    /// Render a verdict and record it.
    fn render_verdict(
        &mut self,
        dispute_id: DisputeId,
        outcome: VerdictOutcome,
        replay_proof_hash: Option<Hash256>,
        slash_amount: u128,
        current_block: BlockHeight,
    ) -> VerdictRecord {
        let mut verdict = VerdictRecord {
            dispute_id,
            outcome,
            rendered_at: current_block,
            replay_proof_hash,
            slash_amount,
            verdict_hash: [0u8; 32],
        };

        // Compute verdict hash
        verdict.verdict_hash = Self::hash_verdict(&verdict);

        // Update dispute state
        if let Some(dispute) = self.docket.get_mut(dispute_id) {
            dispute.state = DisputeState::Resolved;
            dispute.verdict = Some(verdict.clone());
        }

        verdict
    }

    /// Compute deterministic hash of a verdict.
    fn hash_verdict(verdict: &VerdictRecord) -> Hash256 {
        let mut hasher = Sha256::new();
        hasher.update(&verdict.dispute_id.0.to_le_bytes());
        hasher.update(&[verdict.outcome as u8]);
        hasher.update(&verdict.rendered_at.to_le_bytes());
        if let Some(h) = &verdict.replay_proof_hash {
            hasher.update(&[0x01]);
            hasher.update(h);
        } else {
            hasher.update(&[0x00]);
        }
        hasher.update(&verdict.slash_amount.to_le_bytes());
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// Process timed-out disputes.
    pub fn process_timeouts(&mut self, current_block: BlockHeight) -> Vec<DisputeId> {
        self.docket.process_timeouts(current_block)
    }

    /// Get the court docket.
    pub fn docket(&self) -> &CourtDocket {
        &self.docket
    }

    /// Get configuration.
    pub fn config(&self) -> &CourtConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x3_proof::hasher::DeterministicHasher;
    use x3_proof::types::ExecutionProof;

    fn test_agent() -> AgentIdentity {
        AgentIdentity {
            pubkey: [1u8; 32],
            ephemeral: false,
        }
    }

    fn make_chain(
        block: u64,
        program: Hash256,
        proofs: Vec<(Hash256, Hash256, u64)>,
    ) -> ProofChain {
        let mut chain = ProofChain::new(block, program);
        for (i, (pre, post, gas)) in proofs.iter().enumerate() {
            let mut proof = ExecutionProof {
                id: i as u64,
                block_height: block,
                program_hash: program,
                pre_state_hash: *pre,
                post_state_hash: *post,
                state_diffs: vec![],
                gas_consumed: *gas,
                fee_charged: 10,
                agent_id: test_agent(),
                intent_id: None,
                proof_hash: [0u8; 32],
            };
            proof.proof_hash = DeterministicHasher::hash_execution_proof(&proof);
            chain.append(proof).unwrap();
        }
        chain
    }

    #[test]
    fn test_matching_chains_not_guilty() {
        let mut court = Court::new(CourtConfig::default());

        let id = court
            .file_dispute(
                DisputeType::ExecutionDivergence {
                    proof_chain_hash: [0xAA; 32],
                },
                test_agent(),
                100,
            )
            .unwrap();

        let chain = make_chain(100, [0xAA; 32], vec![([0u8; 32], [1u8; 32], 100)]);

        let verdict = court.adjudicate(id, &chain, &chain, 105).unwrap();
        assert_eq!(verdict.outcome, VerdictOutcome::NotGuilty);
    }

    #[test]
    fn test_divergent_chains_guilty() {
        let mut court = Court::new(CourtConfig::default());

        let id = court
            .file_dispute(
                DisputeType::ExecutionDivergence {
                    proof_chain_hash: [0xAA; 32],
                },
                test_agent(),
                100,
            )
            .unwrap();

        let original = make_chain(100, [0xAA; 32], vec![([0u8; 32], [1u8; 32], 100)]);
        let replay = make_chain(
            100,
            [0xAA; 32],
            vec![
                ([0u8; 32], [2u8; 32], 100), // Different post_state
            ],
        );

        let verdict = court.adjudicate(id, &original, &replay, 105).unwrap();
        assert_eq!(verdict.outcome, VerdictOutcome::Guilty);
    }
}
