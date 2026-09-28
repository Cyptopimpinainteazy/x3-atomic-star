//! Benchmarking setup for `pallet-x3-reconciliation`.
//!
//! Six dispatchables charged literals — 15,000 to report a chain's supply, 30,000 to run a
//! reconciliation, 20,000 to update or aggregate governance power — behind a `runtime-benchmarks`
//! feature with nothing behind it.
//!
//! The reconciliation and the halt-lift both read state the caller does not supply, so their setups
//! establish it through the pallet's own extrinsics or storage: a reconciliation needs a canonical
//! supply and at least one chain report to compute a divergence, and a halt lift needs an active
//! halt to lift.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::traits::EnsureOrigin;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
    use super::*;

    const CHAIN: u32 = 1;
    const SUPPLY: u128 = 1_000_000;

    #[benchmark]
    fn submit_chain_supply_report() -> Result<(), BenchmarkError> {
        let origin =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        submit_chain_supply_report(origin, CHAIN, SUPPLY);

        assert!(ChainSupplyReports::<T>::contains_key(CHAIN));
        Ok(())
    }

    #[benchmark]
    fn set_canonical_supply() -> Result<(), BenchmarkError> {
        let origin =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        set_canonical_supply(origin, SUPPLY);

        assert_eq!(CanonicalSupply::<T>::get(), SUPPLY);
        Ok(())
    }

    #[benchmark]
    fn run_reconciliation() -> Result<(), BenchmarkError> {
        let governance =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        // A reconciliation compares the canonical supply with the sum of the chain reports; both
        // sides are set through the pallet's own extrinsics.
        Pallet::<T>::set_canonical_supply(governance.clone(), SUPPLY)?;
        Pallet::<T>::submit_chain_supply_report(governance, CHAIN, SUPPLY)?;
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        run_reconciliation(RawOrigin::Signed(caller));

        assert!(LastReconciliation::<T>::get().is_some());
        Ok(())
    }

    #[benchmark]
    fn lift_mint_halt() -> Result<(), BenchmarkError> {
        let origin =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        // The lift requires an active halt, and refuses if the last reconciliation was still above
        // tolerance — with no reconciliation record it lifts.
        MintHaltSince::<T>::put(Some(frame_system::Pallet::<T>::block_number()));

        #[extrinsic_call]
        lift_mint_halt(origin);

        assert!(MintHaltSince::<T>::get().is_none());
        Ok(())
    }

    #[benchmark]
    fn update_governance_power() -> Result<(), BenchmarkError> {
        let origin =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        update_governance_power(origin, CHAIN, SUPPLY);

        assert_eq!(TotalGovernancePower::<T>::get(), SUPPLY);
        Ok(())
    }

    #[benchmark]
    fn aggregate_governance_power() -> Result<(), BenchmarkError> {
        GovernancePowerByChain::<T>::insert(1u32, SUPPLY);
        GovernancePowerByChain::<T>::insert(2u32, SUPPLY);
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        aggregate_governance_power(RawOrigin::Signed(caller));

        assert_eq!(TotalGovernancePower::<T>::get(), SUPPLY * 2);
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
