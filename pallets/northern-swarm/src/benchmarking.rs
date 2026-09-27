//! FRAME benchmarks for pallet-northern-swarm.
//!
//! These benchmarks exercise the actual storage/economic paths used by the
//! release runtime. Regenerate weights with the node benchmark CLI; do not hand
//! edit the generated weight file.

use super::*;
use crate::Pallet as NorthernSwarm;
use frame_benchmarking::{v2::*, whitelisted_caller};
use frame_support::{
    assert_ok,
    traits::{ConstU32, Currency, Get},
    BoundedVec,
};
use frame_system::RawOrigin;
use sp_runtime::traits::{Hash, Saturating};

fn hardware() -> HardwareProfile {
    HardwareProfile {
        cpu_cores: 16,
        gpu_vram_mib: 24_576,
        ram_mib: 131_072,
        bandwidth_mbps: 100_000,
    }
}

fn fund<T: Config>(account: &T::AccountId) {
    let balance = T::MinExecutorStake::get().saturating_mul(100u32.into());
    let _ = T::Currency::make_free_balance_be(account, balance);
}

fn register<T: Config>(account: &T::AccountId) {
    fund::<T>(account);
    let stake = T::MinExecutorStake::get();
    assert_ok!(NorthernSwarm::<T>::register_executor(
        RawOrigin::Signed(account.clone()).into(),
        stake,
        hardware(),
    ));
}

fn submit_compute_task<T: Config>(
    submitter: &T::AccountId,
    reward: BalanceOf<T>,
) -> T::Hash {
    fund::<T>(submitter);
    let payload: BoundedVec<u8, ConstU32<512>> =
        BoundedVec::truncate_from(b"hex:0102030405060708".to_vec());
    let block = frame_system::Pallet::<T>::block_number();
    let task_id = T::Hashing::hash_of(&(submitter, &payload, block));
    assert_ok!(NorthernSwarm::<T>::submit_task(
        RawOrigin::Signed(submitter.clone()).into(),
        payload,
        reward,
        TaskKind::Compute,
    ));
    task_id
}

#[benchmarks]
mod benchmarks {
    use super::*;
    use frame_benchmarking::impl_test_function;

    #[benchmark]
    fn register_executor() {
        let caller: T::AccountId = whitelisted_caller();
        fund::<T>(&caller);
        let stake = T::MinExecutorStake::get();

        #[extrinsic_call]
        register_executor(RawOrigin::Signed(caller), stake, hardware());
    }

    #[benchmark]
    fn deregister_executor() {
        let caller: T::AccountId = whitelisted_caller();
        register::<T>(&caller);

        #[extrinsic_call]
        deregister_executor(RawOrigin::Signed(caller));
    }

    #[benchmark]
    fn release_stake() {
        let caller: T::AccountId = whitelisted_caller();
        register::<T>(&caller);
        assert_ok!(NorthernSwarm::<T>::deregister_executor(
            RawOrigin::Signed(caller.clone()).into(),
        ));
        let unlock_at =
            frame_system::Pallet::<T>::block_number() + T::DeregistrationCooldown::get();
        frame_system::Pallet::<T>::set_block_number(unlock_at);

        #[extrinsic_call]
        release_stake(RawOrigin::Signed(caller));
    }

    #[benchmark]
    fn submit_heartbeat() {
        let caller: T::AccountId = whitelisted_caller();
        register::<T>(&caller);

        #[extrinsic_call]
        submit_heartbeat(RawOrigin::Signed(caller));
    }

    #[benchmark]
    fn submit_task() {
        let caller: T::AccountId = whitelisted_caller();
        fund::<T>(&caller);
        let reward = T::MinExecutorStake::get();
        let payload: BoundedVec<u8, ConstU32<512>> =
            BoundedVec::truncate_from(vec![0x42; 512]);

        #[extrinsic_call]
        submit_task(
            RawOrigin::Signed(caller),
            payload,
            reward,
            TaskKind::Compute,
        );
    }

    #[benchmark]
    fn claim_task() {
        let submitter: T::AccountId = frame_benchmarking::account("submitter", 0, 0);
        let caller: T::AccountId = whitelisted_caller();
        register::<T>(&caller);
        let task_id = submit_compute_task::<T>(&submitter, T::MinExecutorStake::get());

        #[extrinsic_call]
        claim_task(RawOrigin::Signed(caller), task_id);
    }

    #[benchmark]
    fn submit_result() {
        // Worst-case bounded path: fill all result slots with distinct hashes so
        // the measured call scans the full commit set and enters Disputed.
        let submitter: T::AccountId = frame_benchmarking::account("submitter", 0, 0);
        let caller: T::AccountId = whitelisted_caller();
        let other_a: T::AccountId = frame_benchmarking::account("executor", 1, 0);
        let other_b: T::AccountId = frame_benchmarking::account("executor", 2, 0);

        register::<T>(&caller);
        register::<T>(&other_a);
        register::<T>(&other_b);

        let reward = T::MinExecutorStake::get().saturating_mul(3u32.into());
        let task_id = submit_compute_task::<T>(&submitter, reward);

        assert_ok!(NorthernSwarm::<T>::claim_task(
            RawOrigin::Signed(caller.clone()).into(),
            task_id,
        ));
        assert_ok!(NorthernSwarm::<T>::claim_task(
            RawOrigin::Signed(other_a.clone()).into(),
            task_id,
        ));
        assert_ok!(NorthernSwarm::<T>::claim_task(
            RawOrigin::Signed(other_b.clone()).into(),
            task_id,
        ));

        let hash_a = T::Hashing::hash(b"benchmark-result-a");
        let hash_b = T::Hashing::hash(b"benchmark-result-b");
        let hash_c = T::Hashing::hash(b"benchmark-result-c");

        assert_ok!(NorthernSwarm::<T>::submit_result(
            RawOrigin::Signed(other_a).into(),
            task_id,
            hash_a,
        ));
        assert_ok!(NorthernSwarm::<T>::submit_result(
            RawOrigin::Signed(other_b).into(),
            task_id,
            hash_b,
        ));

        #[extrinsic_call]
        submit_result(RawOrigin::Signed(caller), task_id, hash_c);
    }

    #[benchmark]
    fn slash_executor() {
        let executor: T::AccountId = frame_benchmarking::account("executor", 0, 0);
        register::<T>(&executor);
        let amount = T::MinExecutorStake::get();

        #[extrinsic_call]
        slash_executor(
            RawOrigin::Root,
            executor,
            amount,
            SlashReason::Governance,
        );
    }

    impl_benchmark_test_suite!(
        NorthernSwarm,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
}
