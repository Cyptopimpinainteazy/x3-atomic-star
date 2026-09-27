//! Validator attestation set tracking with cryptographically verified quorum.
//!
//! `add_attestation` previously admitted any attestation whose `signature`
//! field was a non-empty byte string — a one-byte payload such as `vec![1]`
//! reached quorum with no cryptographic check at all. Attestations are now
//! verified with Ed25519 before they are admitted to the set: a rejected
//! attestation contributes no weight and is not stored, so quorum cannot be
//! reached with forged, truncated, zero-key, or mismatched-statement material.
//!
//! A valid signature only proves the caller controls *some* private key, not
//! that the key belongs to an authorized validator — a self-generated
//! keypair signs just as validly as a real validator's. `new` alone does not
//! check this; use [`AttestationSet::with_authorized_validators`] whenever
//! the caller has a real, governance-sourced validator set to check against.
//! (As of this writing `x3-relayer`'s SVM proof path — the one production
//! caller — does not have one wired in; see the tracked follow-up issue.)

use std::collections::{HashMap, HashSet};

use ed25519_dalek::{Signature, VerifyingKey};

/// Length of an Ed25519 public key, in bytes.
pub const PUBLIC_KEY_LEN: usize = 32;
/// Length of an Ed25519 signature, in bytes.
pub const SIGNATURE_LEN: usize = 64;

/// Numerator of the workspace's supermajority rule (see
/// [`supermajority_threshold`]).
pub const SUPERMAJORITY_NUMERATOR: u64 = 2;
/// Denominator of the workspace's supermajority rule (see
/// [`supermajority_threshold`]).
pub const SUPERMAJORITY_DENOMINATOR: u64 = 3;

