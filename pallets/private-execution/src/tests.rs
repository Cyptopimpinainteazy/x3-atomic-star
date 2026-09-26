//! Unit tests for the Private Execution pallet.
//!
//! # Invariants tested:
//! - PRIV-EXEC-004: Attestation verified before joining confidential set
//! - PRIV-EXEC-005: Premium fee correctly collected and split

use crate::{mock::*, types::*, Error};
use frame_support::{assert_noop, assert_ok};
use sp_core::H256;
use x3_order_window::{commitment_hash, order_key, OrderingWindow, MAX_PLAINTEXT_BYTES};

fn dummy_attestation() -> Vec<u8> {
    // A labelled fixture, not "some non-empty bytes": `TestAttestationVerifier` accepts
    // this prefix and refuses everything else, so a report the verifier does not
    // recognise cannot register a validator just by existing.
    b"TEST-TEE-QUOTE\x00nvidia-h100-test-fixture".to_vec()
}

fn dummy_enclave_key() -> [u8; 32] {
    [0xAA; 32]
}

// ──────────────────────────────────────────────────────────────
// Validator Registration
// ──────────────────────────────────────────────────────────────

/// # Invariant: PRIV-EXEC-004
#[test]
fn reject_unattested() {
    new_test_ext().execute_with(|| {
        // Enable private execution
        assert_ok!(PrivateExecution::set_enabled(RuntimeOrigin::root(), true));

        // Empty attestation should fail
        assert_noop!(
            PrivateExecution::register_confidential_validator(
                RuntimeOrigin::signed(1),
                b"NVIDIA H100".to_vec(),
                vec![], // empty = invalid
                dummy_enclave_key(),
            ),
            Error::<Test>::InvalidAttestation
        );

        // Valid attestation should succeed
        assert_ok!(PrivateExecution::register_confidential_validator(
            RuntimeOrigin::signed(1),
            b"NVIDIA H100".to_vec(),
            dummy_attestation(),
            dummy_enclave_key(),
        ));

        let att = PrivateExecution::confidential_validators(1).unwrap();
        assert_eq!(att.status, EnclaveStatus::Verified);
        assert_eq!(PrivateExecution::confidential_validator_count(), 1);
    });
}

/// The behaviour this pallet shipped with: `verify_attestation` was
/// `!report.is_empty()`, so any signed account could register as a confidential validator
/// — and collect the confidential premium-fee share — with a single byte. A report that
/// merely *looks* like an attestation must be refused too.
#[test]
fn a_plausible_report_the_verifier_does_not_recognise_is_refused() {
    new_test_ext().execute_with(|| {
        assert_ok!(PrivateExecution::set_enabled(RuntimeOrigin::root(), true));

        assert_noop!(
            PrivateExecution::register_confidential_validator(
                RuntimeOrigin::signed(1),
                b"NVIDIA H100".to_vec(),
                b"NVIDIA-CC-ATTESTATION-REPORT-V1".to_vec(),
                dummy_enclave_key(),
            ),
            Error::<Test>::InvalidAttestation
        );
        assert_noop!(
            PrivateExecution::register_confidential_validator(
                RuntimeOrigin::signed(1),
                b"NVIDIA H100".to_vec(),
                vec![1],
                dummy_enclave_key(),
            ),
            Error::<Test>::InvalidAttestation
        );
        assert_eq!(PrivateExecution::confidential_validator_count(), 0);
    });
}

/// The posture the runtime configures: with no vendor trust root, the shipped default
/// verifier refuses every report, so confidential-validator registration is disabled
/// rather than open to anyone who can sign.
#[test]
fn the_shipped_verifier_refuses_every_report() {
    use crate::{RefuseAllAttestations, TeeAttestationVerifier};
    for report in [
        &b""[..],
        &b"x"[..],
        &b"NVIDIA-CC-ATTESTATION-REPORT-V1"[..],
        &[0xFFu8; 4096][..],
    ] {
        assert!(!RefuseAllAttestations::verify(
            report,
            b"NVIDIA H100",
            &[7u8; 32]
        ));
    }
}

#[test]
fn register_multiple_validators() {
    new_test_ext().execute_with(|| {
        assert_ok!(PrivateExecution::set_enabled(RuntimeOrigin::root(), true));

        for i in 1..=3u64 {
            assert_ok!(PrivateExecution::register_confidential_validator(
                RuntimeOrigin::signed(i),
                b"NVIDIA H100".to_vec(),
                dummy_attestation(),
                [i as u8; 32],
            ));
        }

        assert_eq!(PrivateExecution::confidential_validator_count(), 3);
    });
}

