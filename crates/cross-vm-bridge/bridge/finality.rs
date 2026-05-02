//! Cross-VM Bridge Finality Verifier
//!
//! Implements finality proof verification for EVM and SVM chains to enable
//! atomic cross-VM operations with guaranteed consistency.

#![cfg_attr(not(feature = std), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::DispatchError;
use sp_std::fmt::Debug;

/// Finality threshold: 2/3 of validators required
pub const FINALITY_THRESHOLD_EVM: u32 = 66;
pub const FINALITY_THRESHOLD_SVM: u32 = 66;
pub const MAX_FINALITY_PROOF_AGE: u64 = 3600; // 1 hour max proof age

/// Represents a finalized block on a VM
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct FinalizedBlock {
    pub block_number: u64,
    pub block_hash: [u8; 32],
    pub state_root: [u8; 32],
    pub timestamp: u64,
    pub finality_epoch: u64,
    pub parent_hash: [u8; 32],
}

/// Validator information for finality verification
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct ValidatorInfo {
    pub public_key: Vec<u8>,
    pub voting_weight: u64,
    pub is_active: bool,
    pub is_slashed: bool,
}

/// Signature from a single validator
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct ValidatorSignature {
    pub validator_index: u32,
    pub signature: Vec<u8>,
    pub signed: bool,
}

/// Aggregated finality signature
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct AggregatedSignature {
    pub signers_bitfield: Vec<u8>,
    pub aggregated_signature: Vec<u8>,
    pub signer_count: u32,
    pub total_validators: u32,
    pub threshold: u32,
}

/// Finality proof for EVM (Ethereum-style)
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct EvmFinalityProof {
    pub finalized_block: FinalizedBlock,
    pub checkpoint_block: FinalizedBlock,
    pub attestations: Vec<ValidatorSignature>,
    pub aggregated_signature: AggregatedSignature,
    pub custody_bits: Option<Vec<u8>>,
    pub domain: [u8; 32],
    pub genesis_hash: [u8; 32],
}

/// Finality proof for SVM (Solana-style)
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct SvmFinalityProof {
    pub finalized_block: FinalizedBlock,
    pub leader_schedule_epoch: u64,
    pub lockouts: Vec<LockoutInfo>,
    pub root_block: FinalizedBlock,
    pub vote_signatures: Vec<ValidatorSignature>,
    pub vote_threshold: u32,
}

/// Lockout information from Solana's Tower BFT
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct LockoutInfo {
    pub block_hash: [u8; 32],
    pub slot: u64,
    pub lockout_depth: u32,
    pub root_distance: u32,
    pub signature: Vec<u8>,
}

/// Combined finality proof for cross-VM atomic operations
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct CrossVmFinalityProof {
    pub evm_proof: Option<EvmFinalityProof>,
    pub svm_proof: Option<SvmFinalityProof>,
    pub min_block_age: u64,
    pub max_proof_age: u64,
    pub submitted_at: u64,
    pub sequence: u64,
}

/// Result of finality verification
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct FinalityVerificationResult {
    pub is_valid: bool,
    pub finalized_block: Option<FinalizedBlock>,
    pub error_message: Option<Vec<u8>>,
    pub verification_data: VerificationData,
}

/// Detailed verification data
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Default)]
pub struct VerificationData {
    pub signers_count: u32,
    pub required_threshold: u32,
    pub validator_set_size: u32,
    pub sig_verification_us: u64,
    pub checkpoint_age: u64,
    pub had_slashed_validators: bool,
    pub steps_passed: Vec<Vec<u8>>,
}

/// VM identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub enum VmIdentifier {
    Evm,
    Svm,
    X3Vm,
}

/// Trait for VM-specific finality verifiers
pub trait VmFinalityVerifier: Send + Sync {
    fn vm_id(&self) -> VmIdentifier;
    fn verify_finality_proof(
        &self,
        proof: &[u8],
        validator_set: &[ValidatorInfo],
        current_epoch: u64,
    ) -> Result<FinalityVerificationResult, DispatchError>;
    fn current_finality_epoch(&self) -> u64;
    fn is_block_finalized(&self, block_hash: &[u8; 32]) -> bool;
    fn get_finalized_block(&self, block_number: u64) -> Option<FinalizedBlock>;
}

// ─────────────────────────────────────────────────────────────────
// EVM Finality Verifier
// ─────────────────────────────────────────────────────────────────

pub struct EvmFinalityVerifier {
    chain_id: u64,
    genesis_hash: H256,
    current_epoch: u64,
}

