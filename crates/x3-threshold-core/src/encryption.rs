//! Encryption utilities for the private mempool.
//!
//! Provides helpers for encrypting transactions to the committee's threshold
//! key and reconstructing plaintexts from decryption shares.
//!
//! # Cryptographic Scheme
//!
//! 1. Sender generates an ephemeral Ristretto scalar/point keypair.
//! 2. `ephemeral_scalar * committee_group_key` (a Ristretto ECDH) → shared point.
//! 3. HKDF-SHA256(shared point's compressed bytes) → AES-256-GCM key.
//! 4. AES-256-GCM encrypt(plaintext, nonce) → ciphertext.
//!
//! Decryption requires `t`-of-`n` validators to each compute a partial
//! decryption (their [`crate::threshold::SecretShare`] times the ephemeral
//! point) and [`combine_shares`] those partials via Lagrange interpolation
//! in the exponent — see [`crate::threshold`] for why this, and not raw
//! X25519 ECDH, is what makes the threshold guarantee real.

use crate::threshold::{self, SecretShare};
use crate::{DecryptionShare, EncryptedTransaction, MempoolError};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Encrypt a transaction payload for the committee.
///
/// `committee_group_key` is the committee's Ristretto group public key —
/// see [`crate::threshold::group_public_key`].
///
/// # Invariant: PRIV-EXEC-001
#[cfg(feature = "std")]
pub fn encrypt_for_committee(
    plaintext: &[u8],
    committee_group_key: &[u8; 32],
    sender_pk: &[u8; 32],
    fee_commitment: &[u8; 32],
    dkg_epoch: u64,
) -> Result<EncryptedTransaction, MempoolError> {
    let group_key = decompress_point(committee_group_key)?;

    // Ephemeral Ristretto keypair; the ECDH shared point is
    // ephemeral_scalar * group_key = ephemeral_scalar * (committee_secret * G).
    let ephemeral_scalar = Scalar::random(&mut OsRng);
    let ephemeral_pk = (ephemeral_scalar * RISTRETTO_BASEPOINT_POINT)
        .compress()
        .to_bytes();
    let shared_point = (ephemeral_scalar * group_key).compress().to_bytes();

    let aes_key = hkdf_derive(&shared_point)?;
    let nonce = generate_nonce();
    let ciphertext = aes_gcm_encrypt(plaintext, &aes_key, &nonce)?;
    let id = blake3_hash(&ciphertext);

    let submitted_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(EncryptedTransaction {
        id,
        ciphertext,
        ephemeral_pk,
        nonce,
        sender_pk: *sender_pk,
        fee_commitment: *fee_commitment,
        submitted_at,
        dkg_epoch,
    })
}

/// Compute one validator's partial decryption of `tx`, from that
/// validator's own [`SecretShare`] of the committee secret. No single
/// validator's `share` reveals the shared secret; see [`combine_shares`].
///
/// `dkg_epoch` is the DKG ceremony the validator's share was minted under. It
/// is recorded on the returned [`DecryptionShare`] so that [`combine_shares`]
/// can refuse a mix of epochs instead of discovering it as an AES-GCM tag
/// failure several steps later.
///
/// # Invariant: PRIV-EXEC-003
pub fn compute_decryption_share(
    share: &SecretShare,
    ephemeral_pk: &[u8; 32],
    dkg_epoch: u64,
) -> Result<DecryptionShare, MempoolError> {
    let ephemeral_point = decompress_point(ephemeral_pk)?;
    let partial = (share.scalar * ephemeral_point).compress().to_bytes();
    Ok(DecryptionShare {
        validator_index: share.index,
        share: partial.to_vec(),
        dkg_epoch,
        // DLEQ proof that this partial was computed honestly from the
        // validator's committed share is not implemented — see
        // https://github.com/x3-chain/x3-chain/issues (filed alongside this
        // fix) for the confidential-gpu DKG this depends on. A malicious
        // validator can currently submit a bogus partial and the caller
        // only finds out because the resulting AES-GCM tag fails to verify.
        // The epoch below is *not* a substitute for that proof: it binds a
        // share to a ceremony, it does not prove the share is f(i).
        proof: Vec::new(),
    })
}

