//! Benchmarking setup for `pallet-x3-supply-ledger`.
//!
//! Five dispatchables charged literals — a canonical **mint** for 20,000 picoseconds, a **burn**
//! for 15,000 and three governance switches for 10,000 — while each one reads and writes the
//! `Ledgers` storage map. They are measured here.
//!
//! The mint and burn benchmarks have to register an asset first, because the ledger refuses an
//! unknown one (`UnknownAsset`) and the development genesis holds no assets at all. They register
//! it through the asset registry pallet, which is what a chain does, rather than by writing the
//! ledger's storage directly — the latter would have measured a state the chain cannot reach.
//!
//! Run with:
//! ```bash
//! cargo build --release -p x3-chain-node --features runtime-benchmarks
//! ./target/release/x3-chain-node benchmark pallet \
//!     --chain dev --pallet pallet_x3_supply_ledger --extrinsic "*" \
//!     --steps 50 --repeat 20 --wasm-execution=compiled --heap-pages=4096 \
//!     --template .maintain/frame-weight-template.hbs \
//!     --output pallets/x3-supply-ledger/src/weights.rs
//! ```

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::traits::EnsureOrigin;
use sp_std::vec;
use x3_asset_kernel_types::{AssetId, Balance, DomainId, SupplyPolicy};

/// Register an asset through the asset registry and return the id the registry derived for it.
///
/// `derive_asset_id` is the same function the registry uses, so the benchmark's mint/burn calls the
/// asset the setup registered rather than one that merely looks similar.
fn register_asset_in_registry<T>() -> Result<AssetId, BenchmarkError>
where
    T: pallet_x3_asset_registry::Config,
{
    let symbol = b"XBENCH".to_vec();
    let name = b"Benchmark Asset".to_vec();
    let canonical_decimals = 18u8;
    let origin_domain = DomainId::X3Native;
    let origin_chain_id = 1u64;
    let origin_address = vec![0xABu8; 20];
    let supply_policy = SupplyPolicy::NativeMintBurn;

    let asset_id = x3_asset_kernel_types::derive_asset_id(
        origin_domain,
        origin_chain_id,
        &origin_address,
        &symbol,
        canonical_decimals,
    );

    let origin = <T as pallet_x3_asset_registry::Config>::RegistryOrigin::try_successful_origin()
        .map_err(|_| BenchmarkError::Weightless)?;
    pallet_x3_asset_registry::Pallet::<T>::register_asset(
        origin,
        symbol,
        name,
        canonical_decimals,
        origin_domain,
        origin_chain_id,
        origin_address,
        supply_policy,
    )?;

    Ok(asset_id)
}

#[benchmarks(where T: Config + pallet_x3_asset_registry::Config)]
mod benchmarks {
    use super::*;

    /// Benchmark `mint_canonical`: governance mint of canonical supply into one domain leg.
    #[benchmark]
    fn mint_canonical() -> Result<(), BenchmarkError> {
        let asset_id = register_asset_in_registry::<T>()?;
        let origin =
            T::SupplyGovernance::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        let amount: Balance = 1_000_000u32.into();
        let nonce: u64 = 0;

        #[extrinsic_call]
        mint_canonical(origin, asset_id, DomainId::X3Native, amount, nonce);

        let ledger = Ledgers::<T>::get(asset_id).ok_or(BenchmarkError::Weightless)?;
        assert_eq!(ledger.canonical_supply, amount);
        Ok(())
    }

    /// Benchmark `burn_canonical`: governance burn of canonical supply from one domain leg.
    #[benchmark]
    fn burn_canonical() -> Result<(), BenchmarkError> {
        let asset_id = register_asset_in_registry::<T>()?;
        let minted: Balance = 2_000_000u32.into();
        let amount: Balance = 1_000_000u32.into();
        // The burn has to have something to burn; the seed is outside the measured section.
        Pallet::<T>::do_mint_canonical(&asset_id, DomainId::X3Native, minted)?;

        let origin =
            T::SupplyGovernance::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        burn_canonical(origin, asset_id, DomainId::X3Native, amount);

        let ledger = Ledgers::<T>::get(asset_id).ok_or(BenchmarkError::Weightless)?;
        assert_eq!(ledger.canonical_supply, minted - amount);
        Ok(())
    }

    /// Benchmark `set_invariant_violation_policy`.
    #[benchmark]
    fn set_invariant_violation_policy() -> Result<(), BenchmarkError> {
        let origin =
            T::SupplyGovernance::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        let policy = InvariantViolationPolicy::RejectNewTransfers;

        #[extrinsic_call]
        set_invariant_violation_policy(origin, policy);

        assert_eq!(InvariantPolicy::<T>::get(), policy);
        Ok(())
    }

    /// Benchmark `halt_transfers`.
    #[benchmark]
    fn halt_transfers() -> Result<(), BenchmarkError> {
        let origin =
            T::SupplyGovernance::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        halt_transfers(origin);

        assert!(TransferHalted::<T>::get());
        Ok(())
    }

    /// Benchmark `resume_transfers`.
    #[benchmark]
    fn resume_transfers() -> Result<(), BenchmarkError> {
        let origin =
            T::SupplyGovernance::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        // The resume switch is only reached with the halt in place.
        TransferHalted::<T>::put(true);

        #[extrinsic_call]
        resume_transfers(origin);

        assert!(!TransferHalted::<T>::get());
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