#[test]
fn deregister_validator() {
    new_test_ext().execute_with(|| {
        assert_ok!(PrivateExecution::set_enabled(RuntimeOrigin::root(), true));

        assert_ok!(PrivateExecution::register_confidential_validator(
            RuntimeOrigin::signed(1),
            b"NVIDIA H100".to_vec(),
            dummy_attestation(),
            dummy_enclave_key(),
        ));

        assert_ok!(PrivateExecution::deregister_confidential_validator(
            RuntimeOrigin::signed(1)
        ));

        assert!(PrivateExecution::confidential_validators(1).is_none());
        assert_eq!(PrivateExecution::confidential_validator_count(), 0);
    });
}

// ──────────────────────────────────────────────────────────────
// Private Transaction Submission
// ──────────────────────────────────────────────────────────────

fn setup_quorum() {
    assert_ok!(PrivateExecution::set_enabled(RuntimeOrigin::root(), true));

    // Register 2 validators (MinConfidentialQuorum = 2)
    for i in 1..=2u64 {
        assert_ok!(PrivateExecution::register_confidential_validator(
            RuntimeOrigin::signed(i),
            b"NVIDIA H100".to_vec(),
            dummy_attestation(),
            [i as u8; 32],
        ));
    }

    // Set DKG committee key
    assert_ok!(PrivateExecution::set_committee_key(
        RuntimeOrigin::root(),
        vec![0xBB; 32],
    ));
}

#[test]
fn submit_private_tx_requires_quorum() {
    new_test_ext().execute_with(|| {
        assert_ok!(PrivateExecution::set_enabled(RuntimeOrigin::root(), true));

        // No validators registered yet
        assert_noop!(
            PrivateExecution::submit_private_transaction(
                RuntimeOrigin::signed(10),
                H256::repeat_byte(0x01),
                vec![0xCA; 256],
                H256::repeat_byte(0x02),
                1_000u128,
            ),
            Error::<Test>::InsufficientQuorum
        );
    });
}

/// # Invariant: PRIV-EXEC-005
#[test]
fn fee_premium_accounting() {
    new_test_ext().execute_with(|| {
        setup_quorum();

        let user_balance_before = Balances::free_balance(10);
        let base_fee: u128 = 10_000;

        assert_ok!(PrivateExecution::submit_private_transaction(
            RuntimeOrigin::signed(10),
            H256::repeat_byte(0x01),
            vec![0xCA; 256],
            H256::repeat_byte(0x02),
            base_fee,
        ));

        // Premium = 1.5% of 10_000 = 150
        // Total fee = 10_000 + 150 = 10_150
        let user_balance_after = Balances::free_balance(10);
        let fee_charged = user_balance_before - user_balance_after;
        assert_eq!(fee_charged, 10_150);

        // Premium tracked
        assert_eq!(PrivateExecution::total_premium_fees(), 150);

        // TX recorded
        let record = PrivateExecution::private_transactions(H256::repeat_byte(0x01)).unwrap();
        assert_eq!(record.status, PrivateTxStatus::Pending);
        assert_eq!(record.fee_paid, 10_150);
    });
}

// ──────────────────────────────────────────────────────────────
// State Diff Commitment
// ──────────────────────────────────────────────────────────────

#[test]
fn commit_state_diff_works() {
    new_test_ext().execute_with(|| {
        setup_quorum();

        let tx_hash = H256::repeat_byte(0x01);

        // Submit private TX
        assert_ok!(PrivateExecution::submit_private_transaction(
            RuntimeOrigin::signed(10),
            tx_hash,
            vec![0xCA; 256],
            H256::repeat_byte(0x02),
            1_000u128,
        ));

        // Validator 1 commits state diff
        assert_ok!(PrivateExecution::commit_encrypted_state_diff(
            RuntimeOrigin::signed(1),
            tx_hash,
            vec![0xDE; 128],         // encrypted state changes
            H256::repeat_byte(0x03), // commitment
            None,                    // no ZK proof
            [0x51; 64],              // enclave signature (placeholder)
        ));

        // TX status updated
        let record = PrivateExecution::private_transactions(tx_hash).unwrap();
        assert_eq!(record.status, PrivateTxStatus::Committed);
        assert_eq!(record.executed_by, Some(1));

        // State diff stored
        let diffs = PrivateExecution::encrypted_state_diffs(1); // block 1
        assert_eq!(diffs.len(), 1);
    });
}

// ──────────────────────────────────────────────────────────────
// DKG Key Rotation
// ──────────────────────────────────────────────────────────────

#[test]
fn dkg_key_rotation() {
    new_test_ext().execute_with(|| {
        assert_ok!(PrivateExecution::set_committee_key(
            RuntimeOrigin::root(),
            vec![0xAA; 32],
        ));
        assert_eq!(PrivateExecution::dkg_epoch(), 1);

        assert_ok!(PrivateExecution::set_committee_key(
            RuntimeOrigin::root(),
            vec![0xBB; 32],
        ));
        assert_eq!(PrivateExecution::dkg_epoch(), 2);
    });
}

