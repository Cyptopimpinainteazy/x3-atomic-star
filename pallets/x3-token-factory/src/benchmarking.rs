//! Benchmarking setup for `pallet-x3-token-factory`.
//!
//! All four dispatchables charged literals — 60,000 picoseconds to launch a token, 20,000 to mint
//! or burn supply — and none of them had ever been measured: the pallet declared a
//! `runtime-benchmarks` feature with nothing behind it and no `WeightInfo`.
//!
//! `mint`, `burn` and `transfer_mint_authority` all operate on a launch produced by `create_token`,
//! so each establishes that state first, through the pallet's own extrinsic rather than by writing
//! `Tokens` directly. The launch is permissionless (`CreateTokenOrigin = EnsureSigned`), and the
//! account the origin yields is the mint authority the later calls need.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::{traits::EnsureOrigin, BoundedVec};
use frame_system::{pallet_prelude::OriginFor, RawOrigin};
use sp_std::vec;
use x3_asset_kernel_types::{AssetId, Balance, DomainId, TokenClass};

/// The `ledger` accessor the mint/burn benchmarks read their result from lives on this trait; the
/// pallet's `Config` requires it, but the method only resolves with the trait in scope.
use x3_asset_kernel_types::traits::SupplyLedgerWrite;

#[benchmarks]
mod benchmarks {
    use super::*;

    const INITIAL_SUPPLY: Balance = 1_000_000;
    const MAX_SUPPLY: Balance = 10_000_000;
    const MINT_AMOUNT: Balance = 1_000;
    const BURN_AMOUNT: Balance = 1_000;

    /// Build the launch config, refusing rather than panicking on a bound failure.
    ///
    /// The panic ratchet counts `expect`/`unwrap` in code that is not `#[cfg(test)]`, and a benchmark
    /// module is not — three `expect` calls here grew the baseline 440 → 443 and reddened
    /// `make mainnet-check`.
    fn launch_config(class: TokenClass) -> Result<TokenFactoryConfig, BenchmarkError> {
        Ok(TokenFactoryConfig {
            symbol: BoundedVec::try_from(b"XBENCH".to_vec())
                .map_err(|_| BenchmarkError::Weightless)?,
            name: BoundedVec::try_from(b"Benchmark Token".to_vec())
                .map_err(|_| BenchmarkError::Weightless)?,
            canonical_decimals: 18,
            initial_supply: INITIAL_SUPPLY,
            max_supply: Some(MAX_SUPPLY),
            class,
            enabled_domains: BoundedVec::try_from(vec![DomainId::X3Native, DomainId::X3Evm])
                .map_err(|_| BenchmarkError::Weightless)?,
        })
    }

    /// Launch a token exactly the way the extrinsic does and return its canonical id.
    fn launch_token<T: Config>(
        origin: OriginFor<T>,
        class: TokenClass,
    ) -> Result<AssetId, BenchmarkError> {
        Pallet::<T>::create_token(origin, launch_config(class)?)?;
        Tokens::<T>::iter_keys()
            .next()
            .ok_or(BenchmarkError::Weightless)
    }

    #[benchmark]
    fn create_token() -> Result<(), BenchmarkError> {
        let origin = T::CreateTokenOrigin::try_successful_origin()
            .map_err(|_| BenchmarkError::Weightless)?;
        let launch = launch_config(TokenClass::CappedMintable)?;

        #[extrinsic_call]
        create_token(origin, launch);

        assert_eq!(Tokens::<T>::iter().count(), 1);
        assert_eq!(FactoryNonce::<T>::get(), 1);
        Ok(())
    }

    #[benchmark]
    fn mint() -> Result<(), BenchmarkError> {
        let creator_origin = T::CreateTokenOrigin::try_successful_origin()
            .map_err(|_| BenchmarkError::Weightless)?;
        let creator = T::CreateTokenOrigin::ensure_origin(creator_origin.clone())
            .map_err(|_| BenchmarkError::Weightless)?;
        let asset_id = launch_token::<T>(creator_origin, TokenClass::CappedMintable)?;

        #[extrinsic_call]
        mint(
            RawOrigin::Signed(creator),
            asset_id,
            DomainId::X3Native,
            MINT_AMOUNT,
        );

        let ledger = T::Ledger::ledger(&asset_id).ok_or(BenchmarkError::Weightless)?;
        assert_eq!(ledger.canonical_supply, INITIAL_SUPPLY + MINT_AMOUNT);
        Ok(())
    }

    #[benchmark]
    fn burn() -> Result<(), BenchmarkError> {
        let creator_origin = T::CreateTokenOrigin::try_successful_origin()
            .map_err(|_| BenchmarkError::Weightless)?;
        let creator = T::CreateTokenOrigin::ensure_origin(creator_origin.clone())
            .map_err(|_| BenchmarkError::Weightless)?;
        // Only `Burnable` permits a burn and only `CappedMintable`/`GovernanceMintable` permit a
        // post-launch mint, so the two benchmarks launch different classes.
        let asset_id = launch_token::<T>(creator_origin, TokenClass::Burnable)?;

        #[extrinsic_call]
        burn(
            RawOrigin::Signed(creator),
            asset_id,
            DomainId::X3Native,
            BURN_AMOUNT,
        );

        let ledger = T::Ledger::ledger(&asset_id).ok_or(BenchmarkError::Weightless)?;
        assert_eq!(ledger.canonical_supply, INITIAL_SUPPLY - BURN_AMOUNT);
        Ok(())
    }

    #[benchmark]
    fn transfer_mint_authority() -> Result<(), BenchmarkError> {
        let creator_origin = T::CreateTokenOrigin::try_successful_origin()
            .map_err(|_| BenchmarkError::Weightless)?;
        let creator = T::CreateTokenOrigin::ensure_origin(creator_origin.clone())
            .map_err(|_| BenchmarkError::Weightless)?;
        let asset_id = launch_token::<T>(creator_origin, TokenClass::CappedMintable)?;
        let new_authority: T::AccountId = frame_benchmarking::account("new_authority", 0, 0);

        #[extrinsic_call]
        transfer_mint_authority(RawOrigin::Signed(creator), asset_id, new_authority.clone());

        let record = Tokens::<T>::get(asset_id).ok_or(BenchmarkError::Weightless)?;
        assert_eq!(record.mint_authority, new_authority);
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
}
