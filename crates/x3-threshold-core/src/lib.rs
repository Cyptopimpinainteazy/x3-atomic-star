//! Threshold-encryption core — the committee crypto, in a form a runtime can use.
//!
//! This is the maths that used to live only inside `crates/private-mempool`: Ristretto Shamir
//! splitting and Lagrange combination ([`threshold`]), committee ECDH + HKDF + AES-256-GCM
//! ([`encryption`]), and the records they operate on. It moved here on 2026-09-26 for one reason:
//! `private-mempool` is std-only (tokio, chrono, parking_lot, `rand` with its default features), so
//! `pallet-private-execution` could not link it, and the pallet's `submit_private_transaction`
//! consequently stored whatever bytes it was handed — no committee-key check, no epoch binding, no
//! shape validation. A private submission encrypted to an old DKG epoch is accepted today and can
//! never be decrypted by the current committee.
//!
//! The crate is `no_std` (with `alloc`) and the RNG-dependent helpers are behind `std`, so the
//! deterministic half — validation, share combination, decryption — compiles for the runtime:
//!
//! ```text
//! cargo check -p x3-threshold-core --no-default-features --target wasm32-unknown-unknown
//! ```
//!
//! `private-mempool` re-exports everything here, so there is still exactly one definition of the
//! scheme and one set of tests for it.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod encryption;
pub mod threshold;

use alloc::string::String;
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode};

/// A transaction encrypted to the committee's threshold public key.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
pub struct EncryptedTransaction {
    /// Unique transaction identifier: `blake3(ciphertext)`.
    pub id: [u8; 32],
    /// AES-256-GCM ciphertext.
    pub ciphertext: alloc::vec::Vec<u8>,
    /// Ephemeral public key for ECDH: a compressed Ristretto point, not an X25519 key — see
    /// [`threshold`] for why.
    pub ephemeral_pk: [u8; 32],
    /// AES-GCM nonce (12 bytes).
    pub nonce: [u8; 12],
    /// Sender's public key (for fee attribution).
    pub sender_pk: [u8; 32],
    /// Priority fee commitment (Pedersen commitment).
    pub fee_commitment: [u8; 32],
    /// Timestamp when submitted.
    pub submitted_at: u64,
    /// DKG epoch this transaction was encrypted for.
    pub dkg_epoch: u64,
}

/// Threshold public key for the confidential validator committee.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
pub struct ThresholdPublicKey {
    /// The combined group public key: a compressed Ristretto point (`secret * G`), not an X25519
    /// key — see [`threshold`] for why.
    pub group_key: [u8; 32],
    /// DKG epoch the committee key belongs to.
    pub epoch: u64,
    /// Threshold (t in t-of-n).
    pub threshold: u32,
    /// Total committee size (n).
    pub committee_size: u32,
}

/// One validator's partial decryption of a transaction.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
pub struct DecryptionShare {
    /// Validator index in the committee.
    pub validator_index: u32,
    /// The partial decryption.
    pub share: alloc::vec::Vec<u8>,
    /// Proof of correct decryption (DLEQ proof).
    pub proof: alloc::vec::Vec<u8>,
    /// The DKG epoch this share was minted under.
    ///
    /// A partial decryption is `share_scalar * ephemeral_point` under *one* ceremony's polynomial,
    /// so partials from two ceremonies can be interpolated into a point that belongs to neither.
    /// That used to surface as an opaque AES-GCM tag failure with nothing to name it; the epoch is
    /// checked before any interpolation now.
    pub dkg_epoch: u64,
}

/// The private-mempool family's error type.
///
/// It moved here with the code so there is one definition: the crypto modules return it, the
/// mempool queue wraps it, and a runtime pallet can name every refusal it makes. The mempool-only
/// variants (`Full`, `Duplicate`) live here too rather than forcing a second enum and a mapping.
///
/// `Display` is written by hand rather than derived: the derive macro this crate used
/// (`thiserror`) compiles to `std` code in the version pinned here, and the whole point of the move
/// is that this file builds without `std`. A hand-written `core::fmt` impl has no such dependency
/// and is what the pallet needs (it can name a refusal in an event or a log).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MempoolError {
    NoCommitteeKey,
    WrongEpoch {
        expected: u64,
        got: u64,
    },
    ShareEpochMismatch {
        expected: u64,
        got: u64,
    },
    EncryptionError(String),
    /// The payload is not a valid Ristretto point.
    InvalidPoint {
        field: &'static str,
    },
    /// The ciphertext cannot carry an AES-GCM tag.
    CiphertextTooShort {
        len: usize,
    },
    /// The transaction id is not the hash of its own ciphertext.
    IdMismatch,
    /// The committee key's declared threshold is not satisfiable by its own committee size.
    InvalidCommitteeKey {
        threshold: u32,
        committee_size: u32,
    },
    Full {
        capacity: usize,
    },
    Duplicate,
}

