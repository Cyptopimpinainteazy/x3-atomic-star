//! Benchmarking setup for `pallet-x3-reservation`.
//!
//! All three dispatchables charged 10,000 picoseconds — a request that locks real vault inventory and
//! increments the lane's unsettled notional, and the two terminal transitions that undo it — behind a
//! `runtime-benchmarks` feature that had nothing in it.
//!
//! Every amount here is a real one. The inventory pallet's `Balance` is generic with no `From<u32>`,
//! and its mutation helpers return early for a zero amount — a benchmark written with zero balances
//! would measure the early-return path and skip exactly the storage work these calls exist to do. The
//! module therefore requires the chain's balance to be `u128`, which is what this runtime and the
//! mock both use.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use pallet_x3_inventory::types::{
    LaneClass, LaneId, LiquiditySourceType, OwnerType, ReservationId, ReservationStatus, RouteId,
    VaultId, VaultType,
};
use sp_std::vec;

#[benchmarks(where T: Config + pallet_x3_inventory::pallet::Config<Balance = u128>)]
mod benchmarks {
    use super::*;

    const VAULT: VaultId = [1u8; 32];
    const LANE: LaneId = [2u8; 32];
    const ROUTE: RouteId = [3u8; 32];
    const RESERVATION: ReservationId = [4u8; 32];
    const AMOUNT: u128 = 1_000;
    const VAULT_FUNDS: u128 = 1_000_000;
    const SNAPSHOT: [u8; 32] = [9u8; 32];

    /// Create the funded vault and the lane a reservation needs, with the inventory pallet's own
    /// root-only calls, so the measured request locks real inventory.
    fn setup_vault_and_lane<T: Config>() -> Result<(), BenchmarkError>
    where
        T: pallet_x3_inventory::pallet::Config<Balance = u128>,
    {
        pallet_x3_inventory::Pallet::<T>::create_vault(
            RawOrigin::Root.into(),
            VAULT,
            VaultType::SettlementFloat,
            OwnerType::Protocol,
            1,
            100,
            0u128,
            0u128,
            0u128,
            0u128,
        )?;
        pallet_x3_inventory::Pallet::<T>::fund_vault(RawOrigin::Root.into(), VAULT, VAULT_FUNDS)?;

        let sources: BoundedVec<
            LiquiditySourceType,
            <T as pallet_x3_inventory::pallet::Config>::MaxLiquiditySources,
        > = BoundedVec::try_from(vec![LiquiditySourceType::ProtocolFloat])
            .map_err(|_| BenchmarkError::Weightless)?;
        pallet_x3_inventory::Pallet::<T>::register_lane(
            RawOrigin::Root.into(),
            LANE,
            1,
            2,
            100,
            200,
            LaneClass::C,
            sources,
            VAULT_FUNDS,
            VAULT_FUNDS,
        )?;
        Ok(())
    }

    fn request<T: Config>() -> Result<(), BenchmarkError>
    where
        T: pallet_x3_inventory::pallet::Config<Balance = u128>,
    {
        Pallet::<T>::request_reservation(
            RawOrigin::Root.into(),
            RESERVATION,
            ROUTE,
            VAULT,
            LANE,
            AMOUNT,
            SNAPSHOT,
        )?;
        Ok(())
    }

    #[benchmark]
    fn request_reservation() -> Result<(), BenchmarkError> {
        setup_vault_and_lane::<T>()?;

        #[extrinsic_call]
        request_reservation(
            RawOrigin::Root,
            RESERVATION,
            ROUTE,
            VAULT,
            LANE,
            AMOUNT,
            SNAPSHOT,
        );

        assert_eq!(
            Reservations::<T>::get(RESERVATION).map(|state| state.status),
            Some(ReservationStatus::Active)
        );
        Ok(())
    }

    #[benchmark]
    fn release_reservation() -> Result<(), BenchmarkError> {
        setup_vault_and_lane::<T>()?;
        request::<T>()?;

        #[extrinsic_call]
        release_reservation(RawOrigin::Root, RESERVATION);

        assert_eq!(
            Reservations::<T>::get(RESERVATION).map(|state| state.status),
            Some(ReservationStatus::Released)
        );
        Ok(())
    }

    #[benchmark]
    fn consume_reservation() -> Result<(), BenchmarkError> {
        setup_vault_and_lane::<T>()?;
        request::<T>()?;

        #[extrinsic_call]
        consume_reservation(RawOrigin::Root, RESERVATION);

        assert_eq!(
            Reservations::<T>::get(RESERVATION).map(|state| state.status),
            Some(ReservationStatus::Consumed)
        );
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