/// Combine `threshold`-or-more decryption shares into the shared ECDH
/// point, via real Lagrange interpolation in the exponent — not by XORing
/// bytes. Any `threshold`-sized subset of honest shares produces the same
/// result; fewer than `threshold` produces an unrelated point, so
/// [`decrypt_transaction`] fails closed (AES-GCM tag mismatch) rather than
/// silently returning garbage.
///
/// Every share must declare the same `dkg_epoch` as the one being decrypted.
/// A share from another ceremony is refused with
/// [`MempoolError::ShareEpochMismatch`] rather than interpolated: partial
/// decryptions from two epochs are points under two different polynomials,
/// and combining them silently yields a point that belongs to neither.
///
/// # Invariant: PRIV-EXEC-003
pub fn combine_shares(
    shares: &[DecryptionShare],
    threshold: u32,
    dkg_epoch: u64,
) -> Result<[u8; 32], MempoolError> {
    if (shares.len() as u32) < threshold {
        return Err(MempoolError::EncryptionError(format!(
            "Need {} shares but only got {}",
            threshold,
            shares.len()
        )));
    }

    let mut seen_indices = alloc::collections::BTreeSet::new();
    let mut points = Vec::with_capacity(shares.len());
    for share in shares {
        if share.dkg_epoch != dkg_epoch {
            return Err(MempoolError::ShareEpochMismatch {
                expected: dkg_epoch,
                got: share.dkg_epoch,
            });
        }
        if share.validator_index == 0 {
            return Err(MempoolError::EncryptionError(
                "decryption share has index 0, which is reserved for the secret itself".to_string(),
            ));
        }
        if !seen_indices.insert(share.validator_index) {
            return Err(MempoolError::EncryptionError(format!(
                "duplicate decryption share for validator index {}",
                share.validator_index
            )));
        }
        points.push((share.validator_index, decompress_point_slice(&share.share)?));
    }

    Ok(threshold::combine_points(&points).compress().to_bytes())
}

/// Decrypt a transaction from `threshold`-or-more decryption shares, in one
/// step, with the epoch taken from the ciphertext rather than from the caller.
///
/// This is the fail-closed path: the epoch a share has to match is the epoch
/// `tx` was encrypted for, so no caller can pick an epoch that makes a stale
/// share acceptable. [`combine_shares`] remains public for callers that
/// combine first and decrypt later, but they have to name the epoch
/// themselves.
pub fn decrypt_with_shares(
    tx: &EncryptedTransaction,
    shares: &[DecryptionShare],
    threshold: u32,
) -> Result<Vec<u8>, MempoolError> {
    let shared_secret = combine_shares(shares, threshold, tx.dkg_epoch)?;
    decrypt_transaction(tx, &shared_secret)
}

/// Decrypt a transaction using the reconstructed shared secret.
pub fn decrypt_transaction(
    tx: &EncryptedTransaction,
    shared_secret: &[u8; 32],
) -> Result<Vec<u8>, MempoolError> {
    let aes_key = hkdf_derive(shared_secret)?;
    aes_gcm_decrypt(&tx.ciphertext, &aes_key, &tx.nonce).map_err(MempoolError::EncryptionError)
}

/// Decompress a point that will be used as key material (a committee group
/// key or an ephemeral public key), rejecting the group identity element.
///
/// The identity is a valid Ristretto encoding, but `scalar * identity ==
/// identity` for every scalar — if it were accepted as a committee group
/// key, every transaction's "shared secret" would collapse to that one
/// constant, publicly-known value regardless of the ephemeral scalar,
/// silently discarding confidentiality for the entire mempool with no
/// error and no dependence on any validator's share.
pub fn decompress_point(bytes: &[u8; 32]) -> Result<RistrettoPoint, MempoolError> {
    let point = CompressedRistretto(*bytes)
        .decompress()
        .ok_or_else(|| MempoolError::EncryptionError("invalid Ristretto point".to_string()))?;
    if point == RistrettoPoint::identity() {
        return Err(MempoolError::EncryptionError(
            "point is the group identity element, which cannot be used as key material".to_string(),
        ));
    }
    Ok(point)
}