// ──────────────────────────────────────────────────────────────
// Enable/Disable Toggle
// ──────────────────────────────────────────────────────────────

#[test]
fn toggle_private_execution() {
    new_test_ext().execute_with(|| {
        assert!(!PrivateExecution::is_enabled());

        assert_ok!(PrivateExecution::set_enabled(RuntimeOrigin::root(), true));
        assert!(PrivateExecution::is_enabled());

        assert_ok!(PrivateExecution::set_enabled(RuntimeOrigin::root(), false));
        assert!(!PrivateExecution::is_enabled());
    });
}

// ──────────────────────────────────────────────────────────────
// Commit–reveal ordering window (X3-MEV-006 / X3-MEV-008)
// ──────────────────────────────────────────────────────────────

const OPEN: u64 = 10;
const CLOSE: u64 = 20;
/// Equal to the mock's `MinOrderingBond`.
const BOND: u128 = 1_000;

/// Turn private execution on and register the confidential quorum the window
/// guards require. Mirrors `setup_quorum` for the ordering tests.
fn enable_with_quorum() {
    assert_ok!(PrivateExecution::set_enabled(RuntimeOrigin::root(), true));
    for i in 1..=2u64 {
        assert_ok!(PrivateExecution::register_confidential_validator(
            RuntimeOrigin::signed(i),
            b"NVIDIA H100".to_vec(),
            dummy_attestation(),
            [i as u8; 32],
        ));
    }
}

/// Open the first window and land at its first block.
fn open_window() -> u64 {
    enable_with_quorum();
    System::set_block_number(OPEN);
    let window_id = PrivateExecution::next_ordering_window_id();
    assert_ok!(PrivateExecution::open_ordering_window(
        RuntimeOrigin::signed(10),
        OPEN,
        CLOSE,
    ));
    window_id
}

/// The commitment hash a participant must publish, computed the same way the
/// chain binds the sender: through the pallet's own label.
fn hash_for(who: u64, plaintext: &[u8], nonce: &[u8; 32]) -> H256 {
    commitment_hash(
        PrivateExecution::ordering_sender_label(&who),
        plaintext,
        nonce,
    )
}

fn commit(who: u64, window_id: u64, hash: H256) {
    assert_ok!(PrivateExecution::commit_ordering(
        RuntimeOrigin::signed(who),
        window_id,
        hash,
        BOND,
    ));
}

fn reveal(who: u64, window_id: u64, hash: H256, plaintext: &[u8], nonce: &[u8; 32]) {
    assert_ok!(PrivateExecution::reveal_ordering(
        RuntimeOrigin::signed(who),
        window_id,
        hash,
        plaintext.to_vec(),
        *nonce,
    ));
}

/// `(hash, account, plaintext, nonce)` per participant, so a test can sort by
/// hash without re-deriving anything or fighting reference patterns.
fn window_entries(
    people: &[(u64, &'static [u8], [u8; 32])],
) -> Vec<(H256, u64, &'static [u8], [u8; 32])> {
    people
        .iter()
        .map(|&(who, plaintext, nonce)| (hash_for(who, plaintext, &nonce), who, plaintext, nonce))
        .collect()
}

#[test]
fn opening_a_window_fixes_its_bond_and_range() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        let record = PrivateExecution::ordering_windows(window_id).unwrap();

        let lane_window = OrderingWindow::new(OPEN, CLOSE).expect("10..=20 is a window");
        assert_eq!(record.open_block, lane_window.open_block);
        assert_eq!(record.close_block, lane_window.close_block);
        assert_eq!(record.minimum_bond, BOND);
        assert!(!record.settled);
        assert_eq!(record.beacon, None);
        assert_eq!(record.commitment_count, 0);
        assert_eq!(record.reveal_count, 0);
        assert_eq!(PrivateExecution::next_ordering_window_id(), 1);
    });
}

#[test]
fn an_inverted_or_already_closed_window_is_refused() {
    new_test_ext().execute_with(|| {
        enable_with_quorum();
        System::set_block_number(OPEN);

        assert_noop!(
            PrivateExecution::open_ordering_window(RuntimeOrigin::signed(10), CLOSE, OPEN),
            Error::<Test>::OrderingWindowInverted
        );

        System::set_block_number(CLOSE + 1);
        assert_noop!(
            PrivateExecution::open_ordering_window(RuntimeOrigin::signed(10), OPEN, CLOSE),
            Error::<Test>::OrderingWindowAlreadyClosed
        );
        assert_eq!(PrivateExecution::next_ordering_window_id(), 0);
    });
}

