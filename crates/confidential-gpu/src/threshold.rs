//! Threshold DKG (Distributed Key Generation) manager.
//!
//! Implements Pedersen DKG for threshold decryption:
//! - Each validator generates polynomial commitments
//! - Validators exchange shares
//! - Group public key is derived without any single party knowing the secret
//!
//! # Invariant: PRIV-EXEC-003
//! No single validator can reconstruct the decryption key.

use crate::ConfidentialGpuError;
use curve25519_dalek::{constants::RISTRETTO_BASEPOINT_POINT, scalar::Scalar};
use rand::rngs::OsRng;

/// A DKG commitment broadcast by a validator.
#[derive(Debug, Clone)]
pub struct DkgCommitment {
    /// Validator index.
    pub validator_index: u32,
    /// Feldman coefficient commitments (compressed Ristretto points).
    pub commitments: Vec<[u8; 32]>,
}

/// A DKG share sent to a specific validator.
#[derive(Debug, Clone)]
pub struct DkgShare {
    /// Source validator index.
    pub from: u32,
    /// Destination validator index.
    pub to: u32,
    /// Share value.
    pub share: [u8; 32],
    /// Proof of correct sharing.
    pub proof: Vec<u8>,
}

use sha2::{Digest, Sha256};

/// Manages the DKG ceremony for this validator.
pub struct DkgManager {
    validator_index: u32,
    threshold: u32,
    committee_size: u32,
    /// The ceremony this manager's shares belong to.
    ///
    /// Recorded on every [`private_mempool::DecryptionShare`] this manager
    /// produces, and required of every share it combines, so a share from an
    /// earlier ceremony is refused with a named error instead of being
    /// interpolated into a point that belongs to no committee.
    dkg_epoch: u64,
    /// Our secret polynomial coefficients.
    secret_coefficients: Vec<Scalar>,
    /// Received shares from other validators.
    received_shares: Vec<DkgShare>,
    /// Public commitments keyed by zero-based validator ID.
    peer_commitments: std::collections::HashMap<u32, DkgCommitment>,
    /// Our aggregated secret share.
    aggregated_secret: Option<[u8; 32]>,
    /// The group public key.
    group_key: Option<[u8; 32]>,
    complete: bool,
}

impl DkgManager {
    pub fn new(validator_index: u32, threshold: u32, committee_size: u32, dkg_epoch: u64) -> Self {
        Self {
            validator_index,
            threshold,
            committee_size,
            dkg_epoch,
            secret_coefficients: Vec::new(),
            received_shares: Vec::new(),
            peer_commitments: std::collections::HashMap::new(),
            aggregated_secret: None,
            group_key: None,
            complete: false,
        }
    }

    /// Generate a fresh random polynomial and real Feldman commitments.
    pub fn generate_commitments(&mut self) -> DkgCommitment {
        if self.threshold == 0
            || self.threshold > self.committee_size
            || self.validator_index >= self.committee_size
        {
            self.secret_coefficients.clear();
            return DkgCommitment {
                validator_index: self.validator_index,
                commitments: Vec::new(),
            };
        }
        let mut rng = OsRng;
        self.secret_coefficients = (0..self.threshold)
            .map(|_| Scalar::random(&mut rng))
            .collect();
        let commitments = self
            .secret_coefficients
            .iter()
            .map(|c| (c * RISTRETTO_BASEPOINT_POINT).compress().to_bytes())
            .collect();
        DkgCommitment {
            validator_index: self.validator_index,
            commitments,
        }
    }

    /// Generate a recipient-specific Shamir share. External validator IDs are
    /// zero-based; field evaluation points are ID+1 so x=0 remains the secret.
    pub fn share_for(&self, recipient: u32) -> Result<DkgShare, ConfidentialGpuError> {
        if recipient >= self.committee_size
            || self.secret_coefficients.len() != self.threshold as usize
        {
            return Err(ConfidentialGpuError::DkgFailed(
                "invalid recipient or DKG polynomial not generated".into(),
            ));
        }
        let scalar = private_mempool::threshold::evaluate_polynomial(
            &self.secret_coefficients,
            recipient + 1,
        );
        let share = scalar.to_bytes();
        let mut proof_hasher = Sha256::new();
        proof_hasher.update(b"x3/confidential-gpu/feldman-share/v1");
        proof_hasher.update(self.validator_index.to_le_bytes());
        proof_hasher.update(recipient.to_le_bytes());
        proof_hasher.update(share);
        Ok(DkgShare {
            from: self.validator_index,
            to: recipient,
            share,
            proof: proof_hasher.finalize().to_vec(),
        })
    }