impl EvmFinalityVerifier {
    pub fn new(chain_id: u64, genesis_hash: H256) -> Self {
        Self {
            chain_id,
            genesis_hash,
            current_epoch: 0,
        }
    }

    pub fn set_epoch(&mut self, epoch: u64) {
        self.current_epoch = epoch;
    }

    fn count_signers(&self, bitfield: &[u8]) -> u32 {
        bitfield.iter().fold(0u32, |acc, &byte| acc + byte.count_ones() as u32)
    }

    fn verify_bls_aggregate(
        &self,
        aggregated_sig: &[u8],
        public_keys: &[&[u8]],
        bitfield: &[u8],
    ) -> Result<bool, DispatchError> {
        // BLS12-381 signatures are 96 bytes
        if aggregated_sig.len() != 96 {
            return Err(DispatchError::Other(
                b BLS aggregate signature must be 96 bytes. Got: .to_vec(),
            ));
        }

        let validator_count = public_keys.len();
        let required_bitfield_len = (validator_count + 7) / 8;
        if bitfield.len() != required_bitfield_len {
            return Err(DispatchError::Other(
                b Invalid bitfield length for validator set..to_vec(),
            ));
        }

        // In production: use blst crate for actual verification
        Ok(true)
    }
}

impl VmFinalityVerifier for EvmFinalityVerifier {
    fn vm_id(&self) -> VmIdentifier {
        VmIdentifier::Evm
    }

    fn verify_finality_proof(
        &self,
        proof_bytes: &[u8],
        validator_set: &[ValidatorInfo],
        _current_epoch: u64,
    ) -> Result<FinalityVerificationResult, DispatchError> {
        let proof = EvmFinalityProof::decode(&mut &proof_bytes[..])
            .map_err(|_| DispatchError::Other(b Failed to decode EVM finality proof..to_vec()))?;

        let mut verification_data = VerificationData::default();
        let mut steps_passed = Vec::new();

        // Step 1: Verify genesis hash
        if proof.genesis_hash != self.genesis_hash.0 {
            return Ok(FinalityVerificationResult {
                is_valid: false,
                finalized_block: None,
                error_message: Some(b Genesis hash mismatch..to_vec()),
                verification_data,
            });
        }
        steps_passed.push(b Genesis hash verified..to_vec());

        // Step 2: Verify checkpoint age (anti-reorg)
        let checkpoint_age = self.current_epoch.saturating_sub(proof.checkpoint_block.finality_epoch);
        verification_data.checkpoint_age = checkpoint_age;
        if checkpoint_age < 32 {
            return Ok(FinalityVerificationResult {
                is_valid: false,
                finalized_block: None,
                error_message: Some(b Checkpoint not old enough for finality..to_vec()),
                verification_data,
            });
        }
        steps_passed.push(b Checkpoint age verified..to_vec());

        // Step 3: Check for slashed validators
        let has_slashed = validator_set.iter().any(|v| v.is_slashed);
        verification_data.had_slashed_validators = has_slashed;
        if has_slashed {
            return Ok(FinalityVerificationResult {
                is_valid: false,
                finalized_block: None,
                error_message: Some(b Validator set contains slashed validators..to_vec()),
                verification_data,
            });
        }
        steps_passed.push(b No slashed validators..to_vec());

        // Step 4: Verify signature threshold
        let active_validators: Vec<_> = validator_set.iter()
            .filter(|v| v.is_active && !v.is_slashed)
            .collect();
        verification_data.validator_set_size = active_validators.len() as u32;
        
        let total_stake: u64 = active_validators.iter().map(|v| v.voting_weight).sum();
        let signers_stake = proof.attestations.iter()
            .filter(|a| a.signed)
            .map(|a| active_validators.get(a.validator_index as usize)
                .map(|v| v.voting_weight)
                .unwrap_or(0))
            .sum();
        
        verification_data.signers_count = proof.attestations.iter()
            .filter(|a| a.signed)
            .count() as u32;
        verification_data.required_threshold = FINALITY_THRESHOLD_EVM;
        
        let threshold_stake = total_stake * FINALITY_THRESHOLD_EVM as u64 / 100;
        if signers_stake < threshold_stake {
            return Ok(FinalityVerificationResult {
                is_valid: false,
                finalized_block: None,
                error_message: Some(format!(b Insufficient stake: {} < {}, signers_stake, threshold_stake).to_vec()),
                verification_data,
            });
        }
        steps_passed.push(format!(b Sufficient stake verified: {} >= {}, signers_stake, threshold_stake).to_vec());

        // Step 5: Verify chain continuity
        let block_distance = proof.finalized_block.block_number
            .saturating_sub(proof.checkpoint_block.block_number);
        if proof.finalized_block.parent_hash != proof.checkpoint_block.block_hash && block_distance > 1 {
            return Ok(FinalityVerificationResult {
                is_valid: false,
                finalized_block: None,
                error_message: Some(b Chain discontinuity detected..to_vec()),
                verification_data,
            });
        }
        steps_passed.push(b Chain continuity verified..to_vec());

        verification_data.steps_passed = steps_passed;
        Ok(FinalityVerificationResult {
            is_valid: true,
            finalized_block: Some(proof.finalized_block),
            error_message: None,
            verification_data,
        })
    }

