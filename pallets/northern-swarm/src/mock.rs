use crate as pallet_northern_swarm;
use frame_support::{construct_runtime, derive_impl, parameter_types};
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        NorthernSwarm: pallet_northern_swarm,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u64>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

parameter_types! {
    pub const MinExecutorStake: u64 = 100;
    pub const DeregistrationCooldown: u64 = 10;
    pub const MaxClaimedTasksPerExecutor: u32 = 4;
    pub const QuorumThreshold: u32 = 2;
    pub const MaxExecutorsPerTask: u32 = 3;
}

impl pallet_northern_swarm::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MinExecutorStake = MinExecutorStake;
    type DeregistrationCooldown = DeregistrationCooldown;
    type MaxClaimedTasksPerExecutor = MaxClaimedTasksPerExecutor;
    type QuorumThreshold = QuorumThreshold;
    type MaxExecutorsPerTask = MaxExecutorsPerTask;
    type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .expect("system genesis builds");

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1, 10_000),
            (2, 10_000),
            (3, 10_000),
            (4, 10_000),
            (5, 10_000),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut storage)
    .expect("balances genesis builds");

    storage.into()
}

pub fn hardware() -> crate::HardwareProfile {
    crate::HardwareProfile {
        cpu_cores: 8,
        gpu_vram_mib: 8_192,
        ram_mib: 32_768,
        bandwidth_mbps: 10_000,
    }
}