    /// Process a complete commitment set and return this validator's own share.
    /// The group key is the sum of constant-term Feldman commitments.
    pub fn participate(
        &mut self,
        peer_commitments: &[DkgCommitment],
    ) -> Result<DkgShare, ConfidentialGpuError> {
        use curve25519_dalek::{
            ristretto::{CompressedRistretto, RistrettoPoint},
            traits::Identity,
        };
        use std::collections::HashSet;

        if self.secret_coefficients.is_empty() {
            let generated = self.generate_commitments();
            if generated.commitments.is_empty() {
                return Err(ConfidentialGpuError::DkgFailed(
                    "invalid DKG configuration".into(),
                ));
            }
        }
        if peer_commitments.len() != self.committee_size as usize {
            return Err(ConfidentialGpuError::DkgFailed(format!(
                "expected {} commitment sets but got {}",
                self.committee_size,
                peer_commitments.len()
            )));
        }

        let mut seen = HashSet::new();
        let mut group = RistrettoPoint::identity();
        for commitment in peer_commitments {
            if commitment.validator_index >= self.committee_size
                || !seen.insert(commitment.validator_index)
            {
                return Err(ConfidentialGpuError::DkgFailed(
                    "duplicate or out-of-range commitment validator".into(),
                ));
            }
            if commitment.commitments.len() != self.threshold as usize {
                return Err(ConfidentialGpuError::DkgFailed(
                    "commitment polynomial has wrong degree".into(),
                ));
            }
            let c0 = CompressedRistretto(commitment.commitments[0])
                .decompress()
                .ok_or_else(|| {
                    ConfidentialGpuError::DkgFailed("invalid Ristretto commitment".into())
                })?;
            group += c0;
        }
        if group == RistrettoPoint::identity() {
            return Err(ConfidentialGpuError::DkgFailed(
                "group key is identity".into(),
            ));
        }
        self.group_key = Some(group.compress().to_bytes());
        self.peer_commitments = peer_commitments
            .iter()
            .map(|c| (c.validator_index, c.clone()))
            .collect();
        self.complete = false;
        self.share_for(self.validator_index)
    }

    /// Verify a private share against the sender's Feldman commitments and
    /// aggregate it only if it is addressed to this validator.
    pub fn accept_share(&mut self, share: DkgShare) -> Result<(), ConfidentialGpuError> {
        use curve25519_dalek::{
            ristretto::{CompressedRistretto, RistrettoPoint},
            traits::Identity,
        };
        if share.to != self.validator_index || share.from >= self.committee_size {
            return Err(ConfidentialGpuError::DkgFailed(
                "share has invalid sender or recipient".into(),
            ));
        }
        if self.received_shares.iter().any(|s| s.from == share.from) {
            return Err(ConfidentialGpuError::DkgFailed(
                "duplicate DKG share".into(),
            ));
        }
        let commitment = self.peer_commitments.get(&share.from).ok_or_else(|| {
            ConfidentialGpuError::DkgFailed("share sender has no registered commitment".into())
        })?;
        let scalar = Option::<Scalar>::from(Scalar::from_canonical_bytes(share.share))
            .ok_or_else(|| ConfidentialGpuError::DkgFailed("non-canonical DKG scalar".into()))?;
        let x = Scalar::from((self.validator_index + 1) as u64);
        let mut expected = RistrettoPoint::identity();
        let mut power = Scalar::ONE;
        for bytes in &commitment.commitments {
            let point = CompressedRistretto(*bytes).decompress().ok_or_else(|| {
                ConfidentialGpuError::DkgFailed("invalid Ristretto commitment".into())
            })?;
            expected += power * point;
            power *= x;
        }
        if scalar * RISTRETTO_BASEPOINT_POINT != expected {
            return Err(ConfidentialGpuError::DkgFailed(
                "share failed Feldman verification".into(),
            ));
        }
        self.received_shares.push(share);
        if self.received_shares.len() == self.committee_size as usize {
            let mut aggregate = Scalar::ZERO;
            for accepted in &self.received_shares {
                let scalar = Option::<Scalar>::from(Scalar::from_canonical_bytes(accepted.share))
                    .ok_or_else(|| {
                    ConfidentialGpuError::DkgFailed("non-canonical DKG scalar".into())
                })?;
                aggregate += scalar;
            }
            self.aggregated_secret = Some(aggregate.to_bytes());
            self.complete = true;
        }
        Ok(())
    }

