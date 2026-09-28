//! Benchmarking setup for `pallet-x3-domain-registry`.
//!
//! The three dispatchables charged literals — 20,000 picoseconds to register a domain, 30,000 to set
//! its records — behind a `runtime-benchmarks` feature with nothing behind it, so none had been
//! measured.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::{traits::EnsureOrigin, BoundedVec};
use frame_system::RawOrigin;
use sp_std::{vec, vec::Vec};

#[benchmarks]
mod benchmarks {
    use super::*;

    /// A domain the pallet's own validator accepts: lower-case, labelled, and under `.x3`.
    const DOMAIN: &[u8] = b"bench.x3";

    fn records<T: Config>() -> Result<Vec<X3DnsRecord<T>>, BenchmarkError> {
        let txt: BoundedVec<u8, T::MaxTxtLen> =
            BoundedVec::try_from(b"benchmark".to_vec()).map_err(|_| BenchmarkError::Weightless)?;
        Ok(vec![X3DnsRecord {
            ttl: 60,
            data: X3RecordData::Txt(txt),
        }])
    }

    #[benchmark]
    fn register_domain() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        register_domain(RawOrigin::Signed(caller), DOMAIN.to_vec());

        assert_eq!(DomainList::<T>::get().len(), 1);
        Ok(())
    }

    #[benchmark]
    fn set_records() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        // The owner has to have registered the domain first; the setup is the pallet's own call.
        Pallet::<T>::register_domain(RawOrigin::Signed(caller.clone()).into(), DOMAIN.to_vec())?;
        let records = records::<T>()?;

        #[extrinsic_call]
        set_records(RawOrigin::Signed(caller), DOMAIN.to_vec(), records);

        assert_eq!(DomainList::<T>::get().len(), 1);
        Ok(())
    }

    #[benchmark]
    fn set_records_as_governance() -> Result<(), BenchmarkError> {
        let owner: T::AccountId = whitelisted_caller();
        let origin =
            T::UpdateOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        let records = records::<T>()?;

        #[extrinsic_call]
        set_records_as_governance(origin, DOMAIN.to_vec(), owner, records);

        assert_eq!(DomainList::<T>::get().len(), 1);
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
