//! Benchmarking setup for `pallet-x3-custody`.
//!
//! All ten dispatchables charged hand-written literals — 10,000 down to 4,000
//! picoseconds with no read or write count at all — behind a
//! `runtime-benchmarks` feature whose only entries were the two frame
//! passthroughs.
//!
//! Governance- and operator-gated calls take their origin from
//! `try_successful_origin()` rather than naming one, because the two runtimes
//! disagree on purpose: the mock models an operator as *any signed account*
//! (`EnsureSigned`, asserted by its own tests 15 and 16), while this runtime
//! requires root or half the council. `try_successful_origin` is implemented for
//! both and returns the origin each `Config` actually accepts.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
// `try_successful_origin` is a method of `EnsureOrigin`, gated behind
// `runtime-benchmarks`; the trait has to be in scope for it to resolve.
use frame_support::traits::EnsureOrigin;
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};

#[benchmarks]
mod benchmarks {
    use super::*;

    const CHAIN_ID: u32 = 1;
    const ASSET_ID: u32 = 2;
    /// `ValidatorSigning` + `Operational` is the one combination the pallet
    /// refuses, so every benchmark uses a role/tier pair that is allowed.
    const ROLE: KeyRole = KeyRole::TreasuryOperational;
    const TIER: AuthorizationTier = AuthorizationTier::Operational;

    fn governance<T: Config>() -> Result<T::RuntimeOrigin, BenchmarkError> {
        T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)
    }

    fn operator<T: Config>() -> Result<T::RuntimeOrigin, BenchmarkError> {
        T::OperatorOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)
    }

    /// Register `signer` for (CHAIN_ID, ASSET_ID) through the pallet's own extrinsic.
    fn setup_signer<T: Config>(signer: &T::AccountId) -> Result<(), BenchmarkError> {
        Pallet::<T>::register_signer(
            governance::<T>()?,
            CHAIN_ID,
            ASSET_ID,
            signer.clone(),
            TIER,
            ROLE,
        )
        .map_err(|_| BenchmarkError::Weightless)
    }

    /// Register an active validator key for `account`.
    fn setup_validator_key<T: Config>(account: &T::AccountId) -> Result<(), BenchmarkError> {
        let due = frame_system::Pallet::<T>::block_number();
        Pallet::<T>::register_validator_key(governance::<T>()?, account.clone(), due)
            .map_err(|_| BenchmarkError::Weightless)
    }

    #[benchmark]
    fn register_signer() -> Result<(), BenchmarkError> {
        let signer: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        register_signer(
            governance::<T>()?,
            CHAIN_ID,
            ASSET_ID,
            signer.clone(),
            TIER,
            ROLE,
        );

        assert!(Pallet::<T>::is_signer_authorized(
            CHAIN_ID, ASSET_ID, &signer, TIER
        ));
        Ok(())
    }

    #[benchmark]
    fn deactivate_signer() -> Result<(), BenchmarkError> {
        let signer: T::AccountId = whitelisted_caller();
        setup_signer::<T>(&signer)?;

        #[extrinsic_call]
        deactivate_signer(governance::<T>()?, CHAIN_ID, ASSET_ID, signer.clone());

        assert!(!Pallet::<T>::is_signer_authorized(
            CHAIN_ID, ASSET_ID, &signer, TIER
        ));
        Ok(())
    }

    #[benchmark]
    fn register_validator_key() -> Result<(), BenchmarkError> {
        let account: T::AccountId = whitelisted_caller();
        let due: BlockNumberFor<T> = frame_system::Pallet::<T>::block_number();

        #[extrinsic_call]
        register_validator_key(governance::<T>()?, account.clone(), due);

        assert!(ValidatorKeyRegistry::<T>::contains_key(&account));
        Ok(())
    }

    #[benchmark]
    fn rotate_validator_key() -> Result<(), BenchmarkError> {
        let old_key: T::AccountId = whitelisted_caller();
        setup_validator_key::<T>(&old_key)?;
        let new_key: T::AccountId = frame_benchmarking::account("new_key", 0, 0);

        #[extrinsic_call]
        rotate_validator_key(governance::<T>()?, old_key.clone(), new_key.clone());

        assert!(ValidatorKeyRegistry::<T>::contains_key(&new_key));
        assert!(!ValidatorKeyRegistry::<T>::get(&old_key)
            .map(|r| r.active)
            .unwrap_or(true));
        Ok(())
    }

    #[benchmark]
    fn set_tier_threshold() -> Result<(), BenchmarkError> {
        #[extrinsic_call]
        set_tier_threshold(governance::<T>()?, TIER, 2u32);

        assert!(TierThresholds::<T>::contains_key(TIER));
        Ok(())
    }

    #[benchmark]
    fn set_signer_limit() -> Result<(), BenchmarkError> {
        let signer: T::AccountId = whitelisted_caller();
        let policy = SignerPolicy {
            max_single_op_amount: 0u128,
            max_daily_aggregate: 0u128,
            allowed_tiers: 0u8,
        };

        #[extrinsic_call]
        set_signer_limit(operator::<T>()?, signer.clone(), policy);

        assert!(SignerLimits::<T>::contains_key(&signer));
        Ok(())
    }

    #[benchmark]
    fn set_key_rotation_schedule() -> Result<(), BenchmarkError> {
        let signer: T::AccountId = whitelisted_caller();
        let block: BlockNumberFor<T> = frame_system::Pallet::<T>::block_number();

        #[extrinsic_call]
        set_key_rotation_schedule(operator::<T>()?, signer.clone(), block);

        assert!(KeyRotationSchedule::<T>::contains_key(&signer));
        Ok(())
    }

    #[benchmark]
    fn check_signer_authorized() -> Result<(), BenchmarkError> {
        let signer: T::AccountId = whitelisted_caller();
        setup_signer::<T>(&signer)?;
        let caller: T::AccountId = frame_benchmarking::account("caller", 0, 0);

        #[extrinsic_call]
        check_signer_authorized(RawOrigin::Signed(caller), CHAIN_ID, ASSET_ID, signer, TIER);

        Ok(())
    }

    #[benchmark]
    fn authorize_gateway() -> Result<(), BenchmarkError> {
        let account: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        authorize_gateway(governance::<T>()?, GatewayRole::X3Lang, account.clone());

        assert!(AuthorizedGateways::<T>::contains_key(
            GatewayRole::X3Lang,
            &account
        ));
        Ok(())
    }

    #[benchmark]
    fn revoke_gateway() -> Result<(), BenchmarkError> {
        let account: T::AccountId = whitelisted_caller();
        Pallet::<T>::authorize_gateway(governance::<T>()?, GatewayRole::X3Lang, account.clone())
            .map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        revoke_gateway(governance::<T>()?, GatewayRole::X3Lang, account.clone());

        assert!(!AuthorizedGateways::<T>::contains_key(
            GatewayRole::X3Lang,
            &account
        ));
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
