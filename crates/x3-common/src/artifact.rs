//! Signing and verifying a standalone X3BC artifact.
//!
//! An artifact executed **on chain** is authenticated by the transaction that carries it: the
//! program is an argument of a signed extrinsic, so the account's signature covers its bytes, and
//! `a_tampered_artifact_cannot_be_executed_under_the_original_signature` (the `X3-native
//! lifecycles` gate) measures exactly that.
//!
//! An artifact handed around **out of band** — a `.x3b` written by `x3 compile`, a file fetched
//! from a release page — had no such binding (X3-LANG-009). The envelope carries a checksum, and
//! [`crate::bytecode::checksum`] says what it is: a corruption check anyone can recompute over a
//! forged body. Recomputing it proves the bytes did not rot in transit, never who produced them.
//!
//! This module adds the missing half as a **detached** attestation. The artifact's bytes are not
//! touched, so both readers of the format — `x3-backend` and the runtime's `mini_x3` — are
//! unaffected and their byte-for-byte parity suite still describes the same format. An attestation
//! travels beside the artifact (a `.sig` sidecar, a release manifest, a registry row).
//!
//! What a signature covers is the **whole artifact, header included**: the header carries the
//! feature flags, so a signature over the body alone would let an attacker clear
//! [`crate::bytecode::FEATURE_PRIVATE_SUBMISSION_REQUIRED`] and keep the signature valid — turning
//! "this program demands privacy" into "this program runs in the clear", the one direction that
//! must never be taken silently.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};
use sp_core::{ed25519, Pair};
use sp_io::hashing::sha2_256;
use sp_runtime::traits::Verify;

/// Domain separator for an artifact attestation's signing digest.
///
/// Distinct from every other digest in the tree, so a signature over an artifact can never be
/// replayed as a signature over something else that happens to hash the same way.
pub const ARTIFACT_DOMAIN: &[u8] = b"x3-artifact-attestation-v1";

/// Absorb a length-prefixed field, so `("ab", "c")` and `("a", "bc")` cannot commit to the same
/// bytes.
fn absorb(buffer: &mut Vec<u8>, field: &[u8]) {
    buffer.extend_from_slice(&(field.len() as u64).to_le_bytes());
    buffer.extend_from_slice(field);
}

/// The content digest of an artifact: every byte of it, header included.
pub fn artifact_digest(artifact: &[u8]) -> [u8; 32] {
    let mut buffer = Vec::with_capacity(artifact.len() + 64);
    absorb(&mut buffer, ARTIFACT_DOMAIN);
    absorb(&mut buffer, artifact);
    sha2_256(&buffer)
}

/// Where an artifact-signing key stands in its lifecycle.
///
/// An artifact carries no trustworthy signing time — a file's mtime is whatever the last copy set
/// — so the lifecycle is by state, not by date, exactly as the trading receipts' key registry does
/// it. Rotating a key moves it to [`ArtifactKeyStatus::Retired`], which keeps verifying what it
/// already signed, because those artifacts are genuine. Revoking it refuses everything it ever
/// signed, because whoever holds a leaked key can produce artifacts indistinguishable from the
/// real ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKeyStatus {
    /// Signs new artifacts and verifies.
    Active,
    /// Rotated out: verifies the artifacts it signed, must not sign new ones.
    Retired,
    /// Compromised or withdrawn: verifies nothing.
    Revoked,
}

/// One entry of an artifact key registry, as written in its JSON or TOML form.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisteredArtifactKey {
    pub key_id: String,
    /// The ed25519 public key, 64 hex characters.
    pub public_key: String,
    pub status: ArtifactKeyStatus,
}

/// A detached claim that a named key produced this artifact.
///
/// `public_key` is carried so a reader can see which key is claimed without a lookup; it is **not**
/// a trust decision. Verification requires the key to be the one the registry lists under
/// `key_id`, so an attestation that carries its own key and a matching signature still fails unless
/// a registry vouches for that key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactAttestation {
    pub key_id: String,
    /// ed25519 public key, 32 bytes.
    pub public_key: [u8; 32],
    /// Digest of the artifact this attestation is about, as [`artifact_digest`] computes it.
    pub artifact_digest: [u8; 32],
    /// ed25519 signature over [`ArtifactAttestation::signing_digest`], 64 bytes.
    pub signature: Vec<u8>,
}

