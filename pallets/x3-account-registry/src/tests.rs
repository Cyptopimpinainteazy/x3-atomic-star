//! Tests for the x3-account-registry pallet.

use crate::pallet::{AccountKind, AccountKinds, AccountRegistry, CrossVmNonces, Error, Event};
use crate::Pallet as AccountRegistry_Pallet;

use frame_support::{
    assert_noop, assert_ok, construct_runtime, derive_impl, parameter_types,
    traits::{ConstU32, ConstU64},
};
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

// ── Runtime construction ────────────────────────────────────────────────────

construct_runtime!(
    pub enum TestRuntime {
        System: frame_system,
        AccountRegistryPallet: crate,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for TestRuntime {
    type BaseCallFilter = frame_support::traits::Everything;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = frame_system::mocking::MockBlock<TestRuntime>;
    type Nonce = u64;
    type BlockHashCount = ConstU64<250>;
}

parameter_types! {
    pub const MaxNameLength: u32 = 32;
}

impl crate::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type AtlasId = u32;
    type MaxNameLength = MaxNameLength;
}

// ── Test helpers ─────────────────────────────────────────────────────────────

pub fn new_test_ext() -> sp_io::TestExternalities {
    let storage = frame_system::GenesisConfig::<TestRuntime>::default()
        .build_storage()
        .unwrap();
    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| frame_system::Pallet::<TestRuntime>::set_block_number(1));
    ext
}

fn register(origin: u64, atlas_id: u32, kind: AccountKind) {
    assert_ok!(AccountRegistry_Pallet::<TestRuntime>::register_account(
        RuntimeOrigin::signed(origin),
        atlas_id,
        kind,
        b"test".to_vec(),
    ));
}

// ── register_account ─────────────────────────────────────────────────────────

#[test]
fn register_account_succeeds_and_emits_event() {
    new_test_ext().execute_with(|| {
        assert_ok!(AccountRegistry_Pallet::<TestRuntime>::register_account(
            RuntimeOrigin::signed(1),
            42u32,
            AccountKind::Eoa,
            b"alice".to_vec(),
        ));

        // Storage state
        assert_eq!(AccountRegistry::<TestRuntime>::get(1), Some(42u32));
        assert_eq!(
            crate::pallet::AtlasRegistry::<TestRuntime>::get(42u32),
            Some(1u64)
        );
        assert_eq!(AccountKinds::<TestRuntime>::get(1), Some(AccountKind::Eoa));
        assert_eq!(crate::pallet::AccountCount::<TestRuntime>::get(), 1);

        // Event emitted
        frame_system::Pallet::<TestRuntime>::assert_last_event(
            Event::AccountRegistered {
                account: 1,
                atlas_id: 42u32,
            }
            .into(),
        );
    });
}

#[test]
fn register_account_fails_if_already_registered() {
    new_test_ext().execute_with(|| {
        register(1, 42, AccountKind::Eoa);
        assert_noop!(
            AccountRegistry_Pallet::<TestRuntime>::register_account(
                RuntimeOrigin::signed(1),
                99u32,
                AccountKind::Eoa,
                b"alice2".to_vec(),
            ),
            Error::<TestRuntime>::AlreadyRegistered
        );
    });
}

#[test]
fn register_account_fails_if_atlas_id_in_use() {
    new_test_ext().execute_with(|| {
        register(1, 42, AccountKind::Eoa);
        assert_noop!(
            AccountRegistry_Pallet::<TestRuntime>::register_account(
                RuntimeOrigin::signed(2),
                42u32,
                AccountKind::Eoa,
                b"bob".to_vec(),
            ),
            Error::<TestRuntime>::AtlasIdInUse
        );
    });
}

#[test]
fn register_account_fails_if_name_too_long() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            AccountRegistry_Pallet::<TestRuntime>::register_account(
                RuntimeOrigin::signed(1),
                42u32,
                AccountKind::Eoa,
                vec![b'a'; 33], // MaxNameLength + 1
            ),
            Error::<TestRuntime>::NameTooLong
        );
    });
}

#[test]
fn register_account_allows_maximum_name_length() {
    new_test_ext().execute_with(|| {
        assert_ok!(AccountRegistry_Pallet::<TestRuntime>::register_account(
            RuntimeOrigin::signed(1),
            42u32,
            AccountKind::Validator,
            vec![b'v'; 32], // Exactly MaxNameLength
        ));
    });
}

// ── deregister_account ────────────────────────────────────────────────────────

