use serde::{Deserialize, Serialize};

pub type Hash = [u8; 32];
pub type ChainId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProofType {
    StateCommitment,
    ReceiptInclusion,
    IntentLock,
    SlashEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProofPayload {
    StateCommitment(Hash),
    ReceiptInclusion {
        receipt_hash: Hash,
        merkle_proof: Vec<Hash>,
    },
    IntentLock {
        intent_hash: Hash,
        resources: Hash,
    },
    SlashEvent {
        offender: [u8; 32],
        amount: u128,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FinalityProof {
    HotStuffQC {
        validator_set_hash: Hash,
        signatures: Vec<Vec<u8>>,
    },
    TendermintCommit {
        precommits: Vec<Vec<u8>>,
    },
    ZKProof {
        proof_data: Vec<u8>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossChainProof {
    pub source_chain: ChainId,
    pub block_hash: Hash,
    pub block_height: u64,
    pub proof_type: ProofType,
    pub payload: ProofPayload,
    pub finality_proof: FinalityProof,
}

pub struct ProofVerifier;

impl ProofVerifier {
    pub fn verify(proof: &CrossChainProof) -> Result<bool, &'static str> {
        // Implement Court VM on-chain client logic (analogous to IBC)
        // 1. Check finality proof based on chain ID
        let is_final = match &proof.finality_proof {
            FinalityProof::HotStuffQC { .. } => true, // verify QC signatures here
            FinalityProof::TendermintCommit { .. } => true,
            FinalityProof::ZKProof { .. } => true,
        };

        if !is_final {
            return Err("Invalid finality proof");
        }

        // 2. Process payload logic
        match proof.proof_type {
            ProofType::StateCommitment => Ok(true),
            ProofType::ReceiptInclusion => Ok(true),
            ProofType::IntentLock => Ok(true),
            ProofType::SlashEvent => Ok(true),
        }
    }
}
