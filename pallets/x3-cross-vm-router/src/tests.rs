// SPDX-License-Identifier: Apache-2.0
//
// Mock runtime + acceptance tests for the X3 Universal Asset Kernel MVP.
//
// This harness wires together the three kernel pallets — registry, supply
// ledger, cross-VM router — inside a minimal Substrate runtime and exercises
// the golden-path round-trip and the six-route matrix.
//
// The **one** test that matters: `test_x3_native_evm_svm_roundtrip_preserves_supply`.

use crate as pallet_x3_cross_vm_router;
use frame_support::{
    assert_ok, construct_runtime, derive_impl, parameter_types,
    traits::{ConstU32, EnsureOrigin},
};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};
use x3_asset_kernel_types::{
    AccountBytes, AssetId, DomainId, ProofTier, RouteConfig, RouteLimits, SupplyPolicy,
};

type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Registry: pallet_x3_asset_registry,
        Ledger: pallet_x3_supply_ledger,
        Router: pallet_x3_cross_vm_router,
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const SS58Prefix: u8 = 42;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = SS58Prefix;
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

// Root-or-signed passthrough: any signed origin counts as governance in tests.
pub struct RootOrAny;
impl EnsureOrigin<RuntimeOrigin> for RootOrAny {
    type Success = ();
    fn try_origin(o: RuntimeOrigin) -> Result<(), RuntimeOrigin> {
        match o.clone().into() {
            Ok(system::RawOrigin::Root) => Ok(()),
            Ok(system::RawOrigin::Signed(_)) => Ok(()),
            _ => Err(o),
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RuntimeOrigin::root())
    }
}

parameter_types! {
    pub const MaxAssets: u32 = 64;
}

impl pallet_x3_asset_registry::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RegistryOrigin = RootOrAny;
    type EmergencyPauseOrigin = RootOrAny;
    type MaxAssets = MaxAssets;
}

impl pallet_x3_supply_ledger::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type SupplyGovernance = RootOrAny;
    type Registry = Registry;
}

impl pallet_x3_cross_vm_router::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Registry = Registry;
    type Ledger = Ledger;
    type ExternalExecutorOrigin = RootOrAny;
}

fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext: sp_io::TestExternalities = t.into();
    ext.execute_with(|| System::set_block_number(1));
    ext
}

// ── Fixtures ──────────────────────────────────────────────────────────────

/// Alice on X3Native.
fn alice_native() -> AccountBytes {
    AccountBytes::X3Native([1u8; 32])
}
/// Alice's EVM-side address.
fn alice_evm() -> AccountBytes {
    AccountBytes::Evm([2u8; 20])
}
/// Alice's SVM-side address.
fn alice_svm() -> AccountBytes {
    AccountBytes::Svm([3u8; 32])
}

fn permissive_route() -> RouteConfig {
    RouteConfig {
        enabled: true,
        limits: RouteLimits::DEV_PERMISSIVE,
        fee_bps: 0,
        expiry_blocks: 100,
        proof_tier: ProofTier::TrustedInternal,
    }
}

/// Register X3 as a native-mint-burn asset across all three internal domains,
/// enable all six internal routes, mint `supply` into the native leg.
fn bootstrap_x3_asset(supply: u128) -> AssetId {
    // Register.
    Registry::register_asset(
        RuntimeOrigin::root(),
        b"X3".to_vec(),
        b"X3 Token".to_vec(),
        12,
        DomainId::X3Native,
        0,
        b"native".to_vec(),
        SupplyPolicy::NativeMintBurn,
    )
    .expect("register_asset");

    // Recompute the same asset id the pallet derived.
    let asset_id =
        x3_asset_kernel_types::derive_asset_id(DomainId::X3Native, 0, b"native", b"X3", 12);

    Registry::activate_asset(RuntimeOrigin::root(), asset_id).unwrap();

    // Enable all six internal routes.
    for (src, dst) in [
        (DomainId::X3Native, DomainId::X3Evm),
        (DomainId::X3Evm, DomainId::X3Native),
        (DomainId::X3Native, DomainId::X3Svm),
        (DomainId::X3Svm, DomainId::X3Native),
        (DomainId::X3Evm, DomainId::X3Svm),
        (DomainId::X3Svm, DomainId::X3Evm),
    ] {
        Registry::configure_route(
            RuntimeOrigin::root(),
            asset_id,
            src,
            dst,
            permissive_route(),
        )
        .unwrap();
    }

    // Mint canonical supply into the native leg.
    // `mint_canonical` requires a signed origin after governance check.
    Ledger::mint_canonical(
        RuntimeOrigin::signed(1),
        asset_id,
        DomainId::X3Native,
        supply,
        0u64,
    )
    .unwrap();

    asset_id
}

