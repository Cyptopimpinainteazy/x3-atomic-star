//! Benchmarking setup for `pallet-x3-partner`.
//!
//! All eight dispatchables charged literals — 45,000,000 to 70,000,000 picoseconds
//! plus hand-counted reads and writes — behind a `runtime-benchmarks` feature
//! whose only entry was `frame-support/runtime-benchmarks`. The three benchmarks
//! that need an approved lane establish it through
//! `pallet-x3-inventory`'s own `register_lane`, not by writing its storage.
//!
//! Balances are `T::Balance::zero()` because this `Config` does not guarantee a
//! conversion from an integer; `pallet-x3-inventory`'s own benchmarks do the same.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
// `PartnerId`/`LaneId`/`PartnerStatus` live in `pallet-x3-inventory`'s `types`
// module and are only `use`d inside the pallet module, so they are not
// reachable through `pub use pallet::*`.
use pallet_x3_inventory::types::{
    AssetId, ChainId, LaneClass, LaneId, LiquiditySourceType, PartnerId, PartnerStatus,
};
use sp_runtime::traits::Zero;

#[benchmarks]
mod benchmarks {
    use super::*;

    /// `PartnerId` and `LaneId` are both `[u8; 32]`.
    const PARTNER_ID: PartnerId = [0xAA; 32];
    const LANE_ID: LaneId = [0x11; 32];

    /// Register a lane in `pallet-x3-inventory` so the partner calls can reference it.
    fn setup_lane<T: Config>() -> Result<(), BenchmarkError> {
        let allowed: BoundedVec<LiquiditySourceType, T::MaxLiquiditySources> =
            BoundedVec::default();
        let zero = <T as pallet_x3_inventory::pallet::Config>::Balance::zero();
        pallet_x3_inventory::Pallet::<T>::register_lane(
            RawOrigin::Root.into(),
            LANE_ID,
            1u32 as ChainId,
            2u32 as ChainId,
            1u32 as AssetId,
            2u32 as AssetId,
            LaneClass::A,
            allowed,
            zero,
            zero,
        )
        .map_err(|_| BenchmarkError::Weightless)
    }

    /// Register the partner under test, with a limit of zero exposure.
    fn setup_partner<T: Config>() -> Result<(), BenchmarkError> {
        Pallet::<T>::register_partner(
            RawOrigin::Root.into(),
            PARTNER_ID,
            <T as pallet_x3_inventory::pallet::Config>::Balance::zero(),
        )
        .map_err(|_| BenchmarkError::Weightless)
    }

    fn setup_approved_lane<T: Config>() -> Result<(), BenchmarkError> {
        setup_lane::<T>()?;
        setup_partner::<T>()?;
        Pallet::<T>::add_approved_lane(RawOrigin::Root.into(), PARTNER_ID, LANE_ID)
            .map_err(|_| BenchmarkError::Weightless)
    }

    #[benchmark]
    fn register_partner() -> Result<(), BenchmarkError> {
        let zero = <T as pallet_x3_inventory::pallet::Config>::Balance::zero();

        #[extrinsic_call]
        register_partner(RawOrigin::Root, PARTNER_ID, zero);

        assert!(Partners::<T>::contains_key(PARTNER_ID));
        Ok(())
    }

    #[benchmark]
    fn add_approved_lane() -> Result<(), BenchmarkError> {
        setup_lane::<T>()?;
        setup_partner::<T>()?;

        #[extrinsic_call]
        add_approved_lane(RawOrigin::Root, PARTNER_ID, LANE_ID);

        assert!(LanePartners::<T>::get(LANE_ID).contains(&PARTNER_ID));
        Ok(())
    }

    #[benchmark]
    fn remove_approved_lane() -> Result<(), BenchmarkError> {
        setup_approved_lane::<T>()?;

        #[extrinsic_call]
        remove_approved_lane(RawOrigin::Root, PARTNER_ID, LANE_ID);

        assert!(!LanePartners::<T>::get(LANE_ID).contains(&PARTNER_ID));
        Ok(())
    }

    #[benchmark]
    fn update_health_metrics() -> Result<(), BenchmarkError> {
        setup_partner::<T>()?;

        #[extrinsic_call]
        update_health_metrics(
            RawOrigin::Root,
            PARTNER_ID,
            250u32,
            9_000u32,
            100u32,
            100u32,
            0u32,
        );

        assert!(Partners::<T>::get(PARTNER_ID).is_some());
        Ok(())
    }

    #[benchmark]
    fn record_exposure() -> Result<(), BenchmarkError> {
        setup_partner::<T>()?;
        let caller: T::AccountId = whitelisted_caller();
        let zero = <T as pallet_x3_inventory::pallet::Config>::Balance::zero();

        #[extrinsic_call]
        record_exposure(RawOrigin::Signed(caller), PARTNER_ID, zero, false);

        assert!(Partners::<T>::contains_key(PARTNER_ID));
        Ok(())
    }

    #[benchmark]
    fn suspend_partner() -> Result<(), BenchmarkError> {
        setup_partner::<T>()?;

        #[extrinsic_call]
        suspend_partner(RawOrigin::Root, PARTNER_ID);

        assert_eq!(
            Partners::<T>::get(PARTNER_ID).map(|p| p.status),
            Some(PartnerStatus::Suspended)
        );
        Ok(())
    }

    #[benchmark]
    fn reinstate_partner() -> Result<(), BenchmarkError> {
        setup_partner::<T>()?;
        Pallet::<T>::suspend_partner(RawOrigin::Root.into(), PARTNER_ID)
            .map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        reinstate_partner(RawOrigin::Root, PARTNER_ID);

        assert_eq!(
            Partners::<T>::get(PARTNER_ID).map(|p| p.status),
            Some(PartnerStatus::Active)
        );
        Ok(())
    }

    #[benchmark]
    fn terminate_partner() -> Result<(), BenchmarkError> {
        setup_partner::<T>()?;

        #[extrinsic_call]
        terminate_partner(RawOrigin::Root, PARTNER_ID);

        assert_eq!(
            Partners::<T>::get(PARTNER_ID).map(|p| p.status),
            Some(PartnerStatus::Terminated)
        );
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