#[test]
fn opening_and_committing_need_the_same_guards_as_private_submission() {
    new_test_ext().execute_with(|| {
        // Disabled: both refused.
        assert_noop!(
            PrivateExecution::open_ordering_window(RuntimeOrigin::signed(10), OPEN, CLOSE),
            Error::<Test>::PrivateExecutionDisabled
        );

        // Enabled but below quorum: refused with a different name, so the two
        // guards are distinguishable.
        assert_ok!(PrivateExecution::set_enabled(RuntimeOrigin::root(), true));
        assert_noop!(
            PrivateExecution::open_ordering_window(RuntimeOrigin::signed(10), OPEN, CLOSE),
            Error::<Test>::InsufficientQuorum
        );

        enable_with_quorum();
        System::set_block_number(OPEN);
        let window_id = PrivateExecution::next_ordering_window_id();
        assert_ok!(PrivateExecution::open_ordering_window(
            RuntimeOrigin::signed(10),
            OPEN,
            CLOSE,
        ));

        // A commit while private execution is off is refused before it can
        // reserve a bond.
        assert_ok!(PrivateExecution::set_enabled(RuntimeOrigin::root(), false));
        assert_noop!(
            PrivateExecution::commit_ordering(
                RuntimeOrigin::signed(11),
                window_id,
                hash_for(11, b"payload", &[1u8; 32]),
                BOND,
            ),
            Error::<Test>::PrivateExecutionDisabled
        );
        assert_eq!(Balances::reserved_balance(11), 0);
    });
}

/// A commit outside the window is refused and reserves nothing.
#[test]
fn a_commit_outside_the_window_is_refused() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        let hash = hash_for(11, b"payload", &[1u8; 32]);

        System::set_block_number(OPEN - 1);
        assert_noop!(
            PrivateExecution::commit_ordering(RuntimeOrigin::signed(11), window_id, hash, BOND),
            Error::<Test>::OrderingWindowNotOpen
        );
        System::set_block_number(CLOSE + 1);
        assert_noop!(
            PrivateExecution::commit_ordering(RuntimeOrigin::signed(11), window_id, hash, BOND),
            Error::<Test>::OrderingWindowNotOpen
        );
        assert_eq!(PrivateExecution::ordering_commits(window_id, hash), None);
        assert_eq!(Balances::reserved_balance(11), 0);

        // The window is still usable once the clock is back inside it.
        System::set_block_number(OPEN);
        commit(11, window_id, hash);
        assert_eq!(Balances::reserved_balance(11), BOND);
    });
}

#[test]
fn a_bond_below_the_minimum_is_refused() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        let hash = hash_for(11, b"payload", &[1u8; 32]);

        assert_noop!(
            PrivateExecution::commit_ordering(RuntimeOrigin::signed(11), window_id, hash, BOND - 1),
            Error::<Test>::OrderingBondBelowMinimum
        );
        assert_eq!(Balances::reserved_balance(11), 0);
        assert_eq!(
            PrivateExecution::ordering_windows(window_id)
                .unwrap()
                .commitment_count,
            0
        );
    });
}

#[test]
fn one_sender_commits_once_per_window() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        let first = hash_for(11, b"first", &[1u8; 32]);
        commit(11, window_id, first);

        let second = hash_for(11, b"second", &[2u8; 32]);
        assert_noop!(
            PrivateExecution::commit_ordering(RuntimeOrigin::signed(11), window_id, second, BOND),
            Error::<Test>::OrderingAlreadyCommitted
        );
        assert_eq!(PrivateExecution::ordering_commits(window_id, second), None);
        assert_eq!(Balances::reserved_balance(11), BOND);
    });
}

#[test]
fn the_same_commit_hash_is_used_once_per_window() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        // Two distinct accounts, the same published hash: the second is refused
        // because a commit hash names one commitment.
        let hash = hash_for(11, b"shared", &[1u8; 32]);
        commit(11, window_id, hash);

        assert_noop!(
            PrivateExecution::commit_ordering(RuntimeOrigin::signed(12), window_id, hash, BOND),
            Error::<Test>::OrderingCommitAlreadyUsed
        );
        assert_eq!(Balances::reserved_balance(12), 0);
        assert_eq!(
            PrivateExecution::ordering_windows(window_id)
                .unwrap()
                .commitment_count,
            1
        );
    });
}

#[test]
fn a_window_beyond_its_capacity_is_refused() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();

        // The mock allows four commitments; the fifth is refused by name.
        for (index, who) in [10u64, 11, 12, 13].into_iter().enumerate() {
            let nonce = [index as u8 + 1; 32];
            commit(who, window_id, hash_for(who, b"payload", &nonce));
        }
        assert_eq!(
            PrivateExecution::ordering_windows(window_id)
                .unwrap()
                .commitment_count,
            MaxOrderingCommits::get()
        );

        assert_noop!(
            PrivateExecution::commit_ordering(
                RuntimeOrigin::signed(14),
                window_id,
                hash_for(14, b"payload", &[9u8; 32]),
                BOND,
            ),
            Error::<Test>::OrderingWindowFull
        );
        assert_eq!(Balances::reserved_balance(14), 0);
    });
}

