#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{benchmarks, whitelist_account};
use frame_system::RawOrigin;
use sp_core::H256;

benchmarks! {
    validate_evm_header {
        let origin = RawOrigin::Signed(whitelist_account::<T::AccountId>("caller", 0, 0).into());
    }: validate_evm_header(
        origin,
        100u64,
        H256::from([1u8; 32]),
        H256::from([2u8; 32]),
        H256::from([3u8; 32]),
        vec![4u8; 32],
    )

    validate_svm_header {
        let origin = RawOrigin::Signed(whitelist_account::<T::AccountId>("caller", 0, 0).into());
    }: validate_svm_header(
        origin,
        100u64,
        H256::from([1u8; 32]),
        H256::from([2u8; 32]),
        vec![3u8; 32],
        vec![],
    )

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::MockRuntime);
}
