//! Tests for the x3-asset-registry pallet.

use crate::pallet::{Assets, Error, Event, Routes, TotalAssets};
use crate::Pallet as AssetRegistryPallet;

use frame_support::{
    assert_noop, assert_ok, construct_runtime, derive_impl, ord_parameter_types,
    parameter_types,
    traits::{ConstU32, ConstU64, EnsureOrigin},
};
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};
use x3_asset_kernel_types::{
    AssetId, AssetStatus, DomainId, ProofTier, RouteConfig, RouteLimits, SupplyPolicy,
};

// ── Runtime construction ────────────────────────────────────────────────────

construct_runtime!(
    pub enum TestRuntime {
        System: frame_system,
        AssetRegistry: crate,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for TestRuntime {
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
    pub const MaxAssets: u32 = 100;
}

// Use root for registry and signed(99) for emergency pause in tests.
pub struct EnsureRoot;
impl EnsureOrigin<RuntimeOrigin> for EnsureRoot {
    type Success = ();
    fn try_origin(o: RuntimeOrigin) -> Result<(), RuntimeOrigin> {
        match o.into() {
            Ok(frame_system::RawOrigin::Root) => Ok(()),
            Err(o) => Err(o),
            Ok(other) => Err(other.into()),
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RuntimeOrigin::root())
    }
}

pub struct EnsureSigned99;
impl EnsureOrigin<RuntimeOrigin> for EnsureSigned99 {
    type Success = ();
    fn try_origin(o: RuntimeOrigin) -> Result<(), RuntimeOrigin> {
        match o.into() {
            Ok(frame_system::RawOrigin::Signed(99u64)) => Ok(()),
            Ok(other) => Err(other.into()),
            Err(o) => Err(o),
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RuntimeOrigin::signed(99))
    }
}

impl crate::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type RegistryOrigin = EnsureRoot;
    type EmergencyPauseOrigin = EnsureSigned99;
    type MaxAssets = MaxAssets;
}

// ── Helpers ──────────────────────────────────────────────────────────────────

pub fn new_test_ext() -> sp_io::TestExternalities {
    let storage = frame_system::GenesisConfig::<TestRuntime>::default()
        .build_storage()
        .unwrap();
    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| frame_system::Pallet::<TestRuntime>::set_block_number(1));
    ext
}

fn default_route_config(enabled: bool) -> RouteConfig {
    RouteConfig {
        enabled,
        limits: RouteLimits {
            min_amount: 0,
            max_amount: 1_000_000,
            daily_limit: 10_000_000,
            per_wallet_daily_limit: 1_000_000,
            pending_limit: 100,
        },
        fee_bps: 10,
        expiry_blocks: 100,
        proof_tier: ProofTier::TrustedInternal,
    }
}

/// Register a test asset via root origin.  Returns the AssetId.
fn register_test_asset(symbol: &[u8]) -> AssetId {
    assert_ok!(AssetRegistryPallet::<TestRuntime>::register_asset(
        RuntimeOrigin::root(),
        symbol.to_vec(),
        b"Test Asset".to_vec(),
        18,
        DomainId::X3Native,
        1,
        b"0x0000000000000000000000000000000000000001".to_vec(),
        SupplyPolicy::NativeMintBurn,
    ));
    // Derive the id the same way the pallet does
    x3_asset_kernel_types::derive_asset_id(
        DomainId::X3Native,
        1,
        b"0x0000000000000000000000000000000000000001",
        symbol,
        18,
    )
}

// ── register_asset ────────────────────────────────────────────────────────────

#[test]
fn register_asset_succeeds_and_emits_event() {
    new_test_ext().execute_with(|| {
        let asset_id = register_test_asset(b"X3T");

        // Storage populated
        let meta = Assets::<TestRuntime>::get(asset_id).expect("asset should exist");
        assert_eq!(meta.canonical_decimals, 18);
        assert_eq!(meta.status, AssetStatus::Registered);
        assert_eq!(TotalAssets::<TestRuntime>::get(), 1);

        frame_system::Pallet::<TestRuntime>::assert_last_event(
            Event::AssetRegistered {
                asset_id,
                origin_domain: DomainId::X3Native,
                canonical_decimals: 18,
            }
            .into(),
        );
    });
}

#[test]
fn register_asset_fails_if_duplicate() {
    new_test_ext().execute_with(|| {
        register_test_asset(b"DUP");
        assert_noop!(
            AssetRegistryPallet::<TestRuntime>::register_asset(
                RuntimeOrigin::root(),
                b"DUP".to_vec(),
                b"Test Asset".to_vec(),
                18,
                DomainId::X3Native,
                1,
                b"0x0000000000000000000000000000000000000001".to_vec(),
                SupplyPolicy::NativeMintBurn,
            ),
            Error::<TestRuntime>::AssetAlreadyExists
        );
    });
}

