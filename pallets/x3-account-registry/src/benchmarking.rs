//! Benchmarking setup for `pallet-x3-account-registry`.
//!
//! All three dispatchables charged the same literal (10,000) and the pallet had
//! no test runtime, so nothing had ever measured them. `deregister_account` and
//! `anchor_nonce` both require a registered caller, and each establishes that
//! through the pallet's own `register_account` rather than by writing
//! `AccountRegistry` directly, so the measured state is the state a real chain
//! reaches.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use sp_std::vec;

#[benchmarks]
mod benchmarks {
    use super::*;

    /// The name every benchmark registers. Well inside `MaxNameLength = 64`.
    const NAME: usize = 16;

    /// Register `caller` through the pallet's own extrinsic.
    ///
    /// Generic over `T` because a plain helper inside `#[benchmarks]` is not
    /// expanded by the macro and so has no injected `T` of its own.
    fn register<T: Config>(caller: &T::AccountId) -> Result<(), BenchmarkError> {
        Pallet::<T>::register_account(
            RawOrigin::Signed(caller.clone()).into(),
            T::AtlasId::default(),
            AccountKind::Eoa,
            vec![b'a'; NAME],
        )
        .map_err(|_| BenchmarkError::Weightless)
    }

    #[benchmark]
    fn register_account() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        register_account(
            RawOrigin::Signed(caller.clone()),
            T::AtlasId::default(),
            AccountKind::Eoa,
            vec![b'a'; NAME],
        );

        assert!(AccountRegistry::<T>::contains_key(&caller));
        Ok(())
    }

    #[benchmark]
    fn deregister_account() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        register::<T>(&caller)?;

        #[extrinsic_call]
        deregister_account(RawOrigin::Signed(caller.clone()));

        assert!(!AccountRegistry::<T>::contains_key(&caller));
        Ok(())
    }

    #[benchmark]
    fn anchor_nonce() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        register::<T>(&caller)?;

        #[extrinsic_call]
        anchor_nonce(RawOrigin::Signed(caller.clone()));

        assert!(AccountRegistry::<T>::contains_key(&caller));
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
}