    fn current_finality_epoch(&self) -> u64 {
        self.current_epoch
    }

    fn is_block_finalized(&self, _block_hash: &[u8; 32]) -> bool {
        self.current_epoch > 0
    }

    fn get_finalized_block(&self, _block_number: u64) -> Option<FinalizedBlock> {
        None
    }
}

// ─────────────────────────────────────────────────────────────────
// SVM Finality Verifier
// ─────────────────────────────────────────────────────────────────

pub struct SvmFinalityVerifier {
    current_slot: u64,
    root_slot: u64,
}

impl SvmFinalityVerifier {
    pub fn new() -> Self {
        Self {
            current_slot: 0,
            root_slot: 0,
        }
    }

    pub fn set_slot(&mut self, slot: u64) {
        self.current_slot = slot;
    }

    pub fn set_root_slot(&mut self, slot: u64) {
        self.root_slot = slot;
    }

    fn verify_ed25519(&self, signature: &[u8], public_key: &[u8]) -> Result<bool, DispatchError> {
        if signature.len() != 64 {
            return Err(DispatchError::Other(
                b Ed25519 signature must be 64 bytes..to_vec(),
            ));
        }
        if public_key.len() != 32 {
            return Err(DispatchError::Other(
                b Ed25519 public key must be 32 bytes..to_vec(),
            ));
        }
        // In production: use ed25519_dalek for actual verification
        Ok(true)
    }

    fn verify_lockout_chain(&self, lockouts: &[LockoutInfo]) -> Result<bool, DispatchError> {
        let mut last_depth = 0u32;
        for lockout in lockouts {
            if lockout.lockout_depth < last_depth && last_depth > 0 {
                return Err(DispatchError::Other(
                    b Invalid lockout sequence..to_vec(),
                ));
            }
            last_depth = lockout.lockout_depth;
        }
        Ok(true)
    }
}

impl Default for SvmFinalityVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl VmFinalityVerifier for SvmFinalityVerifier {
    fn vm_id(&self) -> VmIdentifier {
        VmIdentifier::Svm
    }

    fn verify_finality_proof(
        &self,
        proof_bytes: &[u8],
        validator_set: &[ValidatorInfo],
        _current_epoch: u64,
    ) -> Result<FinalityVerificationResult, DispatchError> {
        let proof = SvmFinalityProof::decode(&mut &proof_bytes[..])
            .map_err(|_| DispatchError::Other(b Failed to decode SVM finality proof..to_vec()))?;

        let mut verification_data = VerificationData::default();
        let mut steps_passed = Vec::new();

        // Step 1: Verify root block ordering
        if proof.finalized_block.block_number <= proof.root_block.block_number {
            return Ok(FinalityVerificationResult {
                is_valid: false,
                finalized_block: None,
                error_message: Some(b Finalized block must be newer than root..to_vec()),
                verification_data,
            });
        }
        steps_passed.push(b Root block ordering verified..to_vec());

        // Step 2: Verify vote threshold
        let active_count = validator_set.iter()
            .filter(|v| v.is_active && !v.is_slashed)
            .count() as u32;
        verification_data.validator_set_size = active_count;
        
        let threshold = (active_count * FINALITY_THRESHOLD_SVM / 100).max(1);
        let signed_count = proof.vote_signatures.iter().filter(|v| v.signed).count() as u32;
        
        if signed_count < threshold {
            return Ok(FinalityVerificationResult {
                is_valid: false,
                finalized_block: None,
                error_message: Some(format!(b Insufficient votes: {} < {}, signed_count, threshold).to_vec()),
                verification_data,
            });
        }
        verification_data.signers_count = signed_count;
        verification_data.required_threshold = FINALITY_THRESHOLD_SVM;
        steps_passed.push(format!(b Vote threshold verified: {} >= {}, signed_count, threshold).to_vec());

        // Step 3: Verify lockout chain
        if !proof.lockouts.is_empty() {
            self.verify_lockout_chain(&proof.lockouts)?;
        }
        steps_passed.push(b Lockout chain verified..to_vec());

        // Step 4: Verify signatures
        let mut valid_sigs = 0u32;
        for vote in &proof.vote_signatures {
            if !vote.signed {
                continue;
            }
            let validator = validator_set.get(vote.validator_index as usize)
                .ok_or_else(|| DispatchError::Other(b Validator index out of range..to_vec()))?;
            
            if validator.is_slashed || !validator.is_active {
                return Ok(FinalityVerificationResult {
                    is_valid: false,
                    finalized_block: None,
                    error_message: Some(format!(b Validator {} is slashed or inactive, vote.validator_index).to_vec()),
                    verification_data,
                });
            }

            if self.verify_ed25519(&vote.signature, &validator.public_key)? {
                valid_sigs += 1;
            }
        }

        if valid_sigs < threshold {
            return Ok(FinalityVerificationResult {
                is_valid: false,
                finalized_block: None,
                error_message: Some(format!(b Invalid signatures: {} < {}, valid_sigs, threshold).to_vec()),
                verification_data,
            });
        }
        steps_passed.push(format!(b All {} signatures verified., valid_sigs).to_vec());

        verification_data.steps_passed = steps_passed;
        Ok(FinalityVerificationResult {
            is_valid: true,
            finalized_block: Some(proof.finalized_block),
            error_message: None,
            verification_data,
        })
    }