#[test]
fn a_reveal_outside_the_window_is_refused() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        let nonce = [1u8; 32];
        let hash = hash_for(11, b"payload", &nonce);
        commit(11, window_id, hash);

        System::set_block_number(OPEN - 1);
        assert_noop!(
            PrivateExecution::reveal_ordering(
                RuntimeOrigin::signed(11),
                window_id,
                hash,
                b"payload".to_vec(),
                nonce,
            ),
            Error::<Test>::OrderingWindowNotOpen
        );
        System::set_block_number(CLOSE + 1);
        assert_noop!(
            PrivateExecution::reveal_ordering(
                RuntimeOrigin::signed(11),
                window_id,
                hash,
                b"payload".to_vec(),
                nonce,
            ),
            Error::<Test>::OrderingWindowNotOpen
        );
        assert_eq!(PrivateExecution::ordering_reveals(window_id, hash), None);
        assert_eq!(Balances::reserved_balance(11), BOND);
    });
}

#[test]
fn a_reveal_without_a_commit_is_refused() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        let nonce = [1u8; 32];
        let hash = hash_for(11, b"payload", &nonce);

        assert_noop!(
            PrivateExecution::reveal_ordering(
                RuntimeOrigin::signed(11),
                window_id,
                hash,
                b"payload".to_vec(),
                nonce,
            ),
            Error::<Test>::OrderingUnknownCommitment
        );
        assert_eq!(
            PrivateExecution::ordering_windows(window_id)
                .unwrap()
                .reveal_count,
            0
        );
    });
}

#[test]
fn a_reveal_that_does_not_hash_to_its_commit_is_refused() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        let nonce = [1u8; 32];
        let hash = hash_for(11, b"payload", &nonce);
        commit(11, window_id, hash);

        // A substituted payload.
        assert_noop!(
            PrivateExecution::reveal_ordering(
                RuntimeOrigin::signed(11),
                window_id,
                hash,
                b"send-everything-to-me".to_vec(),
                nonce,
            ),
            Error::<Test>::OrderingRevealMismatch
        );

        // A swapped nonce.
        let mut other = nonce;
        other[0] ^= 0xFF;
        assert_noop!(
            PrivateExecution::reveal_ordering(
                RuntimeOrigin::signed(11),
                window_id,
                hash,
                b"payload".to_vec(),
                other,
            ),
            Error::<Test>::OrderingRevealMismatch
        );

        assert_eq!(PrivateExecution::ordering_reveals(window_id, hash), None);
        assert_eq!(Balances::reserved_balance(11), BOND);
        // The correct reveal still lands afterwards.
        reveal(11, window_id, hash, b"payload", &nonce);
    });
}

#[test]
fn only_the_committer_may_reveal() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        let nonce = [1u8; 32];
        let hash = hash_for(11, b"payload", &nonce);
        commit(11, window_id, hash);

        // Account 12 knows the payload and nonce, and still cannot reveal: the
        // authority is the recorded account, checked before anything else.
        assert_noop!(
            PrivateExecution::reveal_ordering(
                RuntimeOrigin::signed(12),
                window_id,
                hash,
                b"payload".to_vec(),
                nonce,
            ),
            Error::<Test>::OrderingNotYourCommitment
        );

        // And a payload that hashes to *its own* label still cannot be swapped
        // into account 11's commitment.
        assert_noop!(
            PrivateExecution::reveal_ordering(
                RuntimeOrigin::signed(12),
                window_id,
                hash,
                b"attacker-payload".to_vec(),
                nonce,
            ),
            Error::<Test>::OrderingNotYourCommitment
        );
        assert_eq!(PrivateExecution::ordering_reveals(window_id, hash), None);
    });
}

#[test]
fn a_second_reveal_of_the_same_commitment_is_refused() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        let nonce = [1u8; 32];
        let hash = hash_for(11, b"payload", &nonce);
        commit(11, window_id, hash);
        reveal(11, window_id, hash, b"payload", &nonce);

        assert_noop!(
            PrivateExecution::reveal_ordering(
                RuntimeOrigin::signed(11),
                window_id,
                hash,
                b"payload".to_vec(),
                nonce,
            ),
            Error::<Test>::OrderingAlreadyRevealed
        );
        // The second attempt must not release the bond twice.
        assert_eq!(Balances::free_balance(11), 1_000_000);
        assert_eq!(
            PrivateExecution::ordering_windows(window_id)
                .unwrap()
                .reveal_count,
            1
        );
    });
}

