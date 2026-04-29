// Tests for pallet-x3-automation

use super::*;
use crate as pallet_x3_automation;
use frame_support::{parameter_types, traits::Everything};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};
use x3_automation::{Action, Condition};

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
        Oracle: pallet_x3_oracle::{Pallet, Call, Storage, Event<T>},
        Automation: pallet_x3_automation::{Pallet, Call, Storage, Event<T>},
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const SS58Prefix: u8 = 42;
}

impl system::Config for Test {
    type BaseCallFilter = Everything;
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
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
    pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Balance = u64;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type ReserveIdentifier = [u8; 8];
    type RuntimeHoldReason = ();
    type FreezeIdentifier = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type MaxHolds = ();
    type MaxFreezes = ();
}

parameter_types! {
    pub const MaxSubmissionsPerBlock: u32 = 10;
    pub const MaxAssets: u32 = 100;
    pub const MaxSubmissionsPerAsset: u32 = 50;
    pub const MinSubmissionsForMedian: u32 = 3;
    pub const MaxSubmissionAge: u64 = 3600;
}

impl pallet_x3_oracle::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxSubmissionsPerBlock = MaxSubmissionsPerBlock;
    type MaxAssets = MaxAssets;
    type MaxSubmissionsPerAsset = MaxSubmissionsPerAsset;
    type MinSubmissionsForMedian = MinSubmissionsForMedian;
    type MaxSubmissionAge = MaxSubmissionAge;
    type UpdateOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type WeightInfo = ();
}

parameter_types! {
    pub const MaxTasksPerAccount: u32 = 10;
    pub const BaseRegistrationFee: u64 = 100;
    pub const ExecutionFee: u64 = 50;
    pub const MaxTaskExpiryBlocks: u32 = 1000;
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxTasksPerAccount = MaxTasksPerAccount;
    type BaseRegistrationFee = BaseRegistrationFee;
    type ExecutionFee = ExecutionFee;
    type MaxTaskExpiryBlocks = MaxTaskExpiryBlocks;
    type WeightInfo = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(1, 1000000), (2, 1000000)],
    }
    .assimilate_storage(&mut t)
    .unwrap();
    t.into()
}