fn addr_for(domain: DomainId) -> AccountBytes {
    match domain {
        DomainId::X3Native => alice_native(),
        DomainId::X3Evm => alice_evm(),
        DomainId::X3Svm => alice_svm(),
        _ => unreachable!("MVP only uses internal domains"),
    }
}

fn do_xvm(asset_id: AssetId, src: DomainId, dst: DomainId, amount: u128) -> H256 {
    let sender = addr_for(src);
    let recipient = addr_for(dst);
    let now = System::block_number();
    let expires_at = now + 50;

    Router::xvm_transfer(
        RuntimeOrigin::signed(1),
        asset_id,
        src,
        dst,
        sender.clone(),
        recipient.clone(),
        amount,
        expires_at,
    )
    .expect("xvm_transfer");

    // P0 Optimization (batch nonce): With batch pre-allocation, we need to
    // derive which nonce was actually used. Read the batch allocation that
    // was created/updated by reserve_nonce_from_batch.
    let nonce = if let Some((batch_start, _batch_size, used_count)) =
        Router::nonce_batch_allocation(src, sender.clone())
    {
        // The nonce that was just used is at (used_count - 1) within the batch
        batch_start.saturating_add((used_count.saturating_sub(1)) as u128)
    } else {
        // Fallback (shouldn't happen after successful xvm_transfer)
        0
    };

    // Rebuild the message exactly as the router did, then rederive id.
    let msg = x3_asset_kernel_types::X3TransferMessage::<u64> {
        version: x3_asset_kernel_types::MESSAGE_FORMAT_VERSION,
        asset_id,
        source_domain: src,
        destination_domain: dst,
        sender,
        recipient,
        amount,
        nonce,
        created_at: now,
        expires_at,
    };
    let message_id = x3_asset_kernel_types::derive_message_id::<u64>(&msg);

    Router::complete_xvm_transfer(RuntimeOrigin::signed(1), message_id).expect("complete");
    message_id
}

// ============================================================================
// PHASE 1.4 CROSS-VM ROUTER TESTS - ENABLED FOR MVP
// ============================================================================
//
// These tests validate the six-route matrix, replay protection, state
// machine transitions, and error handling for the internal cross-VM router.
//
// Test Progression:
// 1. Golden-path: test_x3_native_evm_svm_roundtrip_preserves_supply
// 2. Six-route matrix: test_all_six_internal_routes_succeed
// 3. Negative tests: incompatibility, zero amount, paused asset, etc.
// 4. Replay protection: duplicate messages and nonce ordering
// 5. Expiry handling: cancellations and refunds
// 6. Fuzz: random sequences preserve supply invariant

#[test]
fn test_x3_native_evm_svm_roundtrip_preserves_supply() {
    new_test_ext().execute_with(|| {
        // 1 billion units canonical supply.
        let asset_id = bootstrap_x3_asset(1_000_000_000);

        // Sanity: entire supply sits on the native leg.
        let l0 = Ledger::ledgers(asset_id).unwrap();
        assert_eq!(l0.canonical_supply, 1_000_000_000);
        assert_eq!(l0.native_supply, 1_000_000_000);
        assert_eq!(l0.evm_supply, 0);
        assert_eq!(l0.svm_supply, 0);
        assert_eq!(l0.pending_supply, 0);
        l0.check_invariant().unwrap();

        // Native → EVM 250
        do_xvm(asset_id, DomainId::X3Native, DomainId::X3Evm, 250);
        let l1 = Ledger::ledgers(asset_id).unwrap();
        assert_eq!(l1.native_supply, 1_000_000_000 - 250);
        assert_eq!(l1.evm_supply, 250);
        assert_eq!(l1.svm_supply, 0);
        assert_eq!(l1.pending_supply, 0);
        l1.check_invariant().unwrap();

        // EVM → SVM 100
        do_xvm(asset_id, DomainId::X3Evm, DomainId::X3Svm, 100);
        let l2 = Ledger::ledgers(asset_id).unwrap();
        assert_eq!(l2.native_supply, 1_000_000_000 - 250);
        assert_eq!(l2.evm_supply, 150);
        assert_eq!(l2.svm_supply, 100);
        assert_eq!(l2.pending_supply, 0);
        l2.check_invariant().unwrap();

        // SVM → Native 50
        do_xvm(asset_id, DomainId::X3Svm, DomainId::X3Native, 50);
        let l3 = Ledger::ledgers(asset_id).unwrap();
        assert_eq!(l3.native_supply, 1_000_000_000 - 250 + 50);
        assert_eq!(l3.evm_supply, 150);
        assert_eq!(l3.svm_supply, 50);
        assert_eq!(l3.pending_supply, 0);

        // Canonical supply never changed.
        assert_eq!(l3.canonical_supply, 1_000_000_000);
        // King invariant still holds.
        l3.check_invariant().unwrap();
        // Represented == canonical (nothing minted or burned).
        assert_eq!(l3.represented().unwrap(), l3.canonical_supply);
    });
}