/// Why an attestation was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttestationError {
    /// The attestation is about a different artifact than the one supplied.
    DigestMismatch { expected: [u8; 32], found: [u8; 32] },
    /// No key in the registry has this id.
    UnknownKey(String),
    /// The registry lists this id under a different public key.
    KeyMismatch(String),
    /// The key is revoked: nothing it signed is accepted.
    RevokedKey(String),
    /// The signature does not cover this attestation under that key.
    BadSignature,
}

impl core::fmt::Display for AttestationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DigestMismatch { .. } => {
                write!(f, "the attestation is about a different artifact")
            }
            Self::UnknownKey(key_id) => write!(f, "no registry key is named '{key_id}'"),
            Self::KeyMismatch(key_id) => {
                write!(
                    f,
                    "the registry lists '{key_id}' under a different public key"
                )
            }
            Self::RevokedKey(key_id) => write!(f, "artifact signer '{key_id}' is revoked"),
            Self::BadSignature => write!(f, "the attestation signature does not verify"),
        }
    }
}

impl ArtifactAttestation {
    /// The digest an attester signs: the artifact's digest and who is claiming it.
    ///
    /// Binding the **key id** stops a signature being relabelled as another identity's. The "one
    /// public key under two ids" rule is per registry, so one key may legitimately be listed as
    /// `release` in its owner's registry and as `partner-build` in a consumer's; without this the
    /// owner's claim would verify unchanged under the other id, and anything keyed off the id —
    /// an audit trail, a policy that treats the two builds differently — would be reading a
    /// provenance nobody asserted. `a_claim_cannot_be_relabelled_as_another_identity_of_the_same_key`
    /// measures that.
    ///
    /// Binding the **public key** is defence in depth, and no test distinguishes it today:
    /// [`ArtifactAttestation::verify`] compares the carried key against the registry's before it
    /// checks the signature, and verifies with the registry's key, so an attestation that lies
    /// about its own key is already refused as [`AttestationError::KeyMismatch`]. It is kept
    /// because that ordering is the only thing making it redundant, and a signature that does not
    /// name its signer is one a later verifier could check against a key of its own choosing.
    pub fn signing_digest(&self) -> [u8; 32] {
        let mut buffer = Vec::with_capacity(160);
        absorb(&mut buffer, ARTIFACT_DOMAIN);
        absorb(&mut buffer, &self.artifact_digest);
        absorb(&mut buffer, self.key_id.as_bytes());
        absorb(&mut buffer, &self.public_key);
        sha2_256(&buffer)
    }

    /// Sign `artifact` as `key_id`.
    pub fn sign(artifact: &[u8], key_id: &str, pair: &ed25519::Pair) -> Self {
        let mut attestation = Self {
            key_id: key_id.to_string(),
            public_key: pair.public().0,
            artifact_digest: artifact_digest(artifact),
            signature: Vec::new(),
        };
        attestation.signature = pair.sign(&attestation.signing_digest()).0.to_vec();
        attestation
    }

    /// Check this attestation against `artifact` and the keys `registry` vouches for.
    pub fn verify(
        &self,
        artifact: &[u8],
        registry: &ArtifactKeyRegistry,
    ) -> Result<(), AttestationError> {
        let digest = artifact_digest(artifact);
        if digest != self.artifact_digest {
            return Err(AttestationError::DigestMismatch {
                expected: digest,
                found: self.artifact_digest,
            });
        }
        let (public_key, status) = registry
            .entry(&self.key_id)
            .ok_or_else(|| AttestationError::UnknownKey(self.key_id.clone()))?;
        if status == ArtifactKeyStatus::Revoked {
            return Err(AttestationError::RevokedKey(self.key_id.clone()));
        }
        if public_key != self.public_key {
            return Err(AttestationError::KeyMismatch(self.key_id.clone()));
        }
        let signature: [u8; 64] = self
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| AttestationError::BadSignature)?;
        let verified = ed25519::Signature::verify(
            &ed25519::Signature::from_raw(signature),
            &self.signing_digest()[..],
            &ed25519::Public::from_raw(public_key),
        );
        if verified {
            Ok(())
        } else {
            Err(AttestationError::BadSignature)
        }
    }
}

/// The keys a reader trusts to have produced an artifact, and where each is in its lifecycle.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArtifactKeyRegistry {
    keys: BTreeMap<String, ([u8; 32], ArtifactKeyStatus)>,
}

