//! Weight definitions for pallet-x3-account-registry.
//!
//! These values are placeholders for RC1 and should be replaced by
//! benchmark-generated values during production hardening.

use frame_support::weights::{constants::RocksDbWeight, Weight};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
    fn register_account() -> Weight;
    fn deregister_account() -> Weight;
    fn anchor_nonce() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn register_account() -> Weight {
        Weight::from_parts(10_000, 0)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn deregister_account() -> Weight {
        Weight::from_parts(10_000, 0)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn anchor_nonce() -> Weight {
        Weight::from_parts(10_000, 0).saturating_add(RocksDbWeight::get().reads(2))
    }
}

impl WeightInfo for () {
    fn register_account() -> Weight {
        Weight::zero()
    }
    fn deregister_account() -> Weight {
        Weight::zero()
    }
    fn anchor_nonce() -> Weight {
        Weight::zero()
    }
}