/// The number of **distinct** validators that must attest for a validator set of
/// `total_validators` members to have reached a supermajority.
///
/// The rule is *strictly more than two thirds*: `floor(2 * n / 3) + 1`.
///
/// This is the single definition of the rule in the workspace. A producer that
/// decides how many signatures a proof must carry (the relayer) and a consumer
/// that decides whether an attestation set is enough (a verifier, or
/// [`AttestationSet::has_supermajority`]) must both call this function, so the
/// two can never drift into disagreeing about what "supermajority" means.
///
/// Fail-closed properties, both load-bearing:
///
/// * the result is never `0` — at least one distinct attestation is always
///   required, so an empty (or unconfigured) validator set can never reach
///   quorum by supplying zero signatures;
/// * it is monotonically non-decreasing in `total_validators`, so adding a
///   validator never lowers the bar.
///
/// `total_validators` above `u32::MAX * 3 / 2` saturates at `u32::MAX` rather
/// than wrapping.
pub fn supermajority_threshold(total_validators: usize) -> u32 {
    let total = total_validators as u128;
    let threshold =
        (SUPERMAJORITY_NUMERATOR as u128 * total) / SUPERMAJORITY_DENOMINATOR as u128 + 1;
    threshold.min(u32::MAX as u128) as u32
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ValidatorId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attestation {
    pub validator: ValidatorId,
    /// The statement being attested to.
    pub statement_hash: [u8; 32],
    /// Ed25519 public key of the signing validator.
    pub public_key: [u8; PUBLIC_KEY_LEN],
    /// Ed25519 signature over `statement_hash`.
    pub signature: Vec<u8>,
    /// Voting weight contributed once the attestation verifies.
    pub weight: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttestationError {
    EmptySignature,
    DuplicateValidator,
    InvalidSignatureLength {
        got: usize,
    },
    InvalidPublicKey,
    /// The attestation is for a different statement than this set aggregates.
    /// Without this check a caller can mix attestations from unrelated
    /// statements and still reach `has_quorum`.
    StatementMismatch,
    SignatureVerificationFailed,
    /// The public key verified the signature but is not in the configured
    /// set of authorized validators. Only returned when the set was built
    /// with [`AttestationSet::with_authorized_validators`].
    UnauthorizedValidator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttestationSet {
    statement_hash: [u8; 32],
    attestations: HashMap<ValidatorId, Attestation>,
    total_weight: u64,
    /// `None` means no authorization check is performed — any key that
    /// verifies is accepted. See the module-level warning about this.
    authorized_validators: Option<HashSet<[u8; PUBLIC_KEY_LEN]>>,
}

impl AttestationSet {
    pub fn new(statement_hash: [u8; 32]) -> Self {
        Self {
            statement_hash,
            attestations: HashMap::new(),
            total_weight: 0,
            authorized_validators: None,
        }
    }

    /// Like [`AttestationSet::new`], but every admitted attestation's public
    /// key must also be a member of `authorized_validators` — a signature
    /// from an unlisted (e.g. self-generated) key is rejected regardless of
    /// how cryptographically valid it is.
    pub fn with_authorized_validators(
        statement_hash: [u8; 32],
        authorized_validators: impl IntoIterator<Item = [u8; PUBLIC_KEY_LEN]>,
    ) -> Self {
        Self {
            statement_hash,
            attestations: HashMap::new(),
            total_weight: 0,
            authorized_validators: Some(authorized_validators.into_iter().collect()),
        }
    }

    /// Verify and admit an attestation. Rejected attestations contribute no
    /// weight and are not stored.
    pub fn add_attestation(&mut self, attestation: Attestation) -> Result<(), AttestationError> {
        if attestation.signature.is_empty() {
            return Err(AttestationError::EmptySignature);
        }

        if attestation.statement_hash != self.statement_hash {
            return Err(AttestationError::StatementMismatch);
        }

        if self.attestations.contains_key(&attestation.validator) {
            return Err(AttestationError::DuplicateValidator);
        }

        if attestation.public_key.iter().all(|byte| *byte == 0) {
            return Err(AttestationError::InvalidPublicKey);
        }

        if let Some(authorized) = &self.authorized_validators {
            if !authorized.contains(&attestation.public_key) {
                return Err(AttestationError::UnauthorizedValidator);
            }
        }

        let verifying_key = VerifyingKey::from_bytes(&attestation.public_key)
            .map_err(|_| AttestationError::InvalidPublicKey)?;

        if attestation.signature.len() != SIGNATURE_LEN {
            return Err(AttestationError::InvalidSignatureLength {
                got: attestation.signature.len(),
            });
        }
        let signature = Signature::from_slice(&attestation.signature).map_err(|_| {
            AttestationError::InvalidSignatureLength {
                got: attestation.signature.len(),
            }
        })?;

        // `verify_strict`, not `verify`: this signature may be checked
        // independently by other validators counting toward the same quorum,
        // and non-strict verification accepts some non-canonical signatures
        // (small-order components, non-canonical S) that strict verification
        // rejects — different verifiers could then disagree about whether the
        // same bytes are a valid signature.
        verifying_key
            .verify_strict(&attestation.statement_hash, &signature)
            .map_err(|_| AttestationError::SignatureVerificationFailed)?;

        self.total_weight = self.total_weight.saturating_add(attestation.weight);
        self.attestations
            .insert(attestation.validator.clone(), attestation);
        Ok(())
    }

    pub fn total_weight(&self) -> u64 {
        self.total_weight
    }

    pub fn unique_validators(&self) -> usize {
        self.attestations.len()
    }

    pub fn has_quorum(&self, required_weight: u64) -> bool {
        self.total_weight >= required_weight
    }

    /// Whether this set carries attestations from a supermajority of a
    /// `total_validators`-member validator set.
    ///
    /// Counts **distinct** validators (`unique_validators`), not raw signatures
    /// and not weight: two attestations from one validator are one validator,
    /// which is what stops a single signer from filling an N-of-M quorum by
    /// repeating itself. The bar itself comes from
    /// [`supermajority_threshold`], the workspace's single definition.
    pub fn has_supermajority(&self, total_validators: usize) -> bool {
        self.unique_validators() >= supermajority_threshold(total_validators) as usize
    }

    pub fn validators(&self) -> HashSet<ValidatorId> {
        self.attestations.keys().cloned().collect()
    }

    pub fn statement_hash(&self) -> [u8; 32] {
        self.statement_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn signed_attestation(name: &str, statement: [u8; 32], weight: u64, seed: u8) -> Attestation {
        let key = signing_key(seed);
        Attestation {
            validator: ValidatorId(name.to_string()),
            statement_hash: statement,
            public_key: key.verifying_key().to_bytes(),
            signature: key.sign(&statement).to_vec(),
            weight,
        }
    }

    #[test]
    fn accepts_valid_signature_and_counts_weight() {
        let mut set = AttestationSet::new([7; 32]);
        set.add_attestation(signed_attestation("alice", [7; 32], 40, 1))
            .unwrap();
        assert_eq!(set.total_weight(), 40);
        assert_eq!(set.unique_validators(), 1);
    }

    #[test]
    fn rejects_forged_signature() {
        // Signed by a different key than the one presented.
        let mut forged = signed_attestation("alice", [7; 32], 40, 2);
        forged.public_key = signing_key(3).verifying_key().to_bytes();

        let mut set = AttestationSet::new([7; 32]);
        assert_eq!(
            set.add_attestation(forged),
            Err(AttestationError::SignatureVerificationFailed)
        );
        assert_eq!(set.total_weight(), 0);
        assert!(!set.has_quorum(1));
    }

    #[test]
    fn rejects_tampered_statement_hash() {
        let mut tampered = signed_attestation("alice", [7; 32], 40, 4);
        tampered.statement_hash = [9; 32];

        let mut set = AttestationSet::new([7; 32]);
        assert_eq!(
            set.add_attestation(tampered),
            Err(AttestationError::StatementMismatch)
        );
        assert_eq!(set.total_weight(), 0);
    }

    #[test]
    fn rejects_short_signature() {
        let mut short = signed_attestation("alice", [7; 32], 40, 5);
        short.signature.truncate(SIGNATURE_LEN - 1);

        let mut set = AttestationSet::new([7; 32]);
        assert_eq!(
            set.add_attestation(short),
            Err(AttestationError::InvalidSignatureLength {
                got: SIGNATURE_LEN - 1
            })
        );
        assert_eq!(set.total_weight(), 0);
    }

    #[test]
    fn rejects_empty_signature() {
        let mut empty = signed_attestation("alice", [7; 32], 40, 6);
        empty.signature.clear();

        let mut set = AttestationSet::new([7; 32]);
        assert_eq!(
            set.add_attestation(empty),
            Err(AttestationError::EmptySignature)
        );
        assert_eq!(set.total_weight(), 0);
    }

    #[test]
    fn rejects_zero_public_key() {
        let mut zeroed = signed_attestation("alice", [7; 32], 40, 7);
        zeroed.public_key = [0u8; PUBLIC_KEY_LEN];

        let mut set = AttestationSet::new([7; 32]);
        assert_eq!(
            set.add_attestation(zeroed),
            Err(AttestationError::InvalidPublicKey)
        );
        assert_eq!(set.total_weight(), 0);
    }

    #[test]
    fn rejects_duplicate_validator() {
        let mut set = AttestationSet::new([7; 32]);
        set.add_attestation(signed_attestation("alice", [7; 32], 30, 8))
            .unwrap();
        let second = set.add_attestation(signed_attestation("alice", [7; 32], 20, 9));
        assert_eq!(second, Err(AttestationError::DuplicateValidator));
        assert_eq!(set.total_weight(), 30);
    }

    #[test]
    fn duplicate_validator_does_not_double_count_weight() {
        let mut set = AttestationSet::new([7; 32]);
        set.add_attestation(signed_attestation("alice", [7; 32], 30, 10))
            .unwrap();
        assert!(set
            .add_attestation(signed_attestation("alice", [7; 32], 30, 11))
            .is_err());

        assert_eq!(set.total_weight(), 30);
        assert_eq!(set.unique_validators(), 1);
    }

    #[test]
    fn with_authorized_validators_accepts_a_listed_key() {
        let key = signing_key(20);
        let mut set =
            AttestationSet::with_authorized_validators([7; 32], [key.verifying_key().to_bytes()]);
        set.add_attestation(signed_attestation("alice", [7; 32], 40, 20))
            .unwrap();
        assert_eq!(set.total_weight(), 40);
    }

    #[test]
    fn with_authorized_validators_rejects_a_self_generated_key() {
        // A perfectly valid signature from a key nobody authorized — this is
        // exactly what `new` alone cannot catch.
        let mut set = AttestationSet::with_authorized_validators(
            [7; 32],
            [signing_key(21).verifying_key().to_bytes()],
        );
        assert_eq!(
            set.add_attestation(signed_attestation("mallory", [7; 32], 1000, 22)),
            Err(AttestationError::UnauthorizedValidator)
        );
        assert_eq!(set.total_weight(), 0);
        assert!(!set.has_quorum(1));
    }

    #[test]
    fn computes_weight_and_quorum_correctly() {
        let mut set = AttestationSet::new([7; 32]);
        set.add_attestation(signed_attestation("alice", [7; 32], 40, 12))
            .unwrap();
        set.add_attestation(signed_attestation("bob", [7; 32], 35, 13))
            .unwrap();

        assert_eq!(set.total_weight(), 75);
        assert!(set.has_quorum(67));
        assert!(!set.has_quorum(80));
    }

    #[test]
    fn supermajority_is_strictly_more_than_two_thirds() {
        // floor(2n/3) + 1 is the smallest k with k > 2n/3.
        assert_eq!(supermajority_threshold(0), 1);
        assert_eq!(supermajority_threshold(1), 1);
        assert_eq!(supermajority_threshold(2), 2);
        assert_eq!(supermajority_threshold(3), 3);
        assert_eq!(supermajority_threshold(4), 3);
        assert_eq!(supermajority_threshold(5), 4);
        assert_eq!(supermajority_threshold(6), 5);
        assert_eq!(supermajority_threshold(7), 5);
        assert_eq!(supermajority_threshold(9), 7);
        // The canonical 67-of-100 case.
        assert_eq!(supermajority_threshold(100), 67);
    }

    /// The rule must never return 0, or an unconfigured validator set would
    /// reach quorum with no signatures at all.
    #[test]
    fn supermajority_never_admits_an_empty_set() {
        for total in 0..64usize {
            assert!(
                supermajority_threshold(total) >= 1,
                "a {total}-member set must still require at least one signature"
            );
        }
        assert_eq!(supermajority_threshold(0), 1);
    }

    #[test]
    fn supermajority_is_monotonic_in_the_set_size() {
        for total in 1..256usize {
            assert!(
                supermajority_threshold(total) >= supermajority_threshold(total - 1),
                "adding a validator must never lower the threshold ({total})"
            );
        }
    }

    #[test]
    fn has_supermajority_counts_distinct_validators_from_the_single_rule() {
        let mut set = AttestationSet::new([7; 32]);
        assert!(!set.has_supermajority(3), "no attestations is not a quorum");

        set.add_attestation(signed_attestation("alice", [7; 32], 1, 30))
            .unwrap();
        // One of three is not more than two thirds of three.
        assert!(!set.has_supermajority(3));

        set.add_attestation(signed_attestation("bob", [7; 32], 1, 31))
            .unwrap();
        // Two of three is still not *more* than two thirds.
        assert!(!set.has_supermajority(3));

        set.add_attestation(signed_attestation("carol", [7; 32], 1, 32))
            .unwrap();
        assert!(set.has_supermajority(3));
        assert_eq!(supermajority_threshold(3), 3);
    }
}