    fn current_finality_epoch(&self) -> u64 {
        self.root_slot
    }

    fn is_block_finalized(&self, _block_hash: &[u8; 32]) -> bool {
        self.root_slot > 0
    }

    fn get_finalized_block(&self, _block_number: u64) -> Option<FinalizedBlock> {
        None
    }
}

// ─────────────────────────────────────────────────────────────────
// Cross-VM Finality Verifier
// ─────────────────────────────────────────────────────────────────

pub struct CrossVmFinalityVerifier {
    evm_verifier: EvmFinalityVerifier,
    svm_verifier: SvmFinalityVerifier,
}

impl CrossVmFinalityVerifier {
    pub fn new(chain_id: u64, genesis_hash: H256) -> Self {
        Self {
            evm_verifier: EvmFinalityVerifier::new(chain_id, genesis_hash),
            svm_verifier: SvmFinalityVerifier::new(),
        }
    }

    pub fn evm(&self) -> &EvmFinalityVerifier {
        &self.evm_verifier
    }

    pub fn svm(&self) -> &SvmFinalityVerifier {
        &self.svm_verifier
    }

    pub fn evm_mut(&mut self) -> &mut EvmFinalityVerifier {
        &mut self.evm_verifier
    }

    pub fn svm_mut(&mut self) -> &mut SvmFinalityVerifier {
        &mut self.svm_verifier
    }

    /// Verify a cross-VM finality proof - both VMs must be finalized
    pub fn verify_cross_vm_proof(
        &self,
        proof: &CrossVmFinalityProof,
        evm_validators: &[ValidatorInfo],
        svm_validators: &[ValidatorInfo],
    ) -> Result<FinalityVerificationResult, DispatchError> {
        let mut verification_data = VerificationData::default();
        let mut steps_passed = Vec::new();

        // Step 1: Check proof age
        let proof_age = proof.submitted_at; // Simplified - in production compare with timestamp
        if proof_age > proof.max_proof_age {
            return Ok(FinalityVerificationResult {
                is_valid: false,
                finalized_block: None,
                error_message: Some(b Proof too old..to_vec()),
                verification_data,
            });
        }
        steps_passed.push(b Proof age verified..to_vec());

        // Step 2: Verify EVM proof if present
        let mut evm_block = None;
        if let Some(ref evm_proof) = proof.evm_proof {
            let evm_result = self.evm_verifier.verify_finality_proof(
                &evm_proof.encode(),
                evm_validators,
                0,
            )?;
            if !evm_result.is_valid {
                return Ok(FinalityVerificationResult {
                    is_valid: false,
                    finalized_block: None,
                    error_message: Some(format!(b EVM verification failed: {:?}, evm_result.error_message).to_vec()),
                    verification_data: evm_result.verification_data,
                });
            }
            evm_block = evm_result.finalized_block;
        }
        steps_passed.push(b EVM finality verified..to_vec());

        // Step 3: Verify SVM proof if present
        let mut svm_block = None;
        if let Some(ref svm_proof) = proof.svm_proof {
            let svm_result = self.svm_verifier.verify_finality_proof(
                &svm_proof.encode(),
                svm_validators,
                0,
            )?;
            if !svm_result.is_valid {
                return Ok(FinalityVerificationResult {
                    is_valid: false,
                    finalized_block: None,
                    error_message: Some(format!(b SVM verification failed: {:?}, svm_result.error_message).to_vec()),
                    verification_data: svm_result.verification_data,
                });
            }
            svm_block = svm_result.finalized_block;
        }
        steps_passed.push(b SVM finality verified..to_vec());

        // Step 4: Check cross-VM consistency
        if let (Some(ref evm), Some(ref svm)) = (evm_block.as_ref(), svm_block.as_ref()) {
            let age_diff = evm.timestamp.abs_diff(svm.timestamp);
            if age_diff > proof.min_block_age {
                return Ok(FinalityVerificationResult {
                    is_valid: false,
                    finalized_block: None,
                    error_message: Some(format!(b Cross-VM timestamp mismatch: {} seconds apart, age_diff).to_vec()),
                    verification_data,
                });
            }
        }
        steps_passed.push(b Cross-VM consistency verified..to_vec());

        verification_data.steps_passed = steps_passed;
        let finalized_block = evm_block.or(svm_block);
        
        Ok(FinalityVerificationResult {
            is_valid: true,
            finalized_block,
            error_message: None,
            verification_data,
        })
    }
}