fn decompress_point_slice(bytes: &[u8]) -> Result<RistrettoPoint, MempoolError> {
    let array: [u8; 32] = bytes.try_into().map_err(|_| {
        MempoolError::EncryptionError("decryption share is not 32 bytes".to_string())
    })?;
    decompress_point(&array)
}

// ──────────────────────────────────────────────────────────────
// Cryptographic primitives (real implementation)
// ──────────────────────────────────────────────────────────────

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT;
use curve25519_dalek::ristretto::{CompressedRistretto, RistrettoPoint};
use curve25519_dalek::scalar::Scalar;
use curve25519_dalek::traits::Identity;
use hkdf::Hkdf;
#[cfg(feature = "std")]
use rand::rngs::OsRng;
#[cfg(feature = "std")]
use rand::RngCore;
use sha2::Sha256;

fn hkdf_derive(ikm: &[u8; 32]) -> Result<[u8; 32], MempoolError> {
    let hk = Hkdf::<Sha256>::new(Some(ikm), &[]);
    let mut okm = [0u8; 32];
    hk.expand(b"encryption", &mut okm)
        .map_err(|e| MempoolError::EncryptionError(e.to_string()))?;
    Ok(okm)
}

#[cfg(feature = "std")]
fn generate_nonce() -> [u8; 12] {
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

fn aes_gcm_encrypt(
    plaintext: &[u8],
    key: &[u8; 32],
    nonce: &[u8; 12],
) -> Result<Vec<u8>, MempoolError> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| MempoolError::EncryptionError(e.to_string()))?;
    let nonce = Nonce::from_slice(nonce);
    cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| MempoolError::EncryptionError(e.to_string()))
}

fn aes_gcm_decrypt(ciphertext: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let nonce = Nonce::from_slice(nonce);
    cipher.decrypt(nonce, ciphertext).map_err(|e| e.to_string())
}