// ── Six-route matrix ──────────────────────────────────────────────────────

#[test]
fn test_all_six_internal_routes_succeed() {
    new_test_ext().execute_with(|| {
        let asset_id = bootstrap_x3_asset(10_000);

        // Seed each domain with enough balance to move from it.
        // Start: 10_000 on native, 0 elsewhere. Preload EVM and SVM.
        do_xvm(asset_id, DomainId::X3Native, DomainId::X3Evm, 1_000);
        do_xvm(asset_id, DomainId::X3Native, DomainId::X3Svm, 1_000);

        // Exercise each of the 6 routes.
        for (src, dst) in [
            (DomainId::X3Native, DomainId::X3Evm),
            (DomainId::X3Evm, DomainId::X3Native),
            (DomainId::X3Native, DomainId::X3Svm),
            (DomainId::X3Svm, DomainId::X3Native),
            (DomainId::X3Evm, DomainId::X3Svm),
            (DomainId::X3Svm, DomainId::X3Evm),
        ] {
            do_xvm(asset_id, src, dst, 10);
            let l = Ledger::ledgers(asset_id).unwrap();
            l.check_invariant().unwrap();
            assert_eq!(l.pending_supply, 0);
        }

        // Canonical unchanged.
        let l = Ledger::ledgers(asset_id).unwrap();
        assert_eq!(l.canonical_supply, 10_000);
        assert_eq!(l.represented().unwrap(), 10_000);
    });
}

// ── Negative tests ────────────────────────────────────────────────────────

#[test]
fn test_duplicate_message_replay_rejected() {
    new_test_ext().execute_with(|| {
        let asset_id = bootstrap_x3_asset(10_000);

        // Manually build + submit a transfer to capture the message id.
        let now = System::block_number();
        let sender = alice_native();
        let recipient = alice_evm();
        let nonce = Router::next_nonce(DomainId::X3Native, sender.clone());
        let expires_at = now + 50;

        Router::xvm_transfer(
            RuntimeOrigin::signed(1),
            asset_id,
            DomainId::X3Native,
            DomainId::X3Evm,
            sender.clone(),
            recipient.clone(),
            100,
            expires_at,
        )
        .unwrap();

        let msg = x3_asset_kernel_types::X3TransferMessage::<u64> {
            version: x3_asset_kernel_types::MESSAGE_FORMAT_VERSION,
            asset_id,
            source_domain: DomainId::X3Native,
            destination_domain: DomainId::X3Evm,
            sender,
            recipient,
            amount: 100,
            nonce,
            created_at: now,
            expires_at,
        };
        let message_id = x3_asset_kernel_types::derive_message_id::<u64>(&msg);

        // First completion succeeds.
        Router::complete_xvm_transfer(RuntimeOrigin::signed(1), message_id).unwrap();

        // Second completion must fail — state is now Finalized, not SourceDebited.
        assert!(
            Router::complete_xvm_transfer(RuntimeOrigin::signed(1), message_id).is_err(),
            "re-completing a finalized transfer must fail"
        );
    });
}

#[test]
fn test_paused_asset_rejects_transfers() {
    new_test_ext().execute_with(|| {
        let asset_id = bootstrap_x3_asset(10_000);
        Registry::pause_asset(RuntimeOrigin::root(), asset_id).unwrap();

        let r = Router::xvm_transfer(
            RuntimeOrigin::signed(1),
            asset_id,
            DomainId::X3Native,
            DomainId::X3Evm,
            alice_native(),
            alice_evm(),
            10,
            60,
        );
        assert!(r.is_err(), "paused asset must reject transfers");
    });
}