#[test]
fn register_asset_fails_when_at_max_assets() {
    new_test_ext().execute_with(|| {
        // Fill up to MaxAssets
        for i in 0u32..100 {
            let addr = format!("0x{:040x}", i);
            assert_ok!(AssetRegistryPallet::<TestRuntime>::register_asset(
                RuntimeOrigin::root(),
                format!("T{}", i).into_bytes(),
                b"name".to_vec(),
                18,
                DomainId::X3Native,
                i as u64,
                addr.into_bytes(),
                SupplyPolicy::NativeMintBurn,
            ));
        }
        assert_noop!(
            AssetRegistryPallet::<TestRuntime>::register_asset(
                RuntimeOrigin::root(),
                b"OVERFLOW".to_vec(),
                b"name".to_vec(),
                18,
                DomainId::X3Evm,
                999,
                b"0x00overflow".to_vec(),
                SupplyPolicy::LockMint,
            ),
            Error::<TestRuntime>::TooManyAssets
        );
    });
}

#[test]
fn register_asset_rejected_from_non_registry_origin() {
    new_test_ext().execute_with(|| {
        assert!(AssetRegistryPallet::<TestRuntime>::register_asset(
            RuntimeOrigin::signed(1),
            b"BAD".to_vec(),
            b"name".to_vec(),
            18,
            DomainId::X3Native,
            1,
            b"0xabc".to_vec(),
            SupplyPolicy::NativeMintBurn,
        )
        .is_err());
    });
}

// ── activate_asset / pause_asset / unpause_asset / retire_asset ──────────────

#[test]
fn asset_lifecycle_registered_to_active_to_paused_to_active_to_retired() {
    new_test_ext().execute_with(|| {
        let id = register_test_asset(b"LIFE");

        // Start: Registered
        assert_eq!(Assets::<TestRuntime>::get(id).unwrap().status, AssetStatus::Registered);

        // → Active
        assert_ok!(AssetRegistryPallet::<TestRuntime>::activate_asset(
            RuntimeOrigin::root(),
            id
        ));
        assert_eq!(Assets::<TestRuntime>::get(id).unwrap().status, AssetStatus::Active);

        // → Paused (emergency origin)
        assert_ok!(AssetRegistryPallet::<TestRuntime>::pause_asset(
            RuntimeOrigin::signed(99),
            id
        ));
        assert_eq!(Assets::<TestRuntime>::get(id).unwrap().status, AssetStatus::Paused);

        // → Active again (unpause)
        assert_ok!(AssetRegistryPallet::<TestRuntime>::unpause_asset(
            RuntimeOrigin::root(),
            id
        ));
        assert_eq!(Assets::<TestRuntime>::get(id).unwrap().status, AssetStatus::Active);

        // → Retired (terminal)
        assert_ok!(AssetRegistryPallet::<TestRuntime>::retire_asset(
            RuntimeOrigin::root(),
            id
        ));
        assert_eq!(Assets::<TestRuntime>::get(id).unwrap().status, AssetStatus::Retired);
    });
}

#[test]
fn retired_asset_cannot_be_modified() {
    new_test_ext().execute_with(|| {
        let id = register_test_asset(b"DEAD");
        assert_ok!(AssetRegistryPallet::<TestRuntime>::retire_asset(RuntimeOrigin::root(), id));

        assert_noop!(
            AssetRegistryPallet::<TestRuntime>::activate_asset(RuntimeOrigin::root(), id),
            Error::<TestRuntime>::AssetRetired
        );
        assert_noop!(
            AssetRegistryPallet::<TestRuntime>::pause_asset(RuntimeOrigin::signed(99), id),
            Error::<TestRuntime>::AssetRetired
        );
    });
}

#[test]
fn status_change_on_unknown_asset_fails() {
    new_test_ext().execute_with(|| {
        let bogus = H256::repeat_byte(0xde);
        assert_noop!(
            AssetRegistryPallet::<TestRuntime>::activate_asset(RuntimeOrigin::root(), bogus),
            Error::<TestRuntime>::UnknownAsset
        );
    });
}

// ── configure_route ──────────────────────────────────────────────────────────

#[test]
fn configure_route_succeeds_for_active_asset() {
    new_test_ext().execute_with(|| {
        let id = register_test_asset(b"RTE");
        assert_ok!(AssetRegistryPallet::<TestRuntime>::activate_asset(RuntimeOrigin::root(), id));

        let cfg = default_route_config(true);
        assert_ok!(AssetRegistryPallet::<TestRuntime>::configure_route(
            RuntimeOrigin::root(),
            id,
            DomainId::X3Native,
            DomainId::X3Evm,
            cfg.clone(),
        ));

        let stored = Routes::<TestRuntime>::get(id, (DomainId::X3Native, DomainId::X3Evm))
            .expect("route should be stored");
        assert!(stored.enabled);
        assert_eq!(stored.limits.max_amount, 1_000_000);

        frame_system::Pallet::<TestRuntime>::assert_last_event(
            Event::RouteConfigured {
                asset_id: id,
                source: DomainId::X3Native,
                destination: DomainId::X3Evm,
                enabled: true,
            }
            .into(),
        );
    });
}

