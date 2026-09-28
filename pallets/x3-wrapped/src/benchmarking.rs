//! Benchmarking setup for `pallet-x3-wrapped`.
//!
//! All seven dispatchables charged literals — 8,000 to 20,000 picoseconds — behind a
//! `runtime-benchmarks` feature with nothing behind it, so none had been measured.
//!
//! Every call but the first operates on a registered asset, so the setups register one through the
//! pallet's own governance call, and the mint/burn pair establishes the wrapped supply it moves.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::traits::EnsureOrigin;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
    use super::*;

    const ASSET: AssetId = [0x01; 32];
    const CHAIN: ChainId = 1;
    const AMOUNT: u32 = 1_000;

    fn asset_config<T: Config>() -> WrappedAssetConfig<T::Balance> {
        WrappedAssetConfig {
            native_asset_id: ASSET,
            max_wrapped_supply: 1_000_000u32.into(),
            governance_weight_bps: 10_000,
            bridge_fee_bps: 5,
            status: WrappedAssetStatus::Active,
        }
    }

    /// Register the asset every other call needs, through the pallet's own governance call.
    fn register<T: Config>() -> Result<(), BenchmarkError> {
        let origin =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        Pallet::<T>::register_wrapped_asset(origin, ASSET, asset_config::<T>())?;
        Ok(())
    }

    /// Put wrapped supply on the chain so a burn has something to burn.
    fn mint<T: Config>() -> Result<(), BenchmarkError>
    where
        T: Config,
    {
        register::<T>()?;
        let origin =
            T::BridgeAuthority::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        let recipient: T::AccountId = whitelisted_caller();
        Pallet::<T>::mint_wrapped(origin, CHAIN, ASSET, recipient, AMOUNT.into(), 0u64)?;
        Ok(())
    }

    #[benchmark]
    fn register_wrapped_asset() -> Result<(), BenchmarkError> {
        let origin =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        register_wrapped_asset(origin, ASSET, asset_config::<T>());

        assert!(RegisteredWrappedAssets::<T>::contains_key(ASSET));
        Ok(())
    }

    #[benchmark]
    fn mint_wrapped() -> Result<(), BenchmarkError> {
        register::<T>()?;
        let origin =
            T::BridgeAuthority::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        let recipient: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        mint_wrapped(origin, CHAIN, ASSET, recipient, AMOUNT.into(), 0u64);

        assert_eq!(WrappedSupply::<T>::get(CHAIN, ASSET), AMOUNT.into());
        Ok(())
    }

    #[benchmark]
    fn burn_wrapped() -> Result<(), BenchmarkError> {
        mint::<T>()?;
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        burn_wrapped(RawOrigin::Signed(caller), CHAIN, ASSET, AMOUNT.into());

        assert_eq!(WrappedSupply::<T>::get(CHAIN, ASSET), 0u32.into());
        Ok(())
    }

    #[benchmark]
    fn update_governance_power() -> Result<(), BenchmarkError> {
        mint::<T>()?;
        let account: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        update_governance_power(RawOrigin::Signed(account.clone()), account.clone());

        assert!(GovernancePowerMap::<T>::contains_key(&account));
        Ok(())
    }

    #[benchmark]
    fn pause_wrapped_asset() -> Result<(), BenchmarkError> {
        register::<T>()?;
        let origin =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        pause_wrapped_asset(origin, ASSET);

        assert_eq!(
            RegisteredWrappedAssets::<T>::get(ASSET).map(|config| config.status),
            Some(WrappedAssetStatus::Paused)
        );
        Ok(())
    }

    #[benchmark]
    fn resume_wrapped_asset() -> Result<(), BenchmarkError> {
        register::<T>()?;
        let governance =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        // A resume only means something on a paused asset.
        Pallet::<T>::pause_wrapped_asset(governance.clone(), ASSET)?;
        let origin = governance;

        #[extrinsic_call]
        resume_wrapped_asset(origin, ASSET);

        assert_eq!(
            RegisteredWrappedAssets::<T>::get(ASSET).map(|config| config.status),
            Some(WrappedAssetStatus::Active)
        );
        Ok(())
    }

    #[benchmark]
    fn set_bridge_fee() -> Result<(), BenchmarkError> {
        register::<T>()?;
        let origin =
            T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        set_bridge_fee(origin, ASSET, 10u32);

        assert_eq!(
            RegisteredWrappedAssets::<T>::get(ASSET).map(|config| config.bridge_fee_bps),
            Some(10u32)
        );
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
