//! Comprehensive tests for pallet-x3-wallet.
//!
//! Tests cover:
//! - Hardware wallet registration
//! - Multisig wallet creation
//! - Token transfers
//! - Biometric registration
//! - Error conditions

use crate::{mock::*, pallet::*};
use frame_support::{assert_noop, assert_ok};

// ============================================================================
// Hardware Wallet Tests
// ============================================================================

#[test]
fn register_hardware_wallet_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let device_type = 1u8; // e.g., Ledger
        let device_model = b"Ledger Nano X".to_vec();
        let public_key = [1u8; 32];

        assert_ok!(X3Wallet::register_hardware_wallet(
            RuntimeOrigin::signed(ALICE),
            device_type,
            device_model.clone(),
            public_key,
        ));

        // Check event emitted
        System::assert_has_event(RuntimeEvent::X3Wallet(Event::HardwareWalletConnected {
            account: ALICE,
            device_type,
        }));
    });
}

#[test]
fn register_multiple_hardware_wallets_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        // Register first wallet
        assert_ok!(X3Wallet::register_hardware_wallet(
            RuntimeOrigin::signed(ALICE),
            1,
            b"Ledger Nano X".to_vec(),
            [1u8; 32],
        ));

        // Register second wallet with different key
        assert_ok!(X3Wallet::register_hardware_wallet(
            RuntimeOrigin::signed(ALICE),
            2,
            b"Trezor Model T".to_vec(),
            [2u8; 32],
        ));
    });
}

// ============================================================================
// Multisig Wallet Tests
// ============================================================================

#[test]
fn create_multisig_wallet_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let signers: Vec<[u8; 32]> = vec![[1u8; 32], [2u8; 32], [3u8; 32]];

        assert_ok!(X3Wallet::create_multisig_wallet(
            RuntimeOrigin::signed(ALICE),
            signers,
            2,    // 2-of-3 threshold
            3600, // 1 hour timelock
        ));

        System::assert_has_event(RuntimeEvent::X3Wallet(Event::MultisigWalletCreated {
            account: ALICE,
            threshold: 2,
        }));
    });
}

#[test]
fn create_multisig_wallet_fails_with_invalid_threshold() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let signers: Vec<[u8; 32]> = vec![[1u8; 32], [2u8; 32]];

        // Threshold 0 is invalid
        assert_noop!(
            X3Wallet::create_multisig_wallet(
                RuntimeOrigin::signed(ALICE),
                signers.clone(),
                0, // Invalid threshold
                3600,
            ),
            Error::<Test>::InvalidThreshold
        );

        // Threshold > number of signers is invalid
        assert_noop!(
            X3Wallet::create_multisig_wallet(
                RuntimeOrigin::signed(ALICE),
                signers,
                5, // Greater than 2 signers
                3600,
            ),
            Error::<Test>::InvalidThreshold
        );
    });
}

// ============================================================================
// Token Transfer Tests
// ============================================================================

#[test]
fn transfer_tokens_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let token_id = [42u8; 32];

        // First mint some tokens to ALICE
        assert_ok!(X3Wallet::mint_tokens(
            RuntimeOrigin::signed(ALICE), // admin
            token_id,
            ALICE,
            1000,
        ));

        // Transfer some tokens to BOB
        assert_ok!(X3Wallet::transfer_tokens(
            RuntimeOrigin::signed(ALICE),
            token_id,
            BOB,
            300,
        ));

        // Check balances
        assert_eq!(X3Wallet::get_token_balance(&ALICE, &token_id), 700);
        assert_eq!(X3Wallet::get_token_balance(&BOB, &token_id), 300);
    });
}

#[test]
fn transfer_tokens_fails_with_zero_amount() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let token_id = [42u8; 32];

        assert_noop!(
            X3Wallet::transfer_tokens(
                RuntimeOrigin::signed(ALICE),
                token_id,
                BOB,
                0, // Invalid amount
            ),
            Error::<Test>::InvalidAmount
        );
    });
}

#[test]
fn transfer_tokens_fails_with_insufficient_balance() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let token_id = [42u8; 32];

        // Mint 100 tokens to ALICE
        assert_ok!(X3Wallet::mint_tokens(
            RuntimeOrigin::signed(ALICE),
            token_id,
            ALICE,
            100,
        ));

        // Try to transfer more than balance
        assert_noop!(
            X3Wallet::transfer_tokens(
                RuntimeOrigin::signed(ALICE),
                token_id,
                BOB,
                200, // More than balance
            ),
            Error::<Test>::InsufficientBalance
        );
    });
}

// ============================================================================
// Biometric Registration Tests
// ============================================================================

