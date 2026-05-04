#![cfg(feature = "runtime-benchmarks")]

use super::*;

#[allow(unused)]
use crate::Pallet as X3AtomicKernel;
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::RawOrigin;
use sp_core::H256;

benchmarks! {
    submit_atomic_bundle {
        let b in 1 .. T::MaxLegsPerBundle::get();
        let caller: T::AccountId = whitelisted_caller();

        let mut legs = Vec::new();
        for _i in 0..b {
            legs.push(proof::BundleLeg {
                vm_type: proof::VmType::Svm,
                token_in: H256::repeat_byte(0),
                token_out: H256::repeat_byte(1),
                amount_in: 100,
                min_amount_out: 0,
                deadline: 10_000,
                access: proof::DeclaredAccess {
                    reads: Default::default(),
                    writes: Default::default(),
                },
            });
        }

        let bond = T::MinBond::get();
        let _ = T::Currency::make_free_balance_be(&caller, bond.saturating_mul(10u32.into()));

    }: _(RawOrigin::Signed(caller.clone()), legs, 1000u32.into())
    verify {
    }

    finalize_atomic_bundle {
        let caller: T::AccountId = whitelisted_caller();
        let bond = T::MinBond::get();
        let _ = T::Currency::make_free_balance_be(&caller, bond.saturating_mul(10u32.into()));

        let legs = vec![proof::BundleLeg {
            vm_type: proof::VmType::Svm,
            token_in: H256::repeat_byte(0),
            token_out: H256::repeat_byte(1),
            amount_in: 100,
            min_amount_out: 0,
            deadline: 10_000,
            access: proof::DeclaredAccess {
                reads: Default::default(),
                writes: Default::default(),
            },
        }];
        X3AtomicKernel::<T>::submit_atomic_bundle(RawOrigin::Signed(caller.clone()).into(), legs, 1000u32.into()).unwrap();
        // Get the generated bundle ID from storage (there should only be 1)
        let bundle_id = Bundles::<T>::iter_keys().next().unwrap();

        X3AtomicKernel::<T>::assign_bundle_executor(RawOrigin::Root.into(), bundle_id, caller.clone()).unwrap();

        let receipt_root = H256::repeat_byte(0x11);
        let finality_cert = H256::repeat_byte(0x22);
        let block_number: BlockNumberFor<T> = 1u32.into();
    }: _(RawOrigin::Signed(caller), bundle_id, receipt_root, finality_cert, block_number)
    verify {
        let bundle = Bundles::<T>::get(bundle_id).unwrap();
        assert_eq!(bundle.status, BundleStatus::Finalized);
    }

    impl_benchmark_test_suite!(X3AtomicKernel, crate::tests::new_test_ext(), crate::tests::Test);
}