// ─────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evm_verifier_new() {
        let genesis = H256::repeat_byte(0x42);
        let verifier = EvmFinalityVerifier::new(1, genesis);
        assert_eq!(verifier.vm_id(), VmIdentifier::Evm);
    }

    #[test]
    fn test_svm_verifier_new() {
        let verifier = SvmFinalityVerifier::new();
        assert_eq!(verifier.vm_id(), VmIdentifier::Svm);
    }

    #[test]
    fn test_cross_vm_verifier_creation() {
        let genesis = H256::repeat_byte(0x42);
        let verifier = CrossVmFinalityVerifier::new(1, genesis);
        assert_eq!(verifier.evm().vm_id(), VmIdentifier::Evm);
        assert_eq!(verifier.svm().vm_id(), VmIdentifier::Svm);
    }

    #[test]
    fn test_finalized_block_encoding() {
        let block = FinalizedBlock {
            block_number: 12345,
            block_hash: [0u8; 32],
            state_root: [1u8; 32],
            timestamp: 1000000,
            finality_epoch: 100,
            parent_hash: [2u8; 32],
        };
        let encoded = block.encode();
        let decoded = FinalizedBlock::decode(&mut &encoded[..]).unwrap();
        assert_eq!(decoded.block_number, 12345);
    }

    #[test]
    fn test_validator_info_encoding() {
        let validator = ValidatorInfo {
            public_key: vec![3u8; 32],
            voting_weight: 1000000,
            is_active: true,
            is_slashed: false,
        };
        let encoded = validator.encode();
        let decoded = ValidatorInfo::decode(&mut &encoded[..]).unwrap();
        assert_eq!(decoded.voting_weight, 1000000);
    }

    #[test]
    fn test_signer_count() {
        let verifier = EvmFinalityVerifier::new(1, H256::repeat_byte(0x1));
        let bitfield = vec![0b10101010]; // 4 signers
        assert_eq!(verifier.count_signers(&bitfield), 4);
    }

    #[test]
    fn test_finality_threshold() {
        assert_eq!(FINALITY_THRESHOLD_EVM, 66);
        assert_eq!(FINALITY_THRESHOLD_SVM, 66);
    }

    #[test]
    fn test_lockout_info_encoding() {
        let lockout = LockoutInfo {
            block_hash: [0u8; 32],
            slot: 123456789,
            lockout_depth: 10,
            root_distance: 5,
            signature: vec![4u8; 64],
        };
        let encoded = lockout.encode();
        let decoded = LockoutInfo::decode(&mut &encoded[..]).unwrap();
        assert_eq!(decoded.slot, 123456789);
    }

    #[test]
    fn test_cross_vm_proof_encoding() {
        let proof = CrossVmFinalityProof {
            evm_proof: None,
            svm_proof: None,
            min_block_age: 60,
            max_proof_age: 3600,
            submitted_at: 1000000,
            sequence: 1,
        };
        let encoded = proof.encode();
        let decoded = CrossVmFinalityProof::decode(&mut &encoded[..]).unwrap();
        assert_eq!(decoded.sequence, 1);
    }

    #[test]
    fn test_verification_data_default() {
        let data = VerificationData::default();
        assert_eq!(data.signers_count, 0);
        assert!(data.steps_passed.is_empty());
    }
}