#[test]
fn register_biometric_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let biometric_type = 1u8; // e.g., fingerprint
        let template_hash = [1u8; 32];
        let pin_hash = [2u8; 32];

        assert_ok!(X3Wallet::register_biometric(
            RuntimeOrigin::signed(ALICE),
            biometric_type,
            template_hash,
            pin_hash,
        ));

        // Check profile was created
        let profile =
            X3Wallet::get_biometric_profile(&ALICE).expect("biometric profile should exist");
        assert_eq!(profile.biometric_type, biometric_type);
        assert_eq!(profile.template_hash, template_hash);
        assert!(profile.is_enabled);
        // The profile names the account that registered it. It used to be hardcoded to
        // `[0u8; 32]`, so the stored record did not identify its owner at all.
        assert_eq!(
            profile.owner,
            X3Wallet::account_bytes(&ALICE),
            "the profile must be owned by the signer"
        );
        assert_eq!(profile.attempts_remaining, 5);

        System::assert_has_event(RuntimeEvent::X3Wallet(Event::BiometricProfileCreated {
            account: ALICE,
        }));
    });
}

/// The pallet builds the profile through `BiometricManager`, so the library's rules apply here.
/// Before 2026-09-26 this extrinsic stored whatever it was given.
#[test]
fn register_biometric_refuses_an_unsupported_type() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_noop!(
            X3Wallet::register_biometric(RuntimeOrigin::signed(ALICE), 3, [1u8; 32], [2u8; 32]),
            Error::<Test>::InvalidBiometricType
        );
    });
}

#[test]
fn register_biometric_refuses_an_empty_template_hash() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_noop!(
            X3Wallet::register_biometric(RuntimeOrigin::signed(ALICE), 1, [0u8; 32], [2u8; 32]),
            Error::<Test>::EmptyTemplateHash
        );
    });
}

#[test]
fn register_biometric_refuses_an_empty_pin_hash() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_noop!(
            X3Wallet::register_biometric(RuntimeOrigin::signed(ALICE), 1, [1u8; 32], [0u8; 32]),
            Error::<Test>::EmptyPinHash
        );
    });
}

// ============================================================================
// Recovery Tests
// ============================================================================

#[test]
fn initiate_recovery_fails_without_guardian() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        // Try to initiate recovery without having set up guardians
        assert_noop!(
            X3Wallet::initiate_recovery(
                RuntimeOrigin::signed(ALICE),
                ALICE,
                [3u8; 32], // new owner
            ),
            Error::<Test>::WalletNotFound
        );
    });
}

#[test]
fn register_recovery_guardians_applies_the_library_rules() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        // An empty set, a zero threshold, and a threshold the set cannot meet are all refused.
        assert_noop!(
            X3Wallet::register_recovery_guardians(RuntimeOrigin::signed(ALICE), vec![], 1, 100),
            Error::<Test>::InvalidGuardianSet
        );
        assert_noop!(
            X3Wallet::register_recovery_guardians(
                RuntimeOrigin::signed(ALICE),
                vec![X3Wallet::account_bytes(&BOB)],
                0,
                100
            ),
            Error::<Test>::InvalidGuardianSet
        );
        assert_noop!(
            X3Wallet::register_recovery_guardians(
                RuntimeOrigin::signed(ALICE),
                vec![X3Wallet::account_bytes(&BOB)],
                2,
                100
            ),
            Error::<Test>::InvalidGuardianSet
        );
        // 31 guardians is past the library's cap of 30.
        assert_noop!(
            X3Wallet::register_recovery_guardians(
                RuntimeOrigin::signed(ALICE),
                (1u8..=31).map(|b| [b; 32]).collect(),
                1,
                100
            ),
            Error::<Test>::InvalidGuardianSet
        );

        // A valid set is stored, owned by the signer.
        assert_ok!(X3Wallet::register_recovery_guardians(
            RuntimeOrigin::signed(ALICE),
            vec![
                X3Wallet::account_bytes(&BOB),
                X3Wallet::account_bytes(&_CHARLIE)
            ],
            2,
            100
        ));

        let stored = X3Wallet::get_recovery_account(&ALICE).expect("guardian set stored");
        assert_eq!(stored.owner, X3Wallet::account_bytes(&ALICE));
        assert_eq!(stored.guardians.len(), 2);
        assert_eq!(stored.required_guardians, 2);
        assert_eq!(stored.recovery_delay_blocks, 100);
        assert!(stored.is_active);

        System::assert_has_event(RuntimeEvent::X3Wallet(Event::RecoveryGuardiansRegistered {
            account: ALICE,
            guardians: 2,
            required: 2,
            delay_blocks: 100,
        }));

        // A second registration is refused rather than silently replacing the set.
        assert_noop!(
            X3Wallet::register_recovery_guardians(
                RuntimeOrigin::signed(ALICE),
                vec![X3Wallet::account_bytes(&BOB)],
                1,
                10
            ),
            Error::<Test>::RecoveryAccountExists
        );
    });
}