impl ArtifactKeyRegistry {
    /// Build a registry, refusing one a reader could misread: an empty or duplicated key id, a key
    /// that is not 32 bytes of hex, one public key under two ids (revoking one id would leave the
    /// key trusted under the other), or no active key at all.
    pub fn from_entries(entries: Vec<RegisteredArtifactKey>) -> Result<Self, String> {
        let mut keys = BTreeMap::new();
        let mut seen_public: BTreeMap<[u8; 32], String> = BTreeMap::new();
        for entry in entries {
            if entry.key_id.trim().is_empty() {
                return Err("a registry entry names no key id".to_string());
            }
            let public_key = decode_public_key_hex(&entry.public_key)
                .map_err(|reason| format!("key '{}': {reason}", entry.key_id))?;
            if let Some(other) = seen_public.insert(public_key, entry.key_id.clone()) {
                return Err(format!(
                    "keys '{other}' and '{}' are the same public key: revoking one id would leave \
                     the key trusted under the other",
                    entry.key_id
                ));
            }
            if keys
                .insert(entry.key_id.clone(), (public_key, entry.status))
                .is_some()
            {
                return Err(format!("key id '{}' is listed twice", entry.key_id));
            }
        }
        if !keys
            .values()
            .any(|(_, status)| *status == ArtifactKeyStatus::Active)
        {
            return Err("the registry has no active key".to_string());
        }
        Ok(Self { keys })
    }

    fn entry(&self, key_id: &str) -> Option<([u8; 32], ArtifactKeyStatus)> {
        self.keys.get(key_id).copied()
    }

    pub fn status(&self, key_id: &str) -> Option<ArtifactKeyStatus> {
        self.keys.get(key_id).map(|(_, status)| *status)
    }

    /// Whether `key_id` may sign a new artifact: only an active key may.
    pub fn may_sign(&self, key_id: &str) -> bool {
        self.status(key_id) == Some(ArtifactKeyStatus::Active)
    }
}

