//! Benchmarking setup for `pallet-x3-sentinel`.
//!
//! All seven dispatchables charged 15,000 picoseconds — each of them a security power: freezing an
//! authority's supply-changing rights on an asset, freezing the asset itself, enrolling it for
//! guardian review, and granting an approval — behind a `runtime-benchmarks` feature with nothing
//! behind it.
//!
//! The unfreeze and unenroll calls only mean something on state that already exists, so their setups
//! establish it through the pallet's own calls.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::{traits::EnsureOrigin, BoundedVec};
use frame_system::RawOrigin;
use x3_asset_kernel_types::AssetId;

#[benchmarks]
mod benchmarks {
    use super::*;

    const ASSET: AssetId = sp_core::H256([0x0A; 32]);

    fn reason<T: Config>() -> Result<BoundedVec<u8, MaxFreezeReasonLen>, BenchmarkError> {
        BoundedVec::<u8, MaxFreezeReasonLen>::try_from(b"benchmark".to_vec())
            .map_err(|_| BenchmarkError::Weightless)
    }

    #[benchmark]
    fn freeze_authority() -> Result<(), BenchmarkError> {
        let origin =
            T::FreezeOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        let authority: T::AccountId = whitelisted_caller();
        let reason = reason::<T>()?;

        #[extrinsic_call]
        freeze_authority(origin, ASSET, authority.clone(), reason);

        assert!(FrozenAccounts::<T>::contains_key(ASSET, &authority));
        Ok(())
    }

    #[benchmark]
    fn unfreeze_authority() -> Result<(), BenchmarkError> {
        let origin =
            T::FreezeOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        let authority: T::AccountId = whitelisted_caller();
        let reason = reason::<T>()?;
        Pallet::<T>::freeze_authority(origin.clone(), ASSET, authority.clone(), reason)?;

        #[extrinsic_call]
        unfreeze_authority(origin, ASSET, authority.clone());

        assert!(!FrozenAccounts::<T>::contains_key(ASSET, &authority));
        Ok(())
    }

    #[benchmark]
    fn freeze_asset() -> Result<(), BenchmarkError> {
        let origin =
            T::FreezeOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        let reason = reason::<T>()?;

        #[extrinsic_call]
        freeze_asset(origin, ASSET, reason);

        assert!(FrozenAssets::<T>::contains_key(ASSET));
        Ok(())
    }

    #[benchmark]
    fn unfreeze_asset() -> Result<(), BenchmarkError> {
        let origin =
            T::FreezeOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        Pallet::<T>::freeze_asset(origin.clone(), ASSET, reason::<T>()?)?;

        #[extrinsic_call]
        unfreeze_asset(origin, ASSET);

        assert!(!FrozenAssets::<T>::contains_key(ASSET));
        Ok(())
    }

    #[benchmark]
    fn enroll_for_review() -> Result<(), BenchmarkError> {
        let origin =
            T::FreezeOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        enroll_for_review(origin, ASSET);

        assert!(ReviewEnrolled::<T>::contains_key(ASSET));
        Ok(())
    }

    #[benchmark]
    fn unenroll_from_review() -> Result<(), BenchmarkError> {
        let origin =
            T::FreezeOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        Pallet::<T>::enroll_for_review(origin.clone(), ASSET)?;

        #[extrinsic_call]
        unenroll_from_review(origin, ASSET);

        assert!(!ReviewEnrolled::<T>::contains_key(ASSET));
        Ok(())
    }

    #[benchmark]
    fn grant_guardian_approval() -> Result<(), BenchmarkError> {
        let origin =
            T::FreezeOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        // An approval only counts for an asset that is enrolled for review.
        Pallet::<T>::enroll_for_review(origin.clone(), ASSET)?;

        #[extrinsic_call]
        grant_guardian_approval(origin, ASSET);

        assert_eq!(GuardApprovals::<T>::get(ASSET), 1u64);
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
}
