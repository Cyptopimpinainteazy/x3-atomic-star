//! Benchmarking setup for `pallet-x3-flashloan`.
//!
//! All three dispatchables charged literals — 10,000 picoseconds to borrow or repay, 5,000 to add
//! liquidity — behind a `runtime-benchmarks` feature that had nothing in it, so none had been
//! measured. Each of them moves real currency: `borrow` and `repay` transfer between the pool
//! account and the borrower, and `add_liquidity` transfers into the pool account, so the setup funds
//! the accounts involved the way a chain's balances would already be funded.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use sp_runtime::traits::Saturating;

#[benchmarks]
mod benchmarks {
    use super::*;

    /// Amounts are multiples of the currency's existential deposit.
    ///
    /// A fixed literal does not work here: the mock's deposit is tiny while the runtime's is
    /// `100 * MICRO_ATLAS`, and `pallet_balances` refuses a `KeepAlive` transfer that would leave an
    /// account below it — a 1,000-unit pool failed on the dev chain with
    /// "Account cannot exist with the funds that would be given" even though the same benchmark
    /// passed in the mock.
    fn scaled<T: Config>(multiple: u32) -> <T::Currency as Currency<T::AccountId>>::Balance {
        T::Currency::minimum_balance().saturating_mul(multiple.into())
    }

    fn fund<T: Config>(who: &T::AccountId, multiple: u32) {
        let _ = T::Currency::make_free_balance_be(who, scaled::<T>(multiple));
    }

    #[benchmark]
    fn add_liquidity() -> Result<(), BenchmarkError> {
        let provider: T::AccountId = whitelisted_caller();
        fund::<T>(&provider, 1_000_000);
        let amount = scaled::<T>(100);
        // The mock's genesis already seeds the pool, so the assertion is about the increase.
        let before = PoolBalance::<T>::get();

        #[extrinsic_call]
        add_liquidity(RawOrigin::Signed(provider), amount);

        assert_eq!(PoolBalance::<T>::get(), before.saturating_add(amount));
        Ok(())
    }

    #[benchmark]
    fn borrow() -> Result<(), BenchmarkError> {
        let provider: T::AccountId = whitelisted_caller();
        let borrower: T::AccountId = account("borrower", 0, 0);
        fund::<T>(&provider, 1_000_000);
        // The pool account has to hold the currency, not just `PoolBalance`: `borrow` transfers from
        // it, so the liquidity goes in through the pallet's own extrinsic.
        Pallet::<T>::add_liquidity(RawOrigin::Signed(provider).into(), scaled::<T>(100_000))?;

        #[extrinsic_call]
        borrow(RawOrigin::Signed(borrower), scaled::<T>(100));

        assert!(ActiveLoan::<T>::get().is_some());
        Ok(())
    }

    #[benchmark]
    fn repay() -> Result<(), BenchmarkError> {
        let provider: T::AccountId = whitelisted_caller();
        let borrower: T::AccountId = account("borrower", 0, 0);
        fund::<T>(&provider, 1_000_000);
        fund::<T>(&borrower, 1_000_000);
        Pallet::<T>::add_liquidity(RawOrigin::Signed(provider).into(), scaled::<T>(100_000))?;
        Pallet::<T>::borrow(RawOrigin::Signed(borrower.clone()).into(), scaled::<T>(100))?;
        let loan = ActiveLoan::<T>::get().ok_or(BenchmarkError::Weightless)?;
        let required = loan.amount.saturating_add(loan.fee);

        #[extrinsic_call]
        repay(RawOrigin::Signed(borrower), required);

        assert!(ActiveLoan::<T>::get().is_none());
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