#[test]
fn test_closed_route_rejects_transfers() {
    new_test_ext().execute_with(|| {
        let asset_id = bootstrap_x3_asset(10_000);
        Registry::set_route_enabled(
            RuntimeOrigin::root(),
            asset_id,
            DomainId::X3Native,
            DomainId::X3Evm,
            false,
        )
        .unwrap();

        let r = Router::xvm_transfer(
            RuntimeOrigin::signed(1),
            asset_id,
            DomainId::X3Native,
            DomainId::X3Evm,
            alice_native(),
            alice_evm(),
            10,
            60,
        );
        assert!(r.is_err(), "disabled route must reject transfers");
    });
}

#[test]
fn test_zero_amount_rejected() {
    new_test_ext().execute_with(|| {
        let asset_id = bootstrap_x3_asset(10_000);
        let r = Router::xvm_transfer(
            RuntimeOrigin::signed(1),
            asset_id,
            DomainId::X3Native,
            DomainId::X3Evm,
            alice_native(),
            alice_evm(),
            0,
            60,
        );
        assert!(r.is_err(), "zero amount must be rejected");
    });
}

#[test]
fn test_incompatible_recipient_rejected() {
    new_test_ext().execute_with(|| {
        let asset_id = bootstrap_x3_asset(10_000);
        // Native→Evm but recipient is an SVM key: must fail.
        let r = Router::xvm_transfer(
            RuntimeOrigin::signed(1),
            asset_id,
            DomainId::X3Native,
            DomainId::X3Evm,
            alice_native(),
            alice_svm(), // wrong type for X3Evm
            10,
            60,
        );
        assert!(
            r.is_err(),
            "EVM destination with SVM recipient must be rejected"
        );
    });
}

#[test]
fn test_expired_transfer_refunds_to_source() {
    new_test_ext().execute_with(|| {
        let asset_id = bootstrap_x3_asset(10_000);

        let now = System::block_number();
        let sender = alice_native();
        let recipient = alice_evm();
        let nonce = Router::next_nonce(DomainId::X3Native, sender.clone());
        let expires_at = now + 5;

        Router::xvm_transfer(
            RuntimeOrigin::signed(1),
            asset_id,
            DomainId::X3Native,
            DomainId::X3Evm,
            sender.clone(),
            recipient.clone(),
            100,
            expires_at,
        )
        .unwrap();

        // Advance past expiry.
        System::set_block_number(expires_at + 1);

        let msg = x3_asset_kernel_types::X3TransferMessage::<u64> {
            version: x3_asset_kernel_types::MESSAGE_FORMAT_VERSION,
            asset_id,
            source_domain: DomainId::X3Native,
            destination_domain: DomainId::X3Evm,
            sender,
            recipient,
            amount: 100,
            nonce,
            created_at: now,
            expires_at,
        };
        let message_id = x3_asset_kernel_types::derive_message_id::<u64>(&msg);

        Router::cancel_expired_xvm_transfer(RuntimeOrigin::signed(1), message_id).unwrap();

        let l = Ledger::ledgers(asset_id).unwrap();
        // Supply fully returned to native leg; pending zero.
        assert_eq!(l.native_supply, 10_000);
        assert_eq!(l.evm_supply, 0);
        assert_eq!(l.pending_supply, 0);
        l.check_invariant().unwrap();
    });
}

#[test]
fn test_cannot_cancel_before_expiry() {
    new_test_ext().execute_with(|| {
        let asset_id = bootstrap_x3_asset(10_000);

        let now = System::block_number();
        let sender = alice_native();
        let recipient = alice_evm();
        let nonce = Router::next_nonce(DomainId::X3Native, sender.clone());
        let expires_at = now + 50;

        Router::xvm_transfer(
            RuntimeOrigin::signed(1),
            asset_id,
            DomainId::X3Native,
            DomainId::X3Evm,
            sender.clone(),
            recipient.clone(),
            100,
            expires_at,
        )
        .unwrap();

        let msg = x3_asset_kernel_types::X3TransferMessage::<u64> {
            version: x3_asset_kernel_types::MESSAGE_FORMAT_VERSION,
            asset_id,
            source_domain: DomainId::X3Native,
            destination_domain: DomainId::X3Evm,
            sender,
            recipient,
            amount: 100,
            nonce,
            created_at: now,
            expires_at,
        };
        let message_id = x3_asset_kernel_types::derive_message_id::<u64>(&msg);

        // Still in-flight; cancel must refuse.
        assert!(
            Router::cancel_expired_xvm_transfer(RuntimeOrigin::signed(1), message_id).is_err(),
            "cancel before expiry must fail"
        );
    });
}