#[test]
fn configure_route_rejects_self_loop() {
    new_test_ext().execute_with(|| {
        let id = register_test_asset(b"LOOP");
        assert_ok!(AssetRegistryPallet::<TestRuntime>::activate_asset(RuntimeOrigin::root(), id));

        assert_noop!(
            AssetRegistryPallet::<TestRuntime>::configure_route(
                RuntimeOrigin::root(),
                id,
                DomainId::X3Native,
                DomainId::X3Native,
                default_route_config(true),
            ),
            Error::<TestRuntime>::SelfLoopRoute
        );
    });
}

#[test]
fn configure_route_rejects_enabled_route_with_zero_max_amount() {
    new_test_ext().execute_with(|| {
        let id = register_test_asset(b"ZERO");
        assert_ok!(AssetRegistryPallet::<TestRuntime>::activate_asset(RuntimeOrigin::root(), id));

        let mut bad_cfg = default_route_config(true);
        bad_cfg.limits.max_amount = 0;

        assert_noop!(
            AssetRegistryPallet::<TestRuntime>::configure_route(
                RuntimeOrigin::root(),
                id,
                DomainId::X3Native,
                DomainId::X3Evm,
                bad_cfg,
            ),
            Error::<TestRuntime>::InvalidRouteLimits
        );
    });
}

#[test]
fn configure_route_on_unknown_asset_fails() {
    new_test_ext().execute_with(|| {
        let bogus = H256::repeat_byte(0xab);
        assert_noop!(
            AssetRegistryPallet::<TestRuntime>::configure_route(
                RuntimeOrigin::root(),
                bogus,
                DomainId::X3Native,
                DomainId::X3Evm,
                default_route_config(true),
            ),
            Error::<TestRuntime>::UnknownAsset
        );
    });
}

// ── set_route_enabled ─────────────────────────────────────────────────────────

#[test]
fn set_route_enabled_false_via_emergency_origin_succeeds() {
    new_test_ext().execute_with(|| {
        let id = register_test_asset(b"TOGG");
        assert_ok!(AssetRegistryPallet::<TestRuntime>::activate_asset(RuntimeOrigin::root(), id));
        assert_ok!(AssetRegistryPallet::<TestRuntime>::configure_route(
            RuntimeOrigin::root(),
            id,
            DomainId::X3Native,
            DomainId::X3Evm,
            default_route_config(true),
        ));

        // Disable via emergency origin
        assert_ok!(AssetRegistryPallet::<TestRuntime>::set_route_enabled(
            RuntimeOrigin::signed(99),
            id,
            DomainId::X3Native,
            DomainId::X3Evm,
            false,
        ));
        assert!(!Routes::<TestRuntime>::get(id, (DomainId::X3Native, DomainId::X3Evm))
            .unwrap()
            .enabled);

        // Re-enable via registry origin
        assert_ok!(AssetRegistryPallet::<TestRuntime>::set_route_enabled(
            RuntimeOrigin::root(),
            id,
            DomainId::X3Native,
            DomainId::X3Evm,
            true,
        ));
        assert!(Routes::<TestRuntime>::get(id, (DomainId::X3Native, DomainId::X3Evm))
            .unwrap()
            .enabled);
    });
}

#[test]
fn set_route_enabled_true_rejected_from_emergency_only_origin() {
    new_test_ext().execute_with(|| {
        let id = register_test_asset(b"AUTH");
        assert_ok!(AssetRegistryPallet::<TestRuntime>::activate_asset(RuntimeOrigin::root(), id));
        assert_ok!(AssetRegistryPallet::<TestRuntime>::configure_route(
            RuntimeOrigin::root(),
            id,
            DomainId::X3Native,
            DomainId::X3Evm,
            default_route_config(false),
        ));

        // signed(99) is only EmergencyPauseOrigin, not RegistryOrigin — cannot re-enable
        assert!(AssetRegistryPallet::<TestRuntime>::set_route_enabled(
            RuntimeOrigin::signed(99),
            id,
            DomainId::X3Native,
            DomainId::X3Evm,
            true,
        )
        .is_err());
    });
}

// ── AssetRegistryInspect trait ────────────────────────────────────────────────

#[test]
fn inspect_trait_returns_correct_metadata() {
    use x3_asset_kernel_types::traits::AssetRegistryInspect;
    new_test_ext().execute_with(|| {
        let id = register_test_asset(b"INSP");

        assert!(AssetRegistryPallet::<TestRuntime>::exists(&id));
        assert_eq!(
            AssetRegistryPallet::<TestRuntime>::status(&id),
            Some(AssetStatus::Registered)
        );
        assert_eq!(
            AssetRegistryPallet::<TestRuntime>::supply_policy(&id),
            Some(SupplyPolicy::NativeMintBurn)
        );
        assert_eq!(
            AssetRegistryPallet::<TestRuntime>::canonical_decimals(&id),
            Some(18)
        );

        // Non-existent asset
        let bogus = H256::repeat_byte(0xff);
        assert!(!AssetRegistryPallet::<TestRuntime>::exists(&bogus));
        assert!(AssetRegistryPallet::<TestRuntime>::status(&bogus).is_none());
    });
}
