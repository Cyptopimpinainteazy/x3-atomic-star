//! The authorized validator set a cross-chain proof's attestation quorum is
//! derived from — and the single place the relayer turns configured keys into a
//! required-signature count.
//!
//! Before this module the relayer carried the count as a literal: `submitter.rs`
//! constructed every `SvmProof` with `required_signatures: 1` ("quorum
//! enforcement belongs at the aggregator layer") and no aggregator exists in
//! this workspace. A one-signature proof is therefore trivially satisfied by the
//! one signature the submitter attaches, and the proof's own
//! `required_signatures` field — which the *counterparty* supplies — is what the
//! safety pipeline checks against. A swap's safety rested on a number the proof
//! chose for itself.
//!
//! The count now comes from the configured set and from one rule, the
//! workspace's [`x3_validator_attestation::supermajority_threshold`]. Nothing
//! here invents a second notion of "enough": if a proof is going to be produced,
//! this module says how many *distinct* signatures it must carry, and the same
//! function says how many the verifiers demand.

use std::collections::BTreeSet;

use thiserror::Error;
use x3_validator_attestation::{supermajority_threshold, PUBLIC_KEY_LEN};

use crate::types::ValidatorSignature;

/// A configured validator set that cannot be used, named rather than silently
/// truncated to an empty set.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ValidatorSetError {
    #[error("validator public key `{entry}` is not valid hexadecimal: {reason}")]
    MalformedHex { entry: String, reason: String },

    #[error("validator public key `{entry}` is {got} bytes, expected {PUBLIC_KEY_LEN}")]
    WrongLength { entry: String, got: usize },

    /// A key listed twice would inflate the set, and therefore the
    /// supermajority derived from it, while contributing one real signer.
    #[error("validator public key `{entry}` is configured more than once")]
    DuplicateKey { entry: String },
}

/// The validator set authorized to attest external-chain finality, together with
/// the quorum its size implies.
///
/// The quorum is *derived*, never stored: [`AuthorizedValidatorSet::required_signatures`]
/// recomputes it from the set through the shared supermajority rule, so a set
/// cannot be paired with a threshold that does not belong to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedValidatorSet {
    keys: Vec<[u8; PUBLIC_KEY_LEN]>,
}

impl AuthorizedValidatorSet {
    /// Wrap already-decoded keys.
    ///
    /// The caller is responsible for having deduplicated them; use
    /// [`AuthorizedValidatorSet::from_hex_keys`] for operator-supplied
    /// configuration, which refuses duplicates.
    pub fn from_keys(keys: Vec<[u8; PUBLIC_KEY_LEN]>) -> Self {
        Self { keys }
    }

    /// Decode the hex-encoded keys of a `validator_set` config section.
    ///
    /// An empty list is accepted here — it is the fail-closed default, and the
    /// refusal happens where a proof would be produced or accepted, not at
    /// startup. Anything malformed, wrongly sized or repeated is an error: a
    /// set the operator did not mean is refused rather than silently trimmed.
    pub fn from_hex_keys(entries: &[String]) -> Result<Self, ValidatorSetError> {
        let mut keys: Vec<[u8; PUBLIC_KEY_LEN]> = Vec::with_capacity(entries.len());
        for entry in entries {
            let trimmed = entry.trim();
            let trimmed = trimmed.strip_prefix("0x").unwrap_or(trimmed);
            let bytes = hex::decode(trimmed).map_err(|err| ValidatorSetError::MalformedHex {
                entry: entry.clone(),
                reason: err.to_string(),
            })?;
            if bytes.len() != PUBLIC_KEY_LEN {
                return Err(ValidatorSetError::WrongLength {
                    entry: entry.clone(),
                    got: bytes.len(),
                });
            }
            let mut key = [0u8; PUBLIC_KEY_LEN];
            key.copy_from_slice(&bytes);
            if keys.contains(&key) {
                return Err(ValidatorSetError::DuplicateKey {
                    entry: entry.clone(),
                });
            }
            keys.push(key);
        }
        Ok(Self { keys })
    }