pub fn blake3_hash(data: &[u8]) -> [u8; 32] {
    use blake3::Hasher;
    let mut hasher = Hasher::new();
    hasher.update(data);
    let mut hash = [0u8; 32];
    hasher.finalize_xof().fill(&mut hash);
    hash
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::threshold::{group_public_key, split_secret};

    /// The DKG epoch every transaction in this module is encrypted for; the
    /// epoch-mismatch tests state a different one explicitly.
    const EPOCH: u64 = 1;

    #[test]
    fn encrypt_decrypt_roundtrip_with_a_real_threshold_committee() {
        let plaintext = b"Hello, private world!";

        // A real DKG (elsewhere) would hand each validator one of these
        // shares and publish only the group key; nothing here ever holds
        // `committee_secret` except this line simulating that ceremony.
        let committee_secret = Scalar::random(&mut OsRng);
        let committee_group_key = group_public_key(&committee_secret);
        let shares = split_secret(committee_secret, 3, 5);

        let sender_pk = [0xAA; 32];
        let fee_commitment = [0xBB; 32];

        let tx = encrypt_for_committee(
            plaintext,
            &committee_group_key,
            &sender_pk,
            &fee_commitment,
            EPOCH,
        )
        .unwrap();

        // Any 3 of the 5 validators — not a fixed subset — reconstruct the
        // same shared secret.
        let partials: Vec<DecryptionShare> = [shares[1], shares[2], shares[4]]
            .iter()
            .map(|s| compute_decryption_share(s, &tx.ephemeral_pk, EPOCH).unwrap())
            .collect();

        let shared_secret = combine_shares(&partials, 3, EPOCH).unwrap();
        let decrypted = decrypt_transaction(&tx, &shared_secret).unwrap();
        assert_eq!(&decrypted, plaintext);
    }

    #[test]
    fn below_threshold_shares_fail_to_decrypt() {
        let plaintext = b"top secret trade";
        let committee_secret = Scalar::random(&mut OsRng);
        let committee_group_key = group_public_key(&committee_secret);
        let shares = split_secret(committee_secret, 3, 5);

        let tx = encrypt_for_committee(plaintext, &committee_group_key, &[0; 32], &[0; 32], EPOCH)
            .unwrap();

        // Only 2 of the required 3 shares.
        let partials: Vec<DecryptionShare> = [shares[0], shares[1]]
            .iter()
            .map(|s| compute_decryption_share(s, &tx.ephemeral_pk, EPOCH).unwrap())
            .collect();

        // combine_shares' own length check is bypassed by lying about the
        // threshold, to prove the *cryptographic* guarantee also holds: an
        // under-threshold combination is not just rejected by a length
        // check, it actually reconstructs the wrong point.
        let wrong_secret = combine_shares(&partials, 2, EPOCH).unwrap();
        let result = decrypt_transaction(&tx, &wrong_secret);
        assert!(
            result.is_err(),
            "AES-GCM must reject a reconstructed-from-too-few-shares key"
        );
    }

    #[test]
    fn combine_shares_enforces_the_stated_threshold() {
        let result = combine_shares(&[], 3, EPOCH);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_the_identity_element_as_a_committee_group_key() {
        let identity_key = RistrettoPoint::identity().compress().to_bytes();
        let result = encrypt_for_committee(b"msg", &identity_key, &[0; 32], &[0; 32], EPOCH);
        assert!(
            result.is_err(),
            "encrypting to the identity element must be rejected, not silently \
             produce a publicly-known shared secret"
        );
    }

    #[test]
    fn rejects_the_identity_element_as_an_ephemeral_key() {
        let committee_secret = Scalar::random(&mut OsRng);
        let share = split_secret(committee_secret, 1, 1).remove(0);
        let identity_pk = RistrettoPoint::identity().compress().to_bytes();
        let result = compute_decryption_share(&share, &identity_pk, EPOCH);
        assert!(result.is_err());
    }

    #[test]
    fn combine_shares_rejects_a_zero_validator_index() {
        let committee_secret = Scalar::random(&mut OsRng);
        let committee_group_key = group_public_key(&committee_secret);
        let shares = split_secret(committee_secret, 2, 2);
        let tx =
            encrypt_for_committee(b"msg", &committee_group_key, &[0; 32], &[0; 32], EPOCH).unwrap();

        let mut partials: Vec<DecryptionShare> = shares
            .iter()
            .map(|s| compute_decryption_share(s, &tx.ephemeral_pk, EPOCH).unwrap())
            .collect();
        partials[0].validator_index = 0;

        let result = combine_shares(&partials, 2, EPOCH);
        assert!(result.is_err());
    }

    #[test]
    fn combine_shares_rejects_duplicate_validator_indices() {
        let committee_secret = Scalar::random(&mut OsRng);
        let committee_group_key = group_public_key(&committee_secret);
        let shares = split_secret(committee_secret, 2, 3);
        let tx =
            encrypt_for_committee(b"msg", &committee_group_key, &[0; 32], &[0; 32], EPOCH).unwrap();

        // Two entries both claiming to be validator 1 — a second, possibly
        // malicious, share silently overriding another validator's weight
        // in the Lagrange combination instead of being rejected outright.
        let mut partials: Vec<DecryptionShare> = vec![
            compute_decryption_share(&shares[0], &tx.ephemeral_pk, EPOCH).unwrap(),
            compute_decryption_share(&shares[1], &tx.ephemeral_pk, EPOCH).unwrap(),
        ];
        let mut impostor = compute_decryption_share(&shares[2], &tx.ephemeral_pk, EPOCH).unwrap();
        impostor.validator_index = partials[0].validator_index;
        partials.push(impostor);

        let result = combine_shares(&partials, 2, EPOCH);
        assert!(result.is_err());
    }

    // ──────────────────────────────────────────────────────────────────
    // The adversarial set: a fresh committee for every case, so nothing
    // below can pass because two ceremonies happened to agree.
    // ──────────────────────────────────────────────────────────────────

    /// A brand-new committee — a new random secret each call, and therefore a
    /// new polynomial, new shares and a new group key.
    fn fresh_committee(threshold: u32, size: u32) -> ([u8; 32], Vec<SecretShare>) {
        let secret = Scalar::random(&mut OsRng);
        (
            group_public_key(&secret),
            split_secret(secret, threshold, size),
        )
    }

    fn partials_for(tx: &EncryptedTransaction, shares: &[SecretShare]) -> Vec<DecryptionShare> {
        shares
            .iter()
            .map(|s| compute_decryption_share(s, &tx.ephemeral_pk, tx.dkg_epoch).unwrap())
            .collect()
    }

    fn encrypt(plaintext: &[u8], group_key: &[u8; 32]) -> EncryptedTransaction {
        encrypt_for_committee(plaintext, group_key, &[0x11; 32], &[0x22; 32], EPOCH).unwrap()
    }

    #[test]
    fn t_shares_decrypt_and_one_short_never_does_over_fresh_committees() {
        for _ in 0..4 {
            let plaintext = b"a private order the mempool must keep";
            let (group_key, shares) = fresh_committee(3, 5);
            let tx = encrypt(plaintext, &group_key);

            // Any three of the five decrypt to the same plaintext...
            for subset in [[0usize, 1, 2], [0, 1, 3], [0, 2, 4], [1, 3, 4], [2, 3, 4]] {
                let picked: Vec<SecretShare> = subset.iter().map(|&i| shares[i]).collect();
                let decrypted = decrypt_with_shares(&tx, &partials_for(&tx, &picked), 3).unwrap();
                assert_eq!(
                    decrypted, plaintext,
                    "subset {subset:?} of a 3-of-5 committee must decrypt"
                );
            }

            // ...and any two do not, even when the caller declares a threshold
            // of 2 so that no length check can be doing the work.
            for subset in [[0usize, 1], [2, 3], [3, 4]] {
                let picked: Vec<SecretShare> = subset.iter().map(|&i| shares[i]).collect();
                assert!(
                    decrypt_with_shares(&tx, &partials_for(&tx, &picked), 2).is_err(),
                    "2 shares of a 3-of-5 committee must not reconstruct the key"
                );
            }
        }
    }

    #[test]
    fn a_tampered_ciphertext_is_refused() {
        let (group_key, shares) = fresh_committee(3, 5);
        let mut tx = encrypt(b"transfer 100 to alice", &group_key);
        let middle = tx.ciphertext.len() / 2;
        tx.ciphertext[middle] ^= 0x01;
        assert!(
            decrypt_with_shares(&tx, &partials_for(&tx, &shares[..3]), 3).is_err(),
            "flipping one ciphertext bit must fail the AEAD tag, not yield plaintext"
        );
    }

    #[test]
    fn a_tampered_nonce_is_refused() {
        let (group_key, shares) = fresh_committee(3, 5);
        let mut tx = encrypt(b"transfer 100 to alice", &group_key);
        tx.nonce[11] ^= 0x80;
        assert!(decrypt_with_shares(&tx, &partials_for(&tx, &shares[..3]), 3).is_err());
    }

    #[test]
    fn a_swapped_nonce_between_two_transactions_is_refused() {
        let (group_key, shares) = fresh_committee(3, 5);
        let mut first = encrypt(b"first", &group_key);
        let second = encrypt(b"second", &group_key);
        // Same committee, same sender, valid ciphertext — only the nonce is
        // someone else's. AEAD must not accept it.
        first.nonce = second.nonce;
        assert!(decrypt_with_shares(&first, &partials_for(&first, &shares[..3]), 3).is_err());
    }

    #[test]
    fn a_swapped_ephemeral_key_between_two_transactions_is_refused() {
        let (group_key, shares) = fresh_committee(3, 5);
        let first = encrypt(b"first", &group_key);
        let second = encrypt(b"second", &group_key);
        assert_ne!(first.ephemeral_pk, second.ephemeral_pk);

        // Partials taken against *second*'s ephemeral point, then used to open
        // *first*: the reconstructed ECDH point belongs to another ciphertext.
        let wrong: Vec<DecryptionShare> = shares[..3]
            .iter()
            .map(|s| compute_decryption_share(s, &second.ephemeral_pk, EPOCH).unwrap())
            .collect();
        assert!(decrypt_with_shares(&first, &wrong, 3).is_err());
    }

    #[test]
    fn a_share_labelled_with_an_index_outside_the_committee_fails_closed() {
        let (group_key, shares) = fresh_committee(3, 5);
        let tx = encrypt(b"transfer 100 to alice", &group_key);
        let mut partials = partials_for(&tx, &shares[..3]);
        // The three partials are honest; one of them is simply mislabelled.
        partials[2].validator_index = 9;
        assert!(
            decrypt_with_shares(&tx, &partials, 3).is_err(),
            "interpolating at the wrong x must not reproduce the key"
        );
    }

    #[test]
    fn a_share_that_is_not_a_point_is_refused() {
        let (group_key, shares) = fresh_committee(3, 5);
        let tx = encrypt(b"transfer 100 to alice", &group_key);
        let mut partials = partials_for(&tx, &shares[..3]);
        partials[1].share = vec![0xFF; 32];
        assert!(decrypt_with_shares(&tx, &partials, 3).is_err());
    }

    #[test]
    fn a_share_from_a_different_dkg_epoch_is_refused_with_a_typed_error() {
        let (epoch_one_key, epoch_one_shares) = fresh_committee(3, 5);
        let (_, epoch_two_shares) = fresh_committee(3, 5);
        let tx = encrypt(b"transfer 100 to alice", &epoch_one_key);

        let mut shares = partials_for(&tx, &epoch_one_shares[..3]);
        // A share minted under the next ceremony, offered to this decryption.
        shares.push(
            compute_decryption_share(&epoch_two_shares[0], &tx.ephemeral_pk, EPOCH + 1).unwrap(),
        );

        let err = combine_shares(&shares, 3, EPOCH).unwrap_err();
        assert!(
            matches!(
                err,
                MempoolError::ShareEpochMismatch { expected, got }
                    if expected == EPOCH && got == EPOCH + 1
            ),
            "a stale share must be named as an epoch mismatch, not discovered as a tag failure: {err}"
        );

        // The same refusal on the path that takes its epoch from the ciphertext.
        let stale = vec![
            compute_decryption_share(&epoch_two_shares[0], &tx.ephemeral_pk, EPOCH + 1).unwrap(),
            compute_decryption_share(&epoch_two_shares[1], &tx.ephemeral_pk, EPOCH + 1).unwrap(),
            compute_decryption_share(&epoch_two_shares[2], &tx.ephemeral_pk, EPOCH + 1).unwrap(),
        ];
        assert!(matches!(
            decrypt_with_shares(&tx, &stale, 3),
            Err(MempoolError::ShareEpochMismatch { .. })
        ));
    }

    #[test]
    fn the_epoch_label_is_not_the_security_boundary() {
        // Two different ceremonies, both labelled with the same epoch. The
        // label check passes; the ciphertext is what actually refuses.
        let (group_key, shares_a) = fresh_committee(3, 5);
        let (_, shares_b) = fresh_committee(3, 5);
        let tx = encrypt(b"transfer 100 to alice", &group_key);

        let mixed = vec![
            compute_decryption_share(&shares_a[0], &tx.ephemeral_pk, EPOCH).unwrap(),
            compute_decryption_share(&shares_a[1], &tx.ephemeral_pk, EPOCH).unwrap(),
            compute_decryption_share(&shares_b[2], &tx.ephemeral_pk, EPOCH).unwrap(),
        ];
        let secret = combine_shares(&mixed, 3, EPOCH)
            .expect("the label agrees, so the combiner cannot refuse this");
        assert!(
            decrypt_transaction(&tx, &secret).is_err(),
            "a share from another ceremony must not yield the plaintext, whatever epoch it claims"
        );
    }

    #[test]
    fn combine_shares_refuses_fewer_shares_than_the_declared_threshold() {
        let (group_key, shares) = fresh_committee(3, 5);
        let tx = encrypt(b"transfer 100 to alice", &group_key);
        let two = partials_for(&tx, &shares[..2]);
        let err = combine_shares(&two, 3, EPOCH).unwrap_err();
        assert!(
            matches!(&err, MempoolError::EncryptionError(message) if message.contains("Need 3 shares")),
            "the declared threshold is enforced by the combiner, not trusted from its caller: {err}"
        );
    }
}