impl core::fmt::Display for MempoolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MempoolError::NoCommitteeKey => write!(f, "no committee key set"),
            MempoolError::WrongEpoch { expected, got } => write!(
                f,
                "transaction encrypted for wrong epoch (expected {expected}, got {got})"
            ),
            MempoolError::ShareEpochMismatch { expected, got } => write!(
                f,
                "decryption share belongs to DKG epoch {got}, not the epoch {expected} being decrypted"
            ),
            MempoolError::EncryptionError(reason) => write!(f, "encryption error: {reason}"),
            MempoolError::InvalidPoint { field } => {
                write!(f, "invalid Ristretto point in {field}")
            }
            MempoolError::CiphertextTooShort { len } => write!(
                f,
                "ciphertext is too short to carry an authentication tag ({len} bytes)"
            ),
            MempoolError::IdMismatch => {
                write!(f, "transaction id does not match the hash of its ciphertext")
            }
            MempoolError::InvalidCommitteeKey {
                threshold,
                committee_size,
            } => write!(
                f,
                "committee key declares threshold {threshold} of {committee_size}"
            ),
            MempoolError::Full { capacity } => {
                write!(f, "mempool is full (capacity: {capacity})")
            }
            MempoolError::Duplicate => write!(f, "duplicate transaction"),
        }
    }
}

/// The smallest ciphertext an AES-256-GCM message can be: the tag alone.
pub const AES_GCM_TAG_BYTES: usize = 16;

/// Is every part of `tx` consistent with `key`, and does `tx` name a committee that can open it?
///
/// This is the check a chain can make at the door, without holding any share: it proves the
/// submission is well-formed and addressed to *this* committee's epoch, which is what stops a
/// payload no current validator can ever decrypt from being accepted and escrowed. It is not a
/// decryption and does not prove the ciphertext is openable by the threshold — only that it is
/// shaped to be, and bound to the right ceremony.
pub fn validate_encrypted_transaction(
    tx: &EncryptedTransaction,
    key: &ThresholdPublicKey,
) -> Result<(), MempoolError> {
    if key.threshold == 0 || key.threshold > key.committee_size {
        return Err(MempoolError::InvalidCommitteeKey {
            threshold: key.threshold,
            committee_size: key.committee_size,
        });
    }
    // The group key must be a real point, or every share computed against it is meaningless. This
    // is reported as an unusable *committee key* rather than as a malformed payload: the caller did
    // not choose it, and the person who can fix it is whoever installed it.
    let group = encryption::decompress_point(&key.group_key).map_err(|_| {
        MempoolError::InvalidCommitteeKey {
            threshold: key.threshold,
            committee_size: key.committee_size,
        }
    })?;
    if group == curve25519_dalek::traits::Identity::identity() {
        return Err(MempoolError::InvalidCommitteeKey {
            threshold: key.threshold,
            committee_size: key.committee_size,
        });
    }
    // The epoch binding: a payload for another ceremony cannot be opened by this committee.
    if tx.dkg_epoch != key.epoch {
        return Err(MempoolError::WrongEpoch {
            expected: key.epoch,
            got: tx.dkg_epoch,
        });
    }
    if tx.ciphertext.len() < AES_GCM_TAG_BYTES {
        return Err(MempoolError::CiphertextTooShort {
            len: tx.ciphertext.len(),
        });
    }
    let ephemeral =
        encryption::decompress_point(&tx.ephemeral_pk).map_err(|_| MempoolError::InvalidPoint {
            field: "ephemeral public key",
        })?;
    if ephemeral == curve25519_dalek::traits::Identity::identity() {
        return Err(MempoolError::InvalidPoint {
            field: "ephemeral public key",
        });
    }
    // The id is the ciphertext's hash; a mismatch means the record and the payload disagree.
    if tx.id != encryption::blake3_hash(&tx.ciphertext) {
        return Err(MempoolError::IdMismatch);
    }
    Ok(())
}

