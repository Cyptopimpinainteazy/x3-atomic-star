//! Weight definitions for pallet-x3-wallet.
//!
//! These are conservative placeholders for RC1 and should be replaced by
//! benchmark-generated weights in production hardening.

use frame_support::weights::{constants::RocksDbWeight, Weight};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
    fn register_hardware_wallet() -> Weight;
    fn create_multisig_wallet() -> Weight;
    fn transfer_tokens() -> Weight;
    fn register_biometric() -> Weight;
    fn initiate_recovery() -> Weight;
    fn mint_tokens() -> Weight;
    fn add_minter() -> Weight;
    fn remove_minter() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn register_hardware_wallet() -> Weight {
        Weight::from_parts(10_000, 0)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }

    fn create_multisig_wallet() -> Weight {
        Weight::from_parts(15_000, 0)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }

    fn transfer_tokens() -> Weight {
        Weight::from_parts(10_000, 0)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn register_biometric() -> Weight {
        Weight::from_parts(8_000, 0)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }

    fn initiate_recovery() -> Weight {
        Weight::from_parts(12_000, 0).saturating_add(RocksDbWeight::get().reads(1))
    }

    fn mint_tokens() -> Weight {
        Weight::from_parts(5_000, 0)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(1))
    }

    fn add_minter() -> Weight {
        Weight::from_parts(5_000, 0).saturating_add(RocksDbWeight::get().writes(1))
    }

    fn remove_minter() -> Weight {
        Weight::from_parts(5_000, 0).saturating_add(RocksDbWeight::get().writes(1))
    }
}

impl WeightInfo for () {
    fn register_hardware_wallet() -> Weight {
        Weight::zero()
    }
    fn create_multisig_wallet() -> Weight {
        Weight::zero()
    }
    fn transfer_tokens() -> Weight {
        Weight::zero()
    }
    fn register_biometric() -> Weight {
        Weight::zero()
    }
    fn initiate_recovery() -> Weight {
        Weight::zero()
    }
    fn mint_tokens() -> Weight {
        Weight::zero()
    }
    fn add_minter() -> Weight {
        Weight::zero()
    }
    fn remove_minter() -> Weight {
        Weight::zero()
    }
}