#[test]
fn deregister_account_succeeds_and_clears_storage() {
    new_test_ext().execute_with(|| {
        register(1, 42, AccountKind::Eoa);

        assert_ok!(AccountRegistry_Pallet::<TestRuntime>::deregister_account(
            RuntimeOrigin::signed(1),
        ));

        assert!(AccountRegistry::<TestRuntime>::get(1).is_none());
        assert!(crate::pallet::AtlasRegistry::<TestRuntime>::get(42u32).is_none());
        assert!(AccountKinds::<TestRuntime>::get(1).is_none());
        assert_eq!(crate::pallet::AccountCount::<TestRuntime>::get(), 0);

        frame_system::Pallet::<TestRuntime>::assert_last_event(
            Event::AccountDeregistered {
                account: 1,
                atlas_id: 42u32,
            }
            .into(),
        );
    });
}

#[test]
fn deregister_account_fails_if_not_registered() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            AccountRegistry_Pallet::<TestRuntime>::deregister_account(RuntimeOrigin::signed(1)),
            Error::<TestRuntime>::NotRegistered
        );
    });
}

#[test]
fn account_count_reflects_multiple_registrations_and_deregistrations() {
    new_test_ext().execute_with(|| {
        register(1, 1, AccountKind::Eoa);
        register(2, 2, AccountKind::Eoa);
        register(3, 3, AccountKind::Validator);
        assert_eq!(crate::pallet::AccountCount::<TestRuntime>::get(), 3);

        assert_ok!(AccountRegistry_Pallet::<TestRuntime>::deregister_account(
            RuntimeOrigin::signed(2)
        ));
        assert_eq!(crate::pallet::AccountCount::<TestRuntime>::get(), 2);
    });
}

// ── anchor_nonce ──────────────────────────────────────────────────────────────

#[test]
fn anchor_nonce_emits_event_for_registered_account() {
    new_test_ext().execute_with(|| {
        register(1, 42, AccountKind::Eoa);
        // Increment nonce manually via helper
        AccountRegistry_Pallet::<TestRuntime>::increment_cross_vm_nonce(&1u64);

        assert_ok!(AccountRegistry_Pallet::<TestRuntime>::anchor_nonce(
            RuntimeOrigin::signed(1),
        ));

        frame_system::Pallet::<TestRuntime>::assert_last_event(
            Event::NonceAnchored { account: 1, nonce: 1 }.into(),
        );
    });
}

#[test]
fn anchor_nonce_fails_for_unregistered_account() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            AccountRegistry_Pallet::<TestRuntime>::anchor_nonce(RuntimeOrigin::signed(99)),
            Error::<TestRuntime>::NotRegistered
        );
    });
}

// ── Helper / read-only API ────────────────────────────────────────────────────

#[test]
fn get_atlas_id_and_get_account_are_inverse() {
    new_test_ext().execute_with(|| {
        register(1, 7, AccountKind::System);
        assert_eq!(
            AccountRegistry_Pallet::<TestRuntime>::get_atlas_id(&1u64),
            Some(7u32)
        );
        assert_eq!(
            AccountRegistry_Pallet::<TestRuntime>::get_account(7u32),
            Some(1u64)
        );
        // Non-existent
        assert!(AccountRegistry_Pallet::<TestRuntime>::get_atlas_id(&99u64).is_none());
    });
}

#[test]
fn cross_vm_nonce_starts_at_zero_and_increments() {
    new_test_ext().execute_with(|| {
        register(1, 1, AccountKind::EvmContract);
        assert_eq!(
            AccountRegistry_Pallet::<TestRuntime>::get_next_cross_vm_nonce(&1u64),
            0
        );
        AccountRegistry_Pallet::<TestRuntime>::increment_cross_vm_nonce(&1u64);
        AccountRegistry_Pallet::<TestRuntime>::increment_cross_vm_nonce(&1u64);
        assert_eq!(
            AccountRegistry_Pallet::<TestRuntime>::get_next_cross_vm_nonce(&1u64),
            2
        );
    });
}

#[test]
fn all_account_kinds_can_be_registered() {
    new_test_ext().execute_with(|| {
        let kinds = [
            (1u64, 1u32, AccountKind::Eoa),
            (2, 2, AccountKind::EvmContract),
            (3, 3, AccountKind::SvmProgram),
            (4, 4, AccountKind::X3AppZone),
            (5, 5, AccountKind::Validator),
            (6, 6, AccountKind::System),
        ];
        for (account, atlas_id, kind) in kinds {
            assert_ok!(AccountRegistry_Pallet::<TestRuntime>::register_account(
                RuntimeOrigin::signed(account),
                atlas_id,
                kind,
                b"name".to_vec(),
            ));
            assert_eq!(AccountKinds::<TestRuntime>::get(account), Some(kind));
        }
        assert_eq!(crate::pallet::AccountCount::<TestRuntime>::get(), 6);
    });
}