#[test]
fn an_oversized_reveal_is_refused() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        let oversized = vec![0xAB; MAX_PLAINTEXT_BYTES + 1];

        assert_noop!(
            PrivateExecution::reveal_ordering(
                RuntimeOrigin::signed(11),
                window_id,
                H256::repeat_byte(0x77),
                oversized,
                [1u8; 32],
            ),
            Error::<Test>::OrderingPlaintextTooLarge
        );
    });
}

/// A window's *total* plaintext is bounded, not just each reveal: settling reads
/// every reveal in one transaction, so a window grown past what fits in a block
/// would be a window whose bonds can never be resolved.
#[test]
fn a_window_beyond_its_total_byte_budget_is_refused() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        // The mock window budget is 1_000 bytes.
        assert_eq!(MaxOrderingWindowBytes::get(), 1_000);

        let big = vec![0xCD; 600];
        let big_nonce = [1u8; 32];
        let big_hash = hash_for(10, &big, &big_nonce);
        commit(10, window_id, big_hash);
        reveal(10, window_id, big_hash, &big, &big_nonce);
        assert_eq!(
            PrivateExecution::ordering_windows(window_id)
                .unwrap()
                .revealed_bytes,
            600
        );

        // A second 600-byte reveal would take the window to 1_200.
        let hash_11 = hash_for(11, &big, &[2u8; 32]);
        commit(11, window_id, hash_11);
        assert_noop!(
            PrivateExecution::reveal_ordering(
                RuntimeOrigin::signed(11),
                window_id,
                hash_11,
                big.clone(),
                [2u8; 32],
            ),
            Error::<Test>::OrderingWindowBytesExceeded
        );
        assert_eq!(Balances::reserved_balance(11), BOND);
        assert_eq!(
            PrivateExecution::ordering_windows(window_id)
                .unwrap()
                .revealed_bytes,
            600
        );

        // Filling the budget exactly is allowed.
        let fill = vec![0xCD; 400];
        let fill_nonce = [3u8; 32];
        let fill_hash = hash_for(12, &fill, &fill_nonce);
        commit(12, window_id, fill_hash);
        reveal(12, window_id, fill_hash, &fill, &fill_nonce);
        assert_eq!(
            PrivateExecution::ordering_windows(window_id)
                .unwrap()
                .revealed_bytes,
            1_000
        );

        // One byte more is not.
        let one = vec![0xCD; 1];
        let one_nonce = [4u8; 32];
        let one_hash = hash_for(13, &one, &one_nonce);
        commit(13, window_id, one_hash);
        assert_noop!(
            PrivateExecution::reveal_ordering(
                RuntimeOrigin::signed(13),
                window_id,
                one_hash,
                one,
                one_nonce,
            ),
            Error::<Test>::OrderingWindowBytesExceeded
        );
        assert_eq!(Balances::reserved_balance(13), BOND);
    });
}

#[test]
fn settling_an_open_window_is_refused_and_settling_twice_is_refused() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        commit(11, window_id, hash_for(11, b"payload", &[1u8; 32]));

        System::set_block_number(CLOSE);
        assert_noop!(
            PrivateExecution::settle_ordering_window(RuntimeOrigin::signed(10), window_id),
            Error::<Test>::OrderingWindowStillOpen
        );
        assert!(
            !PrivateExecution::ordering_windows(window_id)
                .unwrap()
                .settled
        );

        System::set_block_number(CLOSE + 1);
        assert_ok!(PrivateExecution::settle_ordering_window(
            RuntimeOrigin::signed(10),
            window_id,
        ));
        assert_noop!(
            PrivateExecution::settle_ordering_window(RuntimeOrigin::signed(10), window_id),
            Error::<Test>::OrderingWindowSettled
        );
    });
}

#[test]
fn unknown_windows_and_settled_windows_refuse_further_work() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        commit(11, window_id, hash_for(11, b"payload", &[1u8; 32]));

        assert_noop!(
            PrivateExecution::commit_ordering(
                RuntimeOrigin::signed(12),
                window_id + 1,
                hash_for(12, b"payload", &[2u8; 32]),
                BOND,
            ),
            Error::<Test>::OrderingWindowNotFound
        );
        assert_noop!(
            PrivateExecution::reveal_ordering(
                RuntimeOrigin::signed(11),
                window_id + 1,
                H256::repeat_byte(0x01),
                b"payload".to_vec(),
                [1u8; 32],
            ),
            Error::<Test>::OrderingWindowNotFound
        );
        assert_noop!(
            PrivateExecution::settle_ordering_window(RuntimeOrigin::signed(10), window_id + 1),
            Error::<Test>::OrderingWindowNotFound
        );

        System::set_block_number(CLOSE + 1);
        assert_ok!(PrivateExecution::settle_ordering_window(
            RuntimeOrigin::signed(10),
            window_id,
        ));
        assert_noop!(
            PrivateExecution::commit_ordering(
                RuntimeOrigin::signed(12),
                window_id,
                hash_for(12, b"payload", &[2u8; 32]),
                BOND,
            ),
            Error::<Test>::OrderingWindowSettled
        );
    });
}