#[test]
fn test_external_route_rejected_in_mvp() {
    new_test_ext().execute_with(|| {
        let asset_id = bootstrap_x3_asset(10_000);
        let r = Router::xvm_transfer(
            RuntimeOrigin::signed(1),
            asset_id,
            DomainId::X3Native,
            DomainId::Ethereum,
            alice_native(),
            AccountBytes::Evm([9u8; 20]),
            10,
            60,
        );
        assert!(r.is_err(), "external routes must be rejected in MVP");
    });
}

// ============================================================================
// ADVANCED CROSS-VM ROUTER TESTS - DEEPER COVERAGE [ARCHIVED]
// ============================================================================
//
// The following tests were archived because they reference the old router
// API that was refactored in Phase 1.4:
//
// Removed:
// - duplicate_message_replay_attack_multiple_attempts
// - all_six_internal_routes_state_independent
// - asset_with_minimum_canonical_supply_boundary
// - asset_with_maximum_canonical_supply_boundary
// - transfer_ledger_state_consistency_after_multiple_operations
// - bridge_pause_prevents_all_route_types
// - events_emitted_for_critical_operations
// - fuzz_random_transfer_sequence_preserves_invariant (64 seeds, PRNG)
// - fuzz_large_value_transfers_preserve_invariant (u128::MAX/2 stress)
//
// These tests should be rewritten using:
// - xvm_transfer() / complete_xvm_transfer() / cancel_expired_xvm_transfer()
// - X3TransferMessage instead of TransferReceipt
// - DomainId pairs instead of RouteKey/InternalRoute
// - do_xvm() helper function
//
// Reference implementations:
// - test_x3_native_evm_svm_roundtrip_preserves_supply (golden path)
// - test_all_six_internal_routes_succeed (six-route matrix)
// - test_duplicate_message_replay_rejected (replay protection)
// - test_expired_transfer_refunds_to_source (expiry handling)
//
// Future developers: See PHASE_1_4_REFERENCE_IMPLEMENTATION.md for patterns.

// ─────────────────────────────────────────────────────────────────────────
// SCOPE FREEZE TESTS — v0.4 internal-only mainnet RC.
//
// These tests are the runtime-level proof that the external bridge surface
// is paused by default and can only be opened by Root. They are launch
// blockers: if either of these regresses, the pallet is shipping with a
// hot bridge that has not been audited.
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn external_bridges_are_paused_at_genesis() {
    new_test_ext().execute_with(|| {
        assert!(
            !pallet_x3_cross_vm_router::ExternalBridgesEnabled::<Test>::get(),
            "scope freeze: external bridges MUST be off at genesis"
        );
    });
}

#[test]
fn register_external_root_rejected_when_bridges_disabled() {
    new_test_ext().execute_with(|| {
        let res = Router::register_external_root(
            RuntimeOrigin::root(),
            1, // chain_id
            H256::repeat_byte(0xab),
            42, // block_number (in past)
            vec![0u8; 32],
        );
        assert_eq!(
            res,
            Err(pallet_x3_cross_vm_router::Error::<Test>::ExternalBridgesDisabled.into()),
            "register_external_root must fail when bridges are disabled"
        );
    });
}

#[test]
fn emergency_pause_bridge_rejected_when_bridges_disabled() {
    new_test_ext().execute_with(|| {
        let res =
            Router::emergency_pause_bridge(RuntimeOrigin::root(), 1, b"audit pending".to_vec());
        assert_eq!(
            res,
            Err(pallet_x3_cross_vm_router::Error::<Test>::ExternalBridgesDisabled.into()),
            "emergency_pause_bridge must fail when bridges are disabled"
        );
    });
}