    /// Combine decryption shares to reconstruct the shared secret.
    ///
    /// # Invariant: PRIV-EXEC-003
    pub fn combine_decryption_shares(
        &self,
        shares: &[private_mempool::DecryptionShare],
    ) -> Result<[u8; 32], ConfidentialGpuError> {
        private_mempool::encryption::combine_shares(shares, self.threshold, self.dkg_epoch)
            .map_err(|e| ConfidentialGpuError::DkgFailed(e.to_string()))
    }

    /// Get the group public key.
    pub fn group_key(&self) -> Option<[u8; 32]> {
        self.group_key
    }

    /// Check if DKG is complete.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// The ceremony this manager's shares belong to.
    pub fn dkg_epoch(&self) -> u64 {
        self.dkg_epoch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dkg_ceremony_flow() {
        // 3-of-5 DKG
        let mut validators: Vec<DkgManager> = (0..5).map(|i| DkgManager::new(i, 3, 5, 1)).collect();

        // Phase 1: Generate commitments
        let commitments: Vec<DkgCommitment> = validators
            .iter_mut()
            .map(|v| v.generate_commitments())
            .collect();

        // Phase 2: Exchange shares
        for v in &mut validators {
            let _share = v.participate(&commitments).unwrap();
        }

        let group_key = validators[0].group_key();
        for v in &validators {
            assert_eq!(v.group_key(), group_key);
        }

        let a = validators[0].share_for(1).unwrap();
        let b = validators[0].share_for(2).unwrap();
        assert_ne!(a.share, b.share);

        // Deliver and verify every sender's private share to every recipient.
        for recipient in 0..5usize {
            let incoming: Vec<_> = (0..5usize)
                .map(|sender| validators[sender].share_for(recipient as u32).unwrap())
                .collect();
            for share in incoming {
                validators[recipient].accept_share(share).unwrap();
            }
        }
        assert!(validators.iter().all(DkgManager::is_complete));
    }

    /// # Invariant: PRIV-EXEC-003
    #[test]
    fn combine_requires_threshold() {
        let mgr = DkgManager::new(0, 3, 5, 1);

        let shares = vec![private_mempool::DecryptionShare {
            validator_index: 0,
            share: vec![0x01; 32],
            proof: vec![],
            dkg_epoch: 1,
        }];

        // Only 1 share, need 3
        let result = mgr.combine_decryption_shares(&shares);
        assert!(result.is_err());
    }

    /// # Invariant: PRIV-EXEC-003
    #[test]
    fn a_share_from_another_ceremony_is_refused_by_name() {
        let mgr = DkgManager::new(0, 3, 5, 7);
        assert_eq!(mgr.dkg_epoch(), 7);

        let shares: Vec<private_mempool::DecryptionShare> = (0..3)
            .map(|index| private_mempool::DecryptionShare {
                validator_index: index + 1,
                share: vec![0x01; 32],
                proof: vec![],
                dkg_epoch: 6, // the ceremony before this one
            })
            .collect();

        let err = mgr.combine_decryption_shares(&shares).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("epoch 7") && message.contains("epoch 6"),
            "a share from an earlier ceremony must be refused by naming both epochs, got: {message}"
        );
    }
}