    pub fn keys(&self) -> &[[u8; PUBLIC_KEY_LEN]] {
        &self.keys
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn is_authorized(&self, key: &[u8; PUBLIC_KEY_LEN]) -> bool {
        self.keys.iter().any(|known| known == key)
    }

    /// The number of **distinct authorized** validators that signed.
    ///
    /// A key that is not in the set contributes nothing, and a key that appears
    /// more than once is counted once — the same rule the verifiers apply, so a
    /// producer cannot satisfy a quorum by repeating one signature.
    pub fn distinct_signers(&self, signatures: &[ValidatorSignature]) -> usize {
        let mut counted: BTreeSet<[u8; PUBLIC_KEY_LEN]> = BTreeSet::new();
        for signature in signatures {
            if self.is_authorized(&signature.validator_pubkey) {
                counted.insert(signature.validator_pubkey);
            }
        }
        counted.len()
    }

    /// How many distinct signatures a proof over this set must carry.
    ///
    /// `floor(2n/3) + 1` for an `n`-key set — [`supermajority_threshold`], the
    /// workspace's single definition. At least `1`, so an empty set can never be
    /// satisfied by zero signatures.
    pub fn required_signatures(&self) -> u32 {
        supermajority_threshold(self.keys.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_hex(seed: u8) -> String {
        hex::encode([seed; PUBLIC_KEY_LEN])
    }

    fn signature(pubkey: [u8; PUBLIC_KEY_LEN]) -> ValidatorSignature {
        ValidatorSignature {
            validator_pubkey: pubkey,
            signature: [0u8; 64],
        }
    }

    #[test]
    fn the_quorum_is_the_supermajority_of_the_set_size() {
        let one = AuthorizedValidatorSet::from_hex_keys(&[key_hex(1)]).unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one.required_signatures(), supermajority_threshold(1));
        assert_eq!(one.required_signatures(), 1);

        let three =
            AuthorizedValidatorSet::from_hex_keys(&[key_hex(1), key_hex(2), key_hex(3)]).unwrap();
        assert_eq!(three.required_signatures(), supermajority_threshold(3));
        assert_eq!(three.required_signatures(), 3);

        let nine = AuthorizedValidatorSet::from_keys(vec![[7u8; PUBLIC_KEY_LEN]; 9]);
        assert_eq!(nine.required_signatures(), supermajority_threshold(9));
        assert_eq!(nine.required_signatures(), 7);
    }

    #[test]
    fn an_empty_set_still_requires_one_signature() {
        let empty = AuthorizedValidatorSet::from_hex_keys(&[]).unwrap();
        assert!(empty.is_empty());
        // Fail closed: zero signatures must never be a quorum.
        assert_eq!(empty.required_signatures(), 1);
    }

    #[test]
    fn accepts_an_optional_0x_prefix() {
        let prefixed = AuthorizedValidatorSet::from_hex_keys(&[format!("0x{}", key_hex(4))]);
        let bare = AuthorizedValidatorSet::from_hex_keys(&[key_hex(4)]);
        assert_eq!(prefixed.unwrap(), bare.unwrap());
    }

    #[test]
    fn a_malformed_key_is_named_not_trimmed() {
        let err = AuthorizedValidatorSet::from_hex_keys(&["zz".to_string()]).unwrap_err();
        assert!(
            matches!(err, ValidatorSetError::MalformedHex { .. }),
            "got {err:?}"
        );
        assert!(err.to_string().contains("zz"));
    }

    #[test]
    fn a_wrong_length_key_is_named() {
        let err = AuthorizedValidatorSet::from_hex_keys(&[hex::encode([1u8; 16])]).unwrap_err();
        assert_eq!(
            err,
            ValidatorSetError::WrongLength {
                entry: hex::encode([1u8; 16]),
                got: 16,
            }
        );
    }

    /// A key listed twice would inflate the set size, and with it the
    /// supermajority, while contributing one real signer.
    #[test]
    fn a_duplicated_key_is_refused_and_cannot_inflate_the_set() {
        let err = AuthorizedValidatorSet::from_hex_keys(&[key_hex(5), key_hex(5)]).unwrap_err();
        assert_eq!(err, ValidatorSetError::DuplicateKey { entry: key_hex(5) });
    }

    #[test]
    fn distinct_signers_ignores_unauthorized_and_repeated_keys() {
        let set =
            AuthorizedValidatorSet::from_keys(vec![[1u8; PUBLIC_KEY_LEN], [2u8; PUBLIC_KEY_LEN]]);
        assert_eq!(set.distinct_signers(&[]), 0);
        // A key nobody authorized does not count.
        assert_eq!(set.distinct_signers(&[signature([9u8; PUBLIC_KEY_LEN])]), 0);
        // One authorized key repeated twice counts once.
        assert_eq!(
            set.distinct_signers(&[
                signature([1u8; PUBLIC_KEY_LEN]),
                signature([1u8; PUBLIC_KEY_LEN]),
            ]),
            1
        );
        assert_eq!(
            set.distinct_signers(&[
                signature([1u8; PUBLIC_KEY_LEN]),
                signature([2u8; PUBLIC_KEY_LEN]),
            ]),
            2
        );
    }
}