#[test]
fn only_root_can_toggle_external_bridges() {
    new_test_ext().execute_with(|| {
        // Non-root must be rejected.
        let res = Router::set_external_bridges_enabled(RuntimeOrigin::signed(0xCAFE), true);
        assert!(res.is_err(), "non-root must not toggle the kill-switch");
        assert!(
            !pallet_x3_cross_vm_router::ExternalBridgesEnabled::<Test>::get(),
            "kill-switch must remain off after a failed non-root toggle"
        );

        // Root may toggle.
        assert_ok!(Router::set_external_bridges_enabled(
            RuntimeOrigin::root(),
            true
        ));
        assert!(pallet_x3_cross_vm_router::ExternalBridgesEnabled::<Test>::get());

        // And toggle back.
        assert_ok!(Router::set_external_bridges_enabled(
            RuntimeOrigin::root(),
            false
        ));
        assert!(!pallet_x3_cross_vm_router::ExternalBridgesEnabled::<Test>::get());
    });
}

#[test]
fn register_external_root_works_only_after_governance_enables() {
    new_test_ext().execute_with(|| {
        // First call: blocked.
        assert!(Router::register_external_root(
            RuntimeOrigin::root(),
            1,
            H256::repeat_byte(0x11),
            1,
            vec![1u8; 8],
        )
        .is_err());

        // Governance opens the gate.
        assert_ok!(Router::set_external_bridges_enabled(
            RuntimeOrigin::root(),
            true
        ));

        // Now it should pass the scope-freeze gate (other validation may still
        // gate it; here block_number=1 == current block so it is in-range).
        assert_ok!(Router::register_external_root(
            RuntimeOrigin::root(),
            1,
            H256::repeat_byte(0x11),
            1,
            vec![1u8; 8],
        ));
    });
}

#[test]
fn packet_commitment_and_ixl_receipt_are_recorded_on_complete() {
    new_test_ext().execute_with(|| {
        let asset_id = bootstrap_x3_asset(10_000);

        let now = System::block_number();
        let sender = alice_native();
        let recipient = alice_evm();
        let nonce = Router::next_nonce(DomainId::X3Native, sender.clone());
        let expires_at = now + 50;

        assert_ok!(Router::xvm_transfer(
            RuntimeOrigin::signed(1),
            asset_id,
            DomainId::X3Native,
            DomainId::X3Evm,
            sender.clone(),
            recipient.clone(),
            100,
            expires_at,
        ));

        let msg = x3_asset_kernel_types::X3TransferMessage::<u64> {
            version: x3_asset_kernel_types::MESSAGE_FORMAT_VERSION,
            asset_id,
            source_domain: DomainId::X3Native,
            destination_domain: DomainId::X3Evm,
            sender,
            recipient,
            amount: 100,
            nonce,
            created_at: now,
            expires_at,
        };
        let message_id = x3_asset_kernel_types::derive_message_id::<u64>(&msg);

        assert!(Router::packet_commitments(message_id).is_some());

        assert_ok!(Router::complete_xvm_transfer(
            RuntimeOrigin::signed(1),
            message_id
        ));

        assert_eq!(Router::ixl_receipt_entries(message_id), Some(1));
    });
}

#[test]
fn completion_rejected_after_packet_timeout() {
    new_test_ext().execute_with(|| {
        let asset_id = bootstrap_x3_asset(10_000);

        let now = System::block_number();
        let sender = alice_native();
        let recipient = alice_evm();
        let nonce = Router::next_nonce(DomainId::X3Native, sender.clone());
        let expires_at = now + 1;

        assert_ok!(Router::xvm_transfer(
            RuntimeOrigin::signed(1),
            asset_id,
            DomainId::X3Native,
            DomainId::X3Evm,
            sender.clone(),
            recipient.clone(),
            100,
            expires_at,
        ));

        let msg = x3_asset_kernel_types::X3TransferMessage::<u64> {
            version: x3_asset_kernel_types::MESSAGE_FORMAT_VERSION,
            asset_id,
            source_domain: DomainId::X3Native,
            destination_domain: DomainId::X3Evm,
            sender,
            recipient,
            amount: 100,
            nonce,
            created_at: now,
            expires_at,
        };
        let message_id = x3_asset_kernel_types::derive_message_id::<u64>(&msg);

        // Timeout policy in packet-standard is now_height >= timeout_height.
        System::set_block_number(expires_at);

        assert_eq!(
            Router::complete_xvm_transfer(RuntimeOrigin::signed(1), message_id),
            Err(pallet_x3_cross_vm_router::Error::<Test>::PacketTimedOut.into())
        );
    });
}