// The old `initiate_recovery_works` called `initiate_recovery` as ALICE — the account being
// recovered — against a guardian record the test had inserted straight into storage, and accepted
// an event that changed nothing: no guardians, no quorum, no delay, no state transition. The
// account owner is refused now, and the three tests below are a completed lifecycle instead of an
// announcement.

#[test]
fn the_account_owner_cannot_initiate_recovery() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(X3Wallet::register_recovery_guardians(
            RuntimeOrigin::signed(ALICE),
            vec![
                X3Wallet::account_bytes(&BOB),
                X3Wallet::account_bytes(&_CHARLIE)
            ],
            2,
            100
        ));

        assert_noop!(
            X3Wallet::initiate_recovery(RuntimeOrigin::signed(ALICE), ALICE, [9u8; 32]),
            Error::<Test>::NotGuardian
        );
        assert!(X3Wallet::get_recovery_request(&ALICE).is_none());
    });
}

#[test]
fn recovery_runs_from_guardian_request_to_owner_change() {
    new_test_ext().execute_with(|| {
        System::set_block_number(10);
        let new_owner = [9u8; 32];
        assert_ok!(X3Wallet::register_recovery_guardians(
            RuntimeOrigin::signed(ALICE),
            vec![
                X3Wallet::account_bytes(&BOB),
                X3Wallet::account_bytes(&_CHARLIE)
            ],
            2,
            100
        ));

        // A guardian starts it. The request records the delay instead of announcing a new owner.
        assert_ok!(X3Wallet::initiate_recovery(
            RuntimeOrigin::signed(BOB),
            ALICE,
            new_owner
        ));
        let request = X3Wallet::get_recovery_request(&ALICE).expect("request stored");
        assert_eq!(request.status, 0, "a fresh request is pending");
        assert_eq!(request.new_owner, new_owner);
        assert_eq!(request.executable_block, 110);
        assert!(request.approvals.is_empty());

        // A second request while one is pending is refused.
        assert_noop!(
            X3Wallet::initiate_recovery(RuntimeOrigin::signed(_CHARLIE), ALICE, [8u8; 32]),
            Error::<Test>::RecoveryAlreadyPending
        );

        // A non-guardian cannot approve.
        assert_noop!(
            X3Wallet::approve_recovery(RuntimeOrigin::signed(ALICE), ALICE),
            Error::<Test>::NotGuardian
        );

        // One approval is below the threshold of two: nothing may finalize yet.
        assert_ok!(X3Wallet::approve_recovery(
            RuntimeOrigin::signed(BOB),
            ALICE
        ));
        assert_noop!(
            X3Wallet::finalize_recovery(RuntimeOrigin::signed(BOB), ALICE),
            Error::<Test>::RecoveryNotApproved
        );
        // The same guardian cannot count twice.
        assert_noop!(
            X3Wallet::approve_recovery(RuntimeOrigin::signed(BOB), ALICE),
            Error::<Test>::DuplicateGuardianApproval
        );

        // The threshold is met, but the delay has not elapsed: still refused.
        assert_ok!(X3Wallet::approve_recovery(
            RuntimeOrigin::signed(_CHARLIE),
            ALICE
        ));
        System::set_block_number(109);
        assert_noop!(
            X3Wallet::finalize_recovery(RuntimeOrigin::signed(BOB), ALICE),
            Error::<Test>::RecoveryNotReady
        );
        assert_eq!(
            X3Wallet::get_recovery_account(&ALICE).unwrap().owner,
            X3Wallet::account_bytes(&ALICE),
            "the owner must not change before the delay elapses"
        );

        // After the delay the recovery executes and the stored owner changes.
        System::set_block_number(110);
        assert_ok!(X3Wallet::finalize_recovery(
            RuntimeOrigin::signed(BOB),
            ALICE
        ));
        assert_eq!(
            X3Wallet::get_recovery_account(&ALICE).unwrap().owner,
            new_owner
        );
        assert_eq!(
            X3Wallet::get_recovery_request(&ALICE).unwrap().status,
            2,
            "an executed request is marked executed"
        );
        System::assert_has_event(RuntimeEvent::X3Wallet(Event::RecoveryExecuted {
            account: ALICE,
            previous_owner: X3Wallet::account_bytes(&ALICE),
            new_owner,
        }));
    });
}