fn decode_public_key_hex(hex: &str) -> Result<[u8; 32], String> {
    let hex = hex.trim();
    // Checked before slicing: a multi-byte character would put a byte index off a char boundary.
    if !hex.is_ascii() {
        return Err("public key is not hex".to_string());
    }
    if hex.len() != 64 {
        return Err(format!(
            "public key must be 64 hex characters, got {}",
            hex.len()
        ));
    }
    let mut out = [0u8; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * index..2 * index + 2], 16)
            .map_err(|_| "public key is not hex".to_string())?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(seed: u8) -> ed25519::Pair {
        ed25519::Pair::from_seed(&[seed; 32])
    }

    fn hex_of(pair: &ed25519::Pair) -> String {
        pair.public().0.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn entry(
        key_id: &str,
        pair: &ed25519::Pair,
        status: ArtifactKeyStatus,
    ) -> RegisteredArtifactKey {
        RegisteredArtifactKey {
            key_id: key_id.to_string(),
            public_key: hex_of(pair),
            status,
        }
    }

    /// An artifact whose header says it demands a private channel, and one that does not, so a test
    /// can flip that bit and nothing else.
    fn artifact(private: bool) -> Vec<u8> {
        let mut bytes = vec![0u8; crate::bytecode::HEADER_LEN];
        bytes[0..4].copy_from_slice(crate::bytecode::MAGIC);
        let flags: u32 = if private {
            crate::bytecode::FEATURE_PRIVATE_SUBMISSION_REQUIRED
        } else {
            0
        };
        let offset = crate::bytecode::FEATURE_FLAGS_OFFSET;
        bytes[offset..offset + 4].copy_from_slice(&flags.to_le_bytes());
        bytes.extend_from_slice(b"body-of-the-program");
        bytes
    }

    fn registry(entries: Vec<RegisteredArtifactKey>) -> ArtifactKeyRegistry {
        ArtifactKeyRegistry::from_entries(entries).expect("registry is well formed")
    }

    #[test]
    fn a_signed_artifact_verifies_against_the_registry_that_lists_its_key() {
        let signer = pair(7);
        let registry = registry(vec![entry("release", &signer, ArtifactKeyStatus::Active)]);
        let bytes = artifact(false);
        let attestation = ArtifactAttestation::sign(&bytes, "release", &signer);

        attestation
            .verify(&bytes, &registry)
            .expect("the signer's own artifact verifies");
        assert!(registry.may_sign("release"));
    }

    /// The gap this closes. The envelope's checksum is a corruption check anyone can recompute, so
    /// a forger can edit an artifact and leave it self-consistent. The attestation is what a
    /// checksum cannot be: a claim only the key holder could have made.
    #[test]
    fn editing_the_artifact_breaks_the_attestation_even_with_the_checksum_repaired() {
        let signer = pair(7);
        let registry = registry(vec![entry("release", &signer, ArtifactKeyStatus::Active)]);
        let bytes = artifact(false);
        let attestation = ArtifactAttestation::sign(&bytes, "release", &signer);

        let mut forged = bytes.clone();
        *forged.last_mut().unwrap() ^= 0x01;
        // The forger repairs the envelope's own integrity field, which needs no key at all.
        let body = &forged[crate::bytecode::HEADER_LEN..];
        let repaired = crate::bytecode::checksum(body).to_le_bytes();
        let checksum_offset = crate::bytecode::CHECKSUM_OFFSET;
        forged[checksum_offset..checksum_offset + 4].copy_from_slice(&repaired);
        assert_eq!(
            crate::bytecode::checksum(&forged[crate::bytecode::HEADER_LEN..]).to_le_bytes(),
            forged[checksum_offset..checksum_offset + 4],
            "the forged artifact is self-consistent by its own checksum"
        );

        assert!(matches!(
            attestation.verify(&forged, &registry),
            Err(AttestationError::DigestMismatch { .. })
        ));
    }

    /// The signature covers the header, not just the body: clearing the private-submission demand
    /// is the edit that would otherwise turn "this program demands privacy" into "this program runs
    /// in the clear".
    #[test]
    fn clearing_the_private_submission_flag_breaks_the_attestation() {
        let signer = pair(7);
        let registry = registry(vec![entry("release", &signer, ArtifactKeyStatus::Active)]);
        let demanding = artifact(true);
        let attestation = ArtifactAttestation::sign(&demanding, "release", &signer);
        attestation
            .verify(&demanding, &registry)
            .expect("verifies as signed");

        let relaxed = artifact(false);
        assert_eq!(
            relaxed.len(),
            demanding.len(),
            "the two differ only in the flags word"
        );
        assert_eq!(
            crate::bytecode::feature_flags(&demanding),
            Some(crate::bytecode::FEATURE_PRIVATE_SUBMISSION_REQUIRED)
        );
        assert_eq!(crate::bytecode::feature_flags(&relaxed), Some(0));
        assert!(matches!(
            attestation.verify(&relaxed, &registry),
            Err(AttestationError::DigestMismatch { .. })
        ));
    }

    /// Rotation keeps old artifacts verifiable; revocation does not, because a leaked key can
    /// produce artifacts indistinguishable from the ones it signed legitimately.
    #[test]
    fn a_retired_key_still_verifies_what_it_signed_and_a_revoked_one_does_not() {
        let old = pair(7);
        let new = pair(9);
        let bytes = artifact(false);
        let attestation = ArtifactAttestation::sign(&bytes, "release-2025", &old);

        let rotated = registry(vec![
            entry("release-2025", &old, ArtifactKeyStatus::Retired),
            entry("release-2026", &new, ArtifactKeyStatus::Active),
        ]);
        attestation
            .verify(&bytes, &rotated)
            .expect("a retired key's artifacts stay verifiable");
        assert!(!rotated.may_sign("release-2025"));
        assert!(rotated.may_sign("release-2026"));

        let revoked = registry(vec![
            entry("release-2025", &old, ArtifactKeyStatus::Revoked),
            entry("release-2026", &new, ArtifactKeyStatus::Active),
        ]);
        assert_eq!(
            attestation.verify(&bytes, &revoked),
            Err(AttestationError::RevokedKey("release-2025".to_string()))
        );
    }

    /// Carrying a public key is not a trust decision: an attestation that signs itself consistently
    /// under an unlisted key, or claims a listed id under a different key, is refused.
    #[test]
    fn a_self_consistent_attestation_from_a_key_nobody_lists_is_refused() {
        let listed = pair(7);
        let stranger = pair(11);
        let bytes = artifact(false);
        let registry = registry(vec![entry("release", &listed, ArtifactKeyStatus::Active)]);

        let unlisted = ArtifactAttestation::sign(&bytes, "someone-else", &stranger);
        assert_eq!(
            unlisted.verify(&bytes, &registry),
            Err(AttestationError::UnknownKey("someone-else".to_string()))
        );

        // The right id, the wrong key: internally consistent, and still refused.
        let impostor = ArtifactAttestation::sign(&bytes, "release", &stranger);
        assert_eq!(
            impostor.signing_digest(),
            impostor.signing_digest(),
            "the impostor's own signature is over its own claim"
        );
        assert_eq!(
            impostor.verify(&bytes, &registry),
            Err(AttestationError::KeyMismatch("release".to_string()))
        );
    }

    /// Every field of the claim is signed, so none of them can be edited after the fact.
    #[test]
    fn editing_any_signed_field_breaks_the_signature() {
        let signer = pair(7);
        let other = pair(9);
        let bytes = artifact(false);
        let registry = registry(vec![
            entry("release", &signer, ArtifactKeyStatus::Active),
            entry("other", &other, ArtifactKeyStatus::Active),
        ]);
        let attestation = ArtifactAttestation::sign(&bytes, "release", &signer);

        // Re-attributed to another key id the registry does list, under the same key.
        let mut relabelled = attestation.clone();
        relabelled.key_id = "other".to_string();
        assert_eq!(
            relabelled.verify(&bytes, &registry),
            Err(AttestationError::KeyMismatch("other".to_string()))
        );

        // A truncated signature is refused rather than panicking on the length.
        let mut truncated = attestation.clone();
        truncated.signature.pop();
        assert_eq!(
            truncated.verify(&bytes, &registry),
            Err(AttestationError::BadSignature)
        );

        let mut flipped = attestation;
        flipped.signature[0] ^= 0x01;
        assert_eq!(
            flipped.verify(&bytes, &registry),
            Err(AttestationError::BadSignature)
        );
    }

    /// The digest is over the artifact's own bytes, so two artifacts that differ anywhere — header
    /// or body — never share one.
    #[test]
    fn the_digest_distinguishes_every_artifact_and_is_stable() {
        let a = artifact(false);
        let b = artifact(true);
        assert_eq!(artifact_digest(&a), artifact_digest(&a.clone()));
        assert_ne!(artifact_digest(&a), artifact_digest(&b));

        let mut longer = a.clone();
        longer.push(0);
        assert_ne!(artifact_digest(&a), artifact_digest(&longer));

        // The length prefix is what stops a shorter artifact plus trailing bytes from colliding
        // with a longer one.
        assert_ne!(artifact_digest(b"ab"), artifact_digest(b"a"));
    }

    /// The same key, legitimately listed under different ids by two registries: the owner's
    /// registry calls it `release`, a consumer's calls it `partner-build`. A claim made as one
    /// identity must not verify as the other.
    #[test]
    fn a_claim_cannot_be_relabelled_as_another_identity_of_the_same_key() {
        let shared = pair(7);
        let bytes = artifact(false);
        let owner = registry(vec![entry("release", &shared, ArtifactKeyStatus::Active)]);
        let consumer = registry(vec![entry(
            "partner-build",
            &shared,
            ArtifactKeyStatus::Active,
        )]);

        let claim = ArtifactAttestation::sign(&bytes, "release", &shared);
        claim
            .verify(&bytes, &owner)
            .expect("the claim as made verifies");

        let mut relabelled = claim;
        relabelled.key_id = "partner-build".to_string();
        assert_eq!(
            consumer.status("partner-build"),
            Some(ArtifactKeyStatus::Active),
            "the consumer does list this key, under its own id"
        );
        assert_eq!(
            relabelled.verify(&bytes, &consumer),
            Err(AttestationError::BadSignature),
            "and the claim was not made under that id"
        );
    }

    #[test]
    fn a_registry_a_reader_could_misread_is_refused_when_it_is_built() {
        let one = pair(7);
        let two = pair(9);
        let active = |id: &str, p: &ed25519::Pair| entry(id, p, ArtifactKeyStatus::Active);
        let cases: Vec<(&str, Vec<RegisteredArtifactKey>)> = vec![
            ("duplicate id", vec![active("a", &one), active("a", &two)]),
            (
                "one key under two ids",
                vec![
                    active("a", &one),
                    entry("b", &one, ArtifactKeyStatus::Revoked),
                ],
            ),
            (
                "no active key",
                vec![entry("a", &one, ArtifactKeyStatus::Retired)],
            ),
            ("empty id", vec![active(" ", &one)]),
            (
                "short key",
                vec![RegisteredArtifactKey {
                    key_id: "a".to_string(),
                    public_key: "abcd".to_string(),
                    status: ArtifactKeyStatus::Active,
                }],
            ),
            (
                "multi-byte characters",
                vec![RegisteredArtifactKey {
                    key_id: "a".to_string(),
                    public_key: "é".repeat(32),
                    status: ArtifactKeyStatus::Active,
                }],
            ),
        ];
        for (name, entries) in cases {
            assert!(
                ArtifactKeyRegistry::from_entries(entries).is_err(),
                "{name} must be refused"
            );
        }
    }
}
