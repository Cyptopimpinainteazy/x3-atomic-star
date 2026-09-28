//! Benchmarking setup for `pallet-x3-asset-registry`.
//!
//! All seven dispatchables charged literals — 25,000 to register an asset, 10,000-15,000 for every
//! lifecycle move and route change — and none had been measured: the pallet had no
//! `runtime-benchmarks` feature and no `weights.rs`.
//!
//! Six of the seven act on an asset that already exists, so each setup registers one through the
//! pallet's own `do_register_asset` (the function the extrinsic calls, and the one the token factory
//! calls), and `set_route_enabled` additionally configures the route it toggles.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::traits::EnsureOrigin;
use sp_std::vec;
use x3_asset_kernel_types::{
    AssetId, AssetStatus, DomainId, RouteConfig, RouteLimits, SupplyPolicy,
};

#[benchmarks]
mod benchmarks {
    use super::*;

    const SYMBOL: &[u8] = b"XBENCH";
    const NAME: &[u8] = b"Benchmark Asset";

    fn register<T: Config>() -> Result<AssetId, BenchmarkError> {
        Pallet::<T>::do_register_asset(
            SYMBOL.to_vec(),
            NAME.to_vec(),
            18u8,
            DomainId::X3Native,
            1u64,
            vec![0xABu8; 20],
            SupplyPolicy::NativeMintBurn,
        )
        .map_err(|_| BenchmarkError::Weightless)
    }

    fn configure<T: Config>(asset_id: AssetId) -> Result<(), BenchmarkError> {
        Pallet::<T>::do_configure_route(
            &asset_id,
            DomainId::X3Native,
            DomainId::X3Evm,
            RouteConfig::internal(RouteLimits::DEV_PERMISSIVE, 100),
        )
        .map_err(|_| BenchmarkError::Weightless)
    }

    #[benchmark]
    fn register_asset() -> Result<(), BenchmarkError> {
        let origin =
            T::RegistryOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        register_asset(
            origin,
            SYMBOL.to_vec(),
            NAME.to_vec(),
            18u8,
            DomainId::X3Native,
            1u64,
            vec![0xABu8; 20],
            SupplyPolicy::NativeMintBurn,
        );

        assert_eq!(TotalAssets::<T>::get(), 1);
        Ok(())
    }

    #[benchmark]
    fn activate_asset() -> Result<(), BenchmarkError> {
        let asset_id = register::<T>()?;
        let origin =
            T::RegistryOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        activate_asset(origin, asset_id);

        assert_eq!(
            Assets::<T>::get(asset_id).map(|meta| meta.status),
            Some(AssetStatus::Active)
        );
        Ok(())
    }

    #[benchmark]
    fn pause_asset() -> Result<(), BenchmarkError> {
        let asset_id = register::<T>()?;
        let origin = T::EmergencyPauseOrigin::try_successful_origin()
            .map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        pause_asset(origin, asset_id);

        assert_eq!(
            Assets::<T>::get(asset_id).map(|meta| meta.status),
            Some(AssetStatus::Paused)
        );
        Ok(())
    }

    #[benchmark]
    fn unpause_asset() -> Result<(), BenchmarkError> {
        let asset_id = register::<T>()?;
        // A pause has to be in place for the unpause to mean anything.
        let emergency = T::EmergencyPauseOrigin::try_successful_origin()
            .map_err(|_| BenchmarkError::Weightless)?;
        Pallet::<T>::pause_asset(emergency, asset_id)?;
        let origin =
            T::RegistryOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        unpause_asset(origin, asset_id);

        assert_eq!(
            Assets::<T>::get(asset_id).map(|meta| meta.status),
            Some(AssetStatus::Active)
        );
        Ok(())
    }

    #[benchmark]
    fn retire_asset() -> Result<(), BenchmarkError> {
        let asset_id = register::<T>()?;
        let origin =
            T::RegistryOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        retire_asset(origin, asset_id);

        assert_eq!(
            Assets::<T>::get(asset_id).map(|meta| meta.status),
            Some(AssetStatus::Retired)
        );
        Ok(())
    }

    #[benchmark]
    fn configure_route() -> Result<(), BenchmarkError> {
        let asset_id = register::<T>()?;
        let origin =
            T::RegistryOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        configure_route(
            origin,
            asset_id,
            DomainId::X3Native,
            DomainId::X3Evm,
            RouteConfig::internal(RouteLimits::DEV_PERMISSIVE, 100),
        );

        assert!(Routes::<T>::contains_key(
            asset_id,
            (DomainId::X3Native, DomainId::X3Evm)
        ));
        Ok(())
    }

    #[benchmark]
    fn set_route_enabled() -> Result<(), BenchmarkError> {
        let asset_id = register::<T>()?;
        configure::<T>(asset_id)?;
        // Enabling is the registry-origin branch and disabling the emergency one; the measured call
        // takes the registry branch, which is the governance path a chain takes to open a corridor.
        let origin =
            T::RegistryOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        set_route_enabled(origin, asset_id, DomainId::X3Native, DomainId::X3Evm, true);

        assert!(Routes::<T>::contains_key(
            asset_id,
            (DomainId::X3Native, DomainId::X3Evm)
        ));
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
}
