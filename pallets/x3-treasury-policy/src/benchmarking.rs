//! Benchmarking setup for `pallet-x3-treasury-policy`.
//!
//! All eight dispatchables charged literals — 80,000,000 picoseconds for a vault funding, 50,000,000
//! for setting a cap — and none of them had ever been measured: the pallet shipped a
//! `runtime-benchmarks` feature with nothing behind it, and the runtime's feature list did not enable
//! it either, so `define_benchmarks!` could not see the pallet at all.
//!
//! The vault funding path needs real state, because `fund_settlement_vault` refuses an unknown vault
//! (`VaultNotFound`), an uncapped triple (`AllocationCapNotSet`) and — with the default threshold of
//! zero — applies anything non-zero only through the governance queue. The setup therefore creates
//! the vault with the inventory pallet's own root-only call and sets a cap and a threshold, so the
//! measured call is the immediate-apply path a chain takes.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::traits::EnsureOrigin;
use frame_system::RawOrigin;
use pallet_x3_inventory::types::{OwnerType, VaultId, VaultType};

#[benchmarks]
mod benchmarks {
    use super::*;

    const VAULT_ID: VaultId = [7u8; 32];
    const CHAIN_ID: ChainId = 1;
    const ASSET_ID: AssetId = 1;
    const AMOUNT: Balance = 1_000_000;

    fn cap_key() -> AllocationCapKey {
        AllocationCapKey {
            chain_id: CHAIN_ID,
            asset_id: ASSET_ID,
            lane_class: LaneClass::A,
        }
    }

    /// Register the settlement vault the funding path requires.
    ///
    /// `create_vault` is root-only, and the treasury's `Config` already requires the inventory's, so
    /// this needs no extra bound on `T`.
    fn create_settlement_vault<T: Config>() -> Result<(), BenchmarkError> {
        let zero = <T as pallet_x3_inventory::pallet::Config>::Balance::default();
        pallet_x3_inventory::Pallet::<T>::create_vault(
            RawOrigin::Root.into(),
            VAULT_ID,
            VaultType::SettlementFloat,
            OwnerType::Treasury,
            CHAIN_ID,
            ASSET_ID,
            zero,
            zero,
            zero,
            zero,
        )?;
        Ok(())
    }

    #[benchmark]
    fn set_allocation_cap() -> Result<(), BenchmarkError> {
        let origin =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        set_allocation_cap(origin, cap_key(), AMOUNT);

        assert_eq!(AllocationCaps::<T>::get(cap_key()), Some(AMOUNT));
        Ok(())
    }

    #[benchmark]
    fn set_operator_funding_threshold() -> Result<(), BenchmarkError> {
        let origin =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        set_operator_funding_threshold(origin, AMOUNT);

        assert_eq!(OperatorFundingThreshold::<T>::get(), AMOUNT);
        Ok(())
    }

    #[benchmark]
    fn fund_settlement_vault() -> Result<(), BenchmarkError> {
        create_settlement_vault::<T>()?;
        let governance =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        Pallet::<T>::set_allocation_cap(governance.clone(), cap_key(), AMOUNT)?;
        Pallet::<T>::set_operator_funding_threshold(governance, AMOUNT)?;

        let origin =
            T::OperatorOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        fund_settlement_vault(origin, VAULT_ID, AMOUNT, LaneClass::A, CHAIN_ID, ASSET_ID);

        assert_eq!(TreasuryDeployedByLaneClass::<T>::get(LaneClass::A), AMOUNT);
        Ok(())
    }

    #[benchmark]
    fn approve_governance_action() -> Result<(), BenchmarkError> {
        create_settlement_vault::<T>()?;
        PendingGovernanceActions::<T>::insert(
            VAULT_ID,
            (
                AMOUNT,
                LaneClass::A,
                frame_system::Pallet::<T>::block_number(),
            ),
        );
        let origin =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        approve_governance_action(origin, VAULT_ID);

        assert!(PendingGovernanceActions::<T>::get(VAULT_ID).is_none());
        assert_eq!(TreasuryDeployedByLaneClass::<T>::get(LaneClass::A), AMOUNT);
        Ok(())
    }

    #[benchmark]
    fn reject_governance_action() -> Result<(), BenchmarkError> {
        PendingGovernanceActions::<T>::insert(
            VAULT_ID,
            (
                AMOUNT,
                LaneClass::A,
                frame_system::Pallet::<T>::block_number(),
            ),
        );
        let origin =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        reject_governance_action(origin, VAULT_ID);

        assert!(PendingGovernanceActions::<T>::get(VAULT_ID).is_none());
        Ok(())
    }

    #[benchmark]
    fn withdraw_from_vault() -> Result<(), BenchmarkError> {
        TreasuryDeployedByLaneClass::<T>::insert(LaneClass::A, AMOUNT);
        let origin =
            T::OperatorOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        withdraw_from_vault(origin, VAULT_ID, AMOUNT, LaneClass::A);

        assert_eq!(TreasuryDeployedByLaneClass::<T>::get(LaneClass::A), 0);
        Ok(())
    }

    #[benchmark]
    fn deposit_insurance_reserve() -> Result<(), BenchmarkError> {
        let origin =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        deposit_insurance_reserve(origin, AMOUNT);

        assert_eq!(InsuranceReserveBalance::<T>::get(), AMOUNT);
        Ok(())
    }

    #[benchmark]
    fn withdraw_insurance_reserve() -> Result<(), BenchmarkError> {
        InsuranceReserveBalance::<T>::put(AMOUNT);
        let origin =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        withdraw_insurance_reserve(origin, AMOUNT);

        assert_eq!(InsuranceReserveBalance::<T>::get(), 0);
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
