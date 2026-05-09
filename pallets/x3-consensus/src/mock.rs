//! Mock runtime for consensus pallet tests

use crate::*;
use frame_support::{derive_impl, parameter_types, traits::ConstU64};
use sp_runtime::{
    impl_opaque_keys,
    testing::UintAuthorityId,
    traits::ConvertInto,
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

// Minimal opaque session key type for the mock.
impl_opaque_keys! {
    pub struct MockSessionKeys {
        pub dummy: UintAuthorityId,
    }
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Timestamp: pallet_timestamp,
        Balances: pallet_balances,
        Aura: pallet_aura,
        Grandpa: pallet_grandpa,
        Session: pallet_session,
        Consensus: crate,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u64>;
}

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = Aura;
    type MinimumPeriod = ConstU64<5>;
    type WeightInfo = ();
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

impl pallet_aura::Config for Test {
    type AuthorityId = sp_consensus_aura::sr25519::AuthorityId;
    type DisabledValidators = ();
    type MaxAuthorities = frame_support::traits::ConstU32<32>;
    type AllowMultipleBlocksPerSlot = frame_support::traits::ConstBool<false>;
    type SlotDuration = pallet_aura::MinimumPeriodTimesTwo<Test>;
}

impl pallet_grandpa::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MaxAuthorities = frame_support::traits::ConstU32<32>;
    type MaxNominators = frame_support::traits::ConstU32<0>;
    type MaxSetIdSessionEntries = frame_support::traits::ConstU64<0>;
    type KeyOwnerProof = sp_core::Void;
    type EquivocationReportSystem = ();
}

parameter_types! {
    pub const Period: u64 = 1;
    pub const Offset: u64 = 0;
}

impl pallet_session::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = <Self as frame_system::Config>::AccountId;
    type ValidatorIdOf = ConvertInto;
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionManager = ();
    type SessionHandler = pallet_session::TestSessionHandler;
    type Keys = MockSessionKeys;
    type DisablingStrategy = ();
    type WeightInfo = ();
    type Currency = Balances;
    type KeyDeposit = ();
}

parameter_types! {
    pub const MaxValidators: u32 = 100;
    pub const SlashFractionConst: sp_runtime::Perbill = sp_runtime::Perbill::from_percent(10);
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxValidators = MaxValidators;
    type WeightInfo = ();
    type Currency = Balances;
    type SlashFraction = SlashFractionConst;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(1, 1_000), (2, 1_000), (3, 1_000)],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}