#[test]
fn the_owner_can_cancel_a_pending_recovery() {
    new_test_ext().execute_with(|| {
        System::set_block_number(10);
        assert_ok!(X3Wallet::register_recovery_guardians(
            RuntimeOrigin::signed(ALICE),
            vec![X3Wallet::account_bytes(&BOB)],
            1,
            100
        ));
        assert_ok!(X3Wallet::initiate_recovery(
            RuntimeOrigin::signed(BOB),
            ALICE,
            [9u8; 32]
        ));

        // A caller who is neither the recovery owner nor an approver cannot cancel.
        assert_noop!(
            X3Wallet::cancel_recovery(RuntimeOrigin::signed(_CHARLIE), ALICE),
            Error::<Test>::NotRecoveryOwner
        );

        assert_ok!(X3Wallet::cancel_recovery(
            RuntimeOrigin::signed(ALICE),
            ALICE
        ));
        assert!(X3Wallet::get_recovery_request(&ALICE).is_none());
        assert_eq!(
            X3Wallet::get_recovery_account(&ALICE).unwrap().owner,
            X3Wallet::account_bytes(&ALICE),
            "cancelling must leave the owner untouched"
        );
    });
}

// ============================================================================
// Token Minting Tests
// ============================================================================

#[test]
fn mint_tokens_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let token_id = [42u8; 32];

        assert_ok!(X3Wallet::mint_tokens(
            RuntimeOrigin::signed(ALICE),
            token_id,
            BOB,
            5000,
        ));

        assert_eq!(X3Wallet::get_token_balance(&BOB, &token_id), 5000);

        System::assert_has_event(RuntimeEvent::X3Wallet(Event::BalanceUpdated {
            account: BOB,
            token_id,
            amount: 5000,
        }));
    });
}

#[test]
fn mint_tokens_accumulates() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let token_id = [42u8; 32];

        assert_ok!(X3Wallet::mint_tokens(
            RuntimeOrigin::signed(ALICE),
            token_id,
            BOB,
            1000,
        ));

        assert_ok!(X3Wallet::mint_tokens(
            RuntimeOrigin::signed(ALICE),
            token_id,
            BOB,
            500,
        ));

        assert_eq!(X3Wallet::get_token_balance(&BOB, &token_id), 1500);
    });
}

// ============================================================================
// Minter Authorization Tests
// ============================================================================

#[test]
fn mint_tokens_authorized_only() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let token_id = [42u8; 32];

        // BOB is not an authorized minter — mint should fail.
        assert_noop!(
            X3Wallet::mint_tokens(RuntimeOrigin::signed(BOB), token_id, BOB, 100,),
            Error::<Test>::Unauthorized
        );
    });
}

#[test]
fn add_remove_minter_root_only() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        // Non-root (signed) call to add_minter must fail
        assert_noop!(
            X3Wallet::add_minter(RuntimeOrigin::signed(ALICE), BOB),
            frame_support::error::BadOrigin
        );

        // Non-root (signed) call to remove_minter must fail
        assert_noop!(
            X3Wallet::remove_minter(RuntimeOrigin::signed(ALICE), ALICE),
            frame_support::error::BadOrigin
        );

        // Root can add BOB as minter
        assert_ok!(X3Wallet::add_minter(RuntimeOrigin::root(), BOB));

        // BOB should now be an authorized minter
        assert!(crate::Minters::<Test>::contains_key(BOB));

        // Root can remove ALICE as minter
        assert_ok!(X3Wallet::remove_minter(RuntimeOrigin::root(), ALICE));

        // ALICE should no longer be authorized
        assert!(!crate::Minters::<Test>::contains_key(ALICE));

        // ALICE can no longer mint after removal
        assert_noop!(
            X3Wallet::mint_tokens(RuntimeOrigin::signed(ALICE), [42u8; 32], BOB, 100,),
            Error::<Test>::Unauthorized
        );
    });
}

// ============================================================================
// Storage Query Tests
// ============================================================================

#[test]
fn storage_queries_return_none_for_missing_data() {
    new_test_ext().execute_with(|| {
        assert!(X3Wallet::get_hardware_wallet(&ALICE, &[0u8; 32]).is_none());
        assert!(X3Wallet::get_multisig_wallet(&ALICE, &[0u8; 32]).is_none());
        assert!(X3Wallet::get_biometric_profile(&ALICE).is_none());
        assert!(X3Wallet::get_recovery_account(&ALICE).is_none());
        assert_eq!(X3Wallet::get_token_balance(&ALICE, &[0u8; 32]), 0);
    });
}
