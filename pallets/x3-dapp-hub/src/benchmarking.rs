//! Benchmarking setup for `pallet-x3-dapp-hub`.
//!
//! All eight dispatchables charged literals — 15,000 to record revenue, 5,000
//! for every governance transition — behind a `runtime-benchmarks` feature that
//! had nothing in it. Each benchmark establishes its state through the pallet's
//! own extrinsics: the split policy through `set_revenue_policy`, the dApp
//! through `register_dapp`, and approval through `approve_dapp`, so what is
//! measured is the state a real chain reaches rather than one written into
//! storage directly.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use x3_revenue_sharing::RevenueSplitEntry;

#[benchmarks]
mod benchmarks {
    use super::*;

    /// The split policy every benchmark registers: 30 % treasury, 70 % developer.
    const POLICY_ID: u32 = 7;
    const CATEGORY_ID: u32 = 1;
    /// Gross revenue to record. 70 % of this becomes the developer's earnings.
    const GROSS: u128 = 1_000_000;
    /// Strictly less than the developer share of `GROSS`, so the withdrawal succeeds.
    const WITHDRAW: u128 = 700_000;

    fn empty_entry() -> RevenueSplitEntry {
        RevenueSplitEntry {
            destination: RevenueDestination::Treasury,
            share_bps: 0,
        }
    }

    /// A policy whose active entries sum to exactly 10 000 bps.
    fn policy() -> RevenueSplitPolicy {
        RevenueSplitPolicy {
            policy_id: POLICY_ID,
            entries_len: 2,
            entries: [
                RevenueSplitEntry {
                    destination: RevenueDestination::Treasury,
                    share_bps: 3_000,
                },
                RevenueSplitEntry {
                    destination: RevenueDestination::DeveloperAccount,
                    share_bps: 7_000,
                },
                empty_entry(),
                empty_entry(),
                empty_entry(),
                empty_entry(),
                empty_entry(),
                empty_entry(),
            ],
        }
    }

    fn install_policy<T: Config>() -> Result<(), BenchmarkError> {
        Pallet::<T>::set_revenue_policy(RawOrigin::Root.into(), POLICY_ID, policy())
            .map_err(|_| BenchmarkError::Weightless)
    }

    /// Register a dApp through the pallet's own extrinsic, returning its id.
    ///
    /// Named `setup_dapp` rather than `register_dapp`: the `#[benchmarks]`
    /// macro generates a module named after each benchmark, so a helper sharing
    /// an extrinsic's name collides with it.
    fn setup_dapp<T: Config>(developer: &T::AccountId) -> Result<DAppId, BenchmarkError> {
        install_policy::<T>()?;
        let id = NextDAppId::<T>::get();
        Pallet::<T>::register_dapp(
            RawOrigin::Signed(developer.clone()).into(),
            CATEGORY_ID,
            POLICY_ID,
        )
        .map_err(|_| BenchmarkError::Weightless)?;
        Ok(id)
    }

    /// Register and approve a dApp, the precondition `record_revenue` needs.
    fn setup_approved_dapp<T: Config>(developer: &T::AccountId) -> Result<DAppId, BenchmarkError> {
        let id = setup_dapp::<T>(developer)?;
        Pallet::<T>::approve_dapp(RawOrigin::Root.into(), id)
            .map_err(|_| BenchmarkError::Weightless)?;
        Ok(id)
    }

    #[benchmark]
    fn register_dapp() -> Result<(), BenchmarkError> {
        install_policy::<T>()?;
        let developer: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        register_dapp(RawOrigin::Signed(developer.clone()), CATEGORY_ID, POLICY_ID);

        assert_eq!(DeveloperDApps::<T>::get(&developer).len(), 1);
        Ok(())
    }

    #[benchmark]
    fn approve_dapp() -> Result<(), BenchmarkError> {
        let developer: T::AccountId = whitelisted_caller();
        let id = setup_dapp::<T>(&developer)?;

        #[extrinsic_call]
        approve_dapp(RawOrigin::Root, id);

        assert_eq!(
            DApps::<T>::get(id).map(|d| d.approval_status),
            Some(ApprovalStatus::Approved)
        );
        Ok(())
    }

    #[benchmark]
    fn reject_dapp() -> Result<(), BenchmarkError> {
        let developer: T::AccountId = whitelisted_caller();
        let id = setup_dapp::<T>(&developer)?;

        #[extrinsic_call]
        reject_dapp(RawOrigin::Root, id);

        assert_eq!(
            DApps::<T>::get(id).map(|d| d.approval_status),
            Some(ApprovalStatus::Rejected)
        );
        Ok(())
    }

    #[benchmark]
    fn suspend_dapp() -> Result<(), BenchmarkError> {
        let developer: T::AccountId = whitelisted_caller();
        let id = setup_dapp::<T>(&developer)?;

        #[extrinsic_call]
        suspend_dapp(RawOrigin::Root, id);

        assert_eq!(
            DApps::<T>::get(id).map(|d| d.approval_status),
            Some(ApprovalStatus::Suspended)
        );
        Ok(())
    }

    #[benchmark]
    fn set_placement() -> Result<(), BenchmarkError> {
        let developer: T::AccountId = whitelisted_caller();
        let id = setup_dapp::<T>(&developer)?;

        #[extrinsic_call]
        set_placement(RawOrigin::Root, id, PlacementTier::Premium);

        assert_eq!(
            DApps::<T>::get(id).map(|d| d.placement),
            Some(PlacementTier::Premium)
        );
        Ok(())
    }

    #[benchmark]
    fn record_revenue() -> Result<(), BenchmarkError> {
        let developer: T::AccountId = whitelisted_caller();
        let id = setup_approved_dapp::<T>(&developer)?;

        #[extrinsic_call]
        record_revenue(RawOrigin::Root, id, GROSS);

        assert_eq!(DeveloperEarnings::<T>::get(&developer), 700_000);
        Ok(())
    }

    #[benchmark]
    fn set_revenue_policy() -> Result<(), BenchmarkError> {
        #[extrinsic_call]
        set_revenue_policy(RawOrigin::Root, POLICY_ID, policy());

        assert!(RevenuePolicies::<T>::contains_key(POLICY_ID));
        Ok(())
    }

    #[benchmark]
    fn withdraw_earnings() -> Result<(), BenchmarkError> {
        let developer: T::AccountId = whitelisted_caller();
        let id = setup_approved_dapp::<T>(&developer)?;
        Pallet::<T>::record_revenue(RawOrigin::Root.into(), id, GROSS)
            .map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        withdraw_earnings(RawOrigin::Signed(developer.clone()), WITHDRAW);

        assert_eq!(DeveloperEarnings::<T>::get(&developer), 0);
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