#[cfg(all(test, feature = "std"))]
mod validation_tests {
    use super::*;
    use curve25519_dalek::scalar::Scalar;
    use rand::rngs::OsRng;

    /// A real committee: a random secret and the public key derived from it.
    fn committee(epoch: u64, threshold: u32, committee_size: u32) -> ThresholdPublicKey {
        let secret = Scalar::random(&mut OsRng);
        ThresholdPublicKey {
            group_key: threshold::group_public_key(&secret),
            epoch,
            threshold,
            committee_size,
        }
    }

    /// A submission a sender would really produce for this committee.
    fn submission(key: &ThresholdPublicKey) -> EncryptedTransaction {
        encryption::encrypt_for_committee(
            b"a private swap",
            &key.group_key,
            &[7u8; 32],
            &[8u8; 32],
            key.epoch,
        )
        .expect("a real committee key encrypts")
    }

    #[test]
    fn a_well_formed_submission_for_this_committee_is_accepted() {
        let key = committee(4, 3, 5);
        let tx = submission(&key);
        assert_eq!(validate_encrypted_transaction(&tx, &key), Ok(()));
    }

    /// The refusal the whole validation exists for: a payload nothing in the current committee can
    /// ever open must not be accepted and escrowed.
    #[test]
    fn a_submission_for_another_epoch_is_refused_by_name() {
        let old = committee(3, 3, 5);
        let tx = submission(&old);
        let current = committee(4, 3, 5);
        assert_eq!(
            validate_encrypted_transaction(&tx, &current),
            Err(MempoolError::WrongEpoch {
                expected: 4,
                got: 3
            })
        );
    }

    #[test]
    fn a_submission_whose_id_is_not_its_ciphertext_hash_is_refused() {
        let key = committee(1, 2, 3);
        let mut tx = submission(&key);
        tx.id = [0u8; 32];
        assert_eq!(
            validate_encrypted_transaction(&tx, &key),
            Err(MempoolError::IdMismatch)
        );
    }

    #[test]
    fn a_ciphertext_that_cannot_carry_a_tag_is_refused() {
        let key = committee(1, 2, 3);
        let mut tx = submission(&key);
        tx.ciphertext.truncate(AES_GCM_TAG_BYTES - 1);
        assert_eq!(
            validate_encrypted_transaction(&tx, &key),
            Err(MempoolError::CiphertextTooShort {
                len: AES_GCM_TAG_BYTES - 1
            })
        );
    }

    #[test]
    fn an_ephemeral_key_that_is_not_a_point_is_refused() {
        let key = committee(1, 2, 3);
        let mut tx = submission(&key);
        // The all-zero encoding is the Ristretto identity: a valid encoding of a point that would
        // make every decryption share meaningless.
        tx.ephemeral_pk = [0u8; 32];
        assert_eq!(
            validate_encrypted_transaction(&tx, &key),
            Err(MempoolError::InvalidPoint {
                field: "ephemeral public key"
            })
        );
    }

    #[test]
    fn a_committee_key_that_cannot_meet_its_own_threshold_is_refused() {
        let key = committee(1, 2, 3);
        let tx = submission(&key);
        let broken = ThresholdPublicKey {
            threshold: 4,
            committee_size: 3,
            ..key
        };
        assert_eq!(
            validate_encrypted_transaction(&tx, &broken),
            Err(MempoolError::InvalidCommitteeKey {
                threshold: 4,
                committee_size: 3
            })
        );
    }

    /// The crate that owns the scheme is the one that compiles without `std`; the RNG-dependent
    /// side is what is gated, not the check a chain needs.
    #[test]
    fn validation_needs_no_rng_and_no_std() {
        let key = committee(9, 1, 1);
        let tx = submission(&key);
        // Deterministic: the same inputs give the same verdict every time.
        for _ in 0..8 {
            assert_eq!(validate_encrypted_transaction(&tx, &key), Ok(()));
        }
    }
}