#[test]
fn a_beacon_cannot_be_installed_while_the_window_is_open_and_only_once() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        let beacon = H256::repeat_byte(0x42);

        System::set_block_number(CLOSE);
        assert_noop!(
            PrivateExecution::install_ordering_beacon(RuntimeOrigin::root(), window_id, beacon,),
            Error::<Test>::OrderingBeaconWhileOpen
        );
        assert_eq!(
            PrivateExecution::ordering_windows(window_id)
                .unwrap()
                .beacon,
            None
        );

        System::set_block_number(CLOSE + 1);
        assert_ok!(PrivateExecution::install_ordering_beacon(
            RuntimeOrigin::root(),
            window_id,
            beacon,
        ));
        assert_eq!(
            PrivateExecution::ordering_windows(window_id)
                .unwrap()
                .beacon,
            Some(beacon)
        );

        assert_noop!(
            PrivateExecution::install_ordering_beacon(
                RuntimeOrigin::root(),
                window_id,
                H256::repeat_byte(0x43),
            ),
            Error::<Test>::OrderingBeaconAlreadySet
        );
    });
}

/// The positive case the whole lane exists for: three commitments settle into
/// the canonical key order, and a verifier recomputes that order from storage.
#[test]
fn a_settled_window_is_the_canonical_key_order_and_recomputes_from_storage() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();

        let people: [(u64, &[u8], [u8; 32]); 3] = [
            (10, b"alpha", [1u8; 32]),
            (11, b"beta", [2u8; 32]),
            (12, b"gamma", [3u8; 32]),
        ];
        let mut entries = window_entries(&people);

        // Commit in the reverse of the canonical order, so a settle that used
        // arrival order would produce a visibly different sequence. With no
        // beacon the order key is the commit hash, so the canonical order is the
        // ascending sort of these three distinct hashes.
        entries.sort_by_key(|(hash, ..)| *hash);
        let canonical: Vec<H256> = entries.iter().map(|(hash, ..)| *hash).collect();
        let mut arrival = entries;
        arrival.reverse();
        let arrived: Vec<H256> = arrival.iter().map(|(hash, ..)| *hash).collect();
        assert_ne!(
            arrived, canonical,
            "the fixture must distinguish the two orders"
        );

        // Each commitment and its reveal land in its own block, so the arrival
        // order is unambiguous: it is exactly the order below.
        for (index, (hash, who, plaintext, nonce)) in arrival.into_iter().enumerate() {
            System::set_block_number(OPEN + index as u64);
            commit(who, window_id, hash);
            reveal(who, window_id, hash, plaintext, &nonce);
        }
        assert_eq!(
            PrivateExecution::ordering_windows(window_id)
                .unwrap()
                .reveal_count,
            3
        );

        System::set_block_number(CLOSE + 1);
        assert_ok!(PrivateExecution::settle_ordering_window(
            RuntimeOrigin::signed(10),
            window_id,
        ));

        let settlement = PrivateExecution::ordering_settlements(window_id).unwrap();
        assert_eq!(settlement.beacon, None);
        assert_eq!(settlement.unrevealed, Vec::<H256>::new());
        assert_eq!(settlement.forfeited_bond, 0);
        assert_eq!(settlement.ordered, canonical);
        assert_ne!(
            settlement.ordered, arrived,
            "arrival order must not decide the sequence"
        );

        // Recompute the order from the settlement alone, the way a verifier
        // would: order_key over the beacon, then sort.
        let mut recomputed: Vec<(H256, H256)> = settlement
            .ordered
            .iter()
            .map(|hash| (order_key(settlement.beacon, hash), *hash))
            .collect();
        recomputed.sort();
        let recomputed: Vec<H256> = recomputed.into_iter().map(|(_, hash)| hash).collect();
        assert_eq!(recomputed, settlement.ordered);

        // Every committed bond was released by its reveal.
        for (who, initial) in [(10u64, 10_000_000u128), (11, 1_000_000), (12, 1_000_000)] {
            assert_eq!(Balances::reserved_balance(who), 0);
            assert_eq!(Balances::free_balance(who), initial);
        }
    });
}

