//! Solana VM state validation pipeline

use crate::error::Result;
use crate::ValidationResult;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct SvmState {
    pub slot: u64,
    pub block_hash: Vec<u8>,
    pub transactions: Vec<Vec<u8>>,
}

/// Solana validator for transaction verification and state validation
pub struct SvmValidator;

impl Default for SvmValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl SvmValidator {
    pub fn new() -> Self {
        Self
    }

    /// Validate Solana transactions for a given slot
    pub async fn validate_transactions(&self, svm_state: &SvmState) -> Result<ValidationResult> {
        let start = Instant::now();

        if svm_state.transactions.is_empty() {
            return Ok(ValidationResult {
                valid: false,
                error: Some("No transactions in slot".to_string()),
                duration_ms: 0,
            });
        }

        // Validate each transaction structure
        for tx in &svm_state.transactions {
            if tx.is_empty() {
                let duration = start.elapsed().as_millis() as u64;
                return Ok(ValidationResult {
                    valid: false,
                    error: Some("Invalid transaction format".to_string()),
                    duration_ms: duration,
                });
            }
        }

        let duration = start.elapsed().as_millis() as u64;
        Ok(ValidationResult {
            valid: true,
            error: None,
            duration_ms: duration,
        })
    }

    /// Validate Solana block hash consistency
    pub async fn validate_block_hash(&self, svm_state: &SvmState) -> Result<ValidationResult> {
        let start = Instant::now();

        // Block hash should be 32 bytes (SHA-256)
        if svm_state.block_hash.len() != 32 {
            let duration = start.elapsed().as_millis() as u64;
            return Ok(ValidationResult {
                valid: false,
                error: Some("Invalid block hash length".to_string()),
                duration_ms: duration,
            });
        }

        let duration = start.elapsed().as_millis() as u64;
        Ok(ValidationResult {
            valid: true,
            error: None,
            duration_ms: duration,
        })
    }

    /// Validate a batch of Solana states
    pub async fn validate_batch(&self, states: &[SvmState]) -> Result<Vec<ValidationResult>> {
        let mut results = Vec::new();
        for state in states {
            let tx_result = self.validate_transactions(state).await?;
            if tx_result.valid {
                results.push(self.validate_block_hash(state).await?);
            } else {
                results.push(tx_result);
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_svm_validation_empty() {
        let validator = SvmValidator::new();
        let state = SvmState {
            slot: 1,
            block_hash: vec![0; 32],
            transactions: vec![],
        };

        let result = validator.validate_transactions(&state).await.unwrap();
        assert!(!result.valid);
    }

    #[tokio::test]
    async fn test_svm_validation_valid_tx() {
        let validator = SvmValidator::new();
        let state = SvmState {
            slot: 1,
            block_hash: vec![1u8; 32],
            transactions: vec![vec![1, 2, 3, 4]],
        };

        let result = validator.validate_transactions(&state).await.unwrap();
        assert!(result.valid);
    }

    #[tokio::test]
    async fn test_svm_block_hash_validation() {
        let validator = SvmValidator::new();

        let state_valid = SvmState {
            slot: 1,
            block_hash: vec![1u8; 32],
            transactions: vec![],
        };

        let result = validator.validate_block_hash(&state_valid).await.unwrap();
        assert!(result.valid);

        let state_invalid = SvmState {
            slot: 1,
            block_hash: vec![1u8; 16], // Wrong length
            transactions: vec![],
        };

        let result = validator.validate_block_hash(&state_invalid).await.unwrap();
        assert!(!result.valid);
    }
}
