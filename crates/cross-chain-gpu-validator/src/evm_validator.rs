//! EVM state root validation pipeline with GPU batching

use crate::error::{Result, ValidatorError};
use crate::kernels::Keccak256Kernel;
use crate::ValidationResult;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct EvmStateRoot {
    pub block_number: u64,
    pub state_root: Vec<u8>,
    pub transactions: Vec<Vec<u8>>,
}

/// EVM state root validator using GPU-batched keccak256
pub struct EvmValidator {
    pub hasher: Keccak256Kernel,
}

impl EvmValidator {
    pub fn new(batch_size: usize, use_gpu: bool) -> Self {
        Self {
            hasher: Keccak256Kernel::new(batch_size, use_gpu),
        }
    }

    /// Validate an EVM state root by computing root hash and comparing
    pub async fn validate_state_root(&self, evm_state: &EvmStateRoot) -> Result<ValidationResult> {
        let start = Instant::now();

        // Build transaction merkle tree
        let tx_bytes: Vec<&[u8]> = evm_state
            .transactions
            .iter()
            .map(|t| t.as_slice())
            .collect();

        if tx_bytes.is_empty() {
            return Ok(ValidationResult {
                valid: false,
                error: Some("No transactions in block".to_string()),
                duration_ms: 0,
            });
        }

        // Hash all transactions
        let (tx_hashes, _) = self.hasher.hash_batch_gpu(&tx_bytes)?;

        // Compute merkle root iteratively
        let computed_root = self.compute_merkle_root(&tx_hashes)?;

        let duration = start.elapsed().as_millis() as u64;
        let valid = computed_root == evm_state.state_root;

        Ok(ValidationResult {
            valid,
            error: if valid {
                None
            } else {
                Some(format!(
                    "State root mismatch: expected {}, computed {}",
                    hex::encode(&evm_state.state_root),
                    hex::encode(&computed_root)
                ))
            },
            duration_ms: duration,
        })
    }

    /// Compute merkle root from leaf hashes
    fn compute_merkle_root(&self, leaves: &[Vec<u8>]) -> Result<Vec<u8>> {
        if leaves.is_empty() {
            return Err(ValidatorError::EvmValidationFailed(
                "Cannot compute root of empty tree".to_string(),
            ));
        }

        let mut current_level = leaves.to_vec();

        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            for chunk in current_level.chunks(2) {
                let mut combined = chunk[0].clone();
                if chunk.len() == 2 {
                    combined.extend_from_slice(&chunk[1]);
                } else {
                    combined.extend_from_slice(&chunk[0]);
                }

                let inputs = vec![combined.as_slice()];
                let (hashes, _) = self.hasher.hash_batch_cpu(&inputs)?;
                next_level.push(hashes[0].clone());
            }

            current_level = next_level;
        }

        Ok(current_level[0].clone())
    }

    /// Validate a batch of EVM state roots
    pub async fn validate_batch(&self, states: &[EvmStateRoot]) -> Result<Vec<ValidationResult>> {
        let mut results = Vec::new();
        for state in states {
            results.push(self.validate_state_root(state).await?);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_evm_state_validation_empty() {
        let validator = EvmValidator::new(32, false);
        let state = EvmStateRoot {
            block_number: 1,
            state_root: vec![0; 32],
            transactions: vec![],
        };

        let result = validator.validate_state_root(&state).await.unwrap();
        assert!(!result.valid);
    }

    #[tokio::test]
    async fn test_evm_state_validation_single_tx() {
        let validator = EvmValidator::new(32, false);

        // Create a single transaction
        let tx = b"test_transaction".to_vec();
        let tx_bytes = vec![tx.as_slice()];

        // Compute expected root
        let (hashes, _) = validator.hasher.hash_batch_cpu(&tx_bytes).unwrap();
        let expected_root = hashes[0].clone();

        let state = EvmStateRoot {
            block_number: 1,
            state_root: expected_root,
            transactions: vec![tx],
        };

        let result = validator.validate_state_root(&state).await.unwrap();
        assert!(result.valid);
    }

    #[tokio::test]
    async fn test_evm_merkle_tree_computation() {
        let validator = EvmValidator::new(32, false);

        let leaves = vec![vec![1u8; 32], vec![2u8; 32], vec![3u8; 32], vec![4u8; 32]];

        let root = validator.compute_merkle_root(&leaves).unwrap();
        assert_eq!(root.len(), 32);
    }
}