/// An installed beacon keys the order, and the settled sequence still recomputes.
#[test]
fn a_settled_window_uses_the_installed_beacon() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        let people: [(u64, &[u8], [u8; 32]); 4] = [
            (10, b"one", [1u8; 32]),
            (11, b"two", [2u8; 32]),
            (12, b"three", [3u8; 32]),
            (13, b"four", [4u8; 32]),
        ];
        let entries = window_entries(&people);
        for (hash, who, plaintext, nonce) in entries.iter().copied() {
            commit(who, window_id, hash);
            reveal(who, window_id, hash, plaintext, &nonce);
        }

        System::set_block_number(CLOSE + 1);
        // Pick a beacon that actually reorders this fixture by comparing against
        // a beacon-free recomputation; the search is over a fixed sequence.
        let mut without: Vec<(H256, H256)> = entries
            .iter()
            .map(|(hash, ..)| (order_key(None, hash), *hash))
            .collect();
        without.sort();

        let beacon = (1u8..=64)
            .map(H256::repeat_byte)
            .find(|candidate| {
                let mut with: Vec<(H256, H256)> = entries
                    .iter()
                    .map(|(hash, ..)| (order_key(Some(*candidate), hash), *hash))
                    .collect();
                with.sort();
                with != without
            })
            .expect("four hashes have 24 orders, so a reordering beacon exists");

        assert_ok!(PrivateExecution::install_ordering_beacon(
            RuntimeOrigin::root(),
            window_id,
            beacon,
        ));
        assert_ok!(PrivateExecution::settle_ordering_window(
            RuntimeOrigin::signed(10),
            window_id,
        ));

        let settlement = PrivateExecution::ordering_settlements(window_id).unwrap();
        assert_eq!(settlement.beacon, Some(beacon));

        let mut expected: Vec<(H256, H256)> = entries
            .iter()
            .map(|(hash, ..)| (order_key(Some(beacon), hash), *hash))
            .collect();
        expected.sort();
        let expected: Vec<H256> = expected.into_iter().map(|(_, hash)| hash).collect();
        assert_eq!(settlement.ordered, expected);
        assert_ne!(
            settlement.ordered,
            without
                .into_iter()
                .map(|(_, hash)| hash)
                .collect::<Vec<_>>(),
            "the beacon must be what the order is derived from"
        );
    });
}

/// A commitment that never reveals is excluded, and its bond is forfeited.
#[test]
fn an_unrevealed_commitment_is_excluded_and_its_bond_forfeited() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();

        let revealed_hash = hash_for(11, b"honest", &[1u8; 32]);
        let silent_hash = hash_for(12, b"silent", &[2u8; 32]);
        commit(11, window_id, revealed_hash);
        commit(12, window_id, silent_hash);
        reveal(11, window_id, revealed_hash, b"honest", &[1u8; 32]);

        assert_eq!(Balances::reserved_balance(12), BOND);

        System::set_block_number(CLOSE + 1);
        assert_ok!(PrivateExecution::settle_ordering_window(
            RuntimeOrigin::signed(10),
            window_id,
        ));

        let settlement = PrivateExecution::ordering_settlements(window_id).unwrap();
        assert_eq!(settlement.ordered, vec![revealed_hash]);
        assert_eq!(settlement.unrevealed, vec![silent_hash]);
        assert_eq!(settlement.forfeited_bond, BOND);

        // The forfeit is real: the silent commitment's bond is gone and its
        // reserve is empty. The honest participant is whole.
        assert_eq!(Balances::reserved_balance(12), 0);
        assert_eq!(Balances::free_balance(12), 1_000_000 - BOND);
        assert_eq!(Balances::reserved_balance(11), 0);
        assert_eq!(Balances::free_balance(11), 1_000_000);
    });
}

/// Disabling private execution stops new exposure but never traps a bond that
/// was taken while the guards held.
#[test]
fn disabling_private_execution_does_not_trap_committed_bonds() {
    new_test_ext().execute_with(|| {
        let window_id = open_window();
        let revealed_hash = hash_for(11, b"honest", &[1u8; 32]);
        let silent_hash = hash_for(12, b"silent", &[2u8; 32]);
        commit(11, window_id, revealed_hash);
        commit(12, window_id, silent_hash);

        assert_ok!(PrivateExecution::set_enabled(RuntimeOrigin::root(), false));

        // Reveal still releases the bond...
        reveal(11, window_id, revealed_hash, b"honest", &[1u8; 32]);
        assert_eq!(Balances::reserved_balance(11), 0);

        // ...and settle still forfeits the silent one rather than leaving it
        // reserved forever.
        System::set_block_number(CLOSE + 1);
        assert_ok!(PrivateExecution::settle_ordering_window(
            RuntimeOrigin::signed(10),
            window_id,
        ));
        assert_eq!(Balances::reserved_balance(12), 0);
        assert_eq!(
            PrivateExecution::ordering_settlements(window_id)
                .unwrap()
                .forfeited_bond,
            BOND
        );
    });
}
