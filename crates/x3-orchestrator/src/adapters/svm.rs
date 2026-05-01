//! SVM (Solana-style) chain adapter (mock implementation).

use crate::{ChainAdapter, ChainId, CrossVmMessage, ExecutionProof, OrchestratorError, Result};

pub struct SvmAdapter {
    pub id: ChainId,
}

impl SvmAdapter {
    pub fn new(id: ChainId) -> Self {
        Self { id }
    }
}

impl ChainAdapter for SvmAdapter {
    fn chain_id(&self) -> ChainId {
        self.id.clone()
    }

    fn send(&self, msg: &CrossVmMessage) -> Result<String> {
        msg.id()
    }

    fn verify(&self, proof: &ExecutionProof) -> Result<bool> {
        if proof.proof_bytes.is_empty() {
            return Err(OrchestratorError::InvalidProof);
        }
        Ok(true)
    }

    fn execute(&self, _msg: &CrossVmMessage) -> Result<()> {
        Ok(())
    }
}
