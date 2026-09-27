// SPDX-License-Identifier: Apache-2.0
//! Weight interface for pallet-northern-swarm.
//!
//! These weights are DB-aware conservative bounds so runtime code never falls
//! back to inline placeholder weights. The FRAME benchmark workflow owns the
//! measurement/regeneration step for production tuning.

use frame_support::{
    traits::Get,
    weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
    fn register_executor() -> Weight;
    fn deregister_executor() -> Weight;
    fn release_stake() -> Weight;
    fn submit_heartbeat() -> Weight;
    fn submit_task() -> Weight;
    fn claim_task() -> Weight;
    fn submit_result() -> Weight;
    fn slash_executor() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn register_executor() -> Weight {
        Weight::from_parts(45_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    fn deregister_executor() -> Weight {
        Weight::from_parts(30_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    fn release_stake() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    fn submit_heartbeat() -> Weight {
        Weight::from_parts(20_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    fn submit_task() -> Weight {
        Weight::from_parts(50_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    fn claim_task() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(5))
            .saturating_add(T::DbWeight::get().writes(4))
    }

    fn submit_result() -> Weight {
        // Temporary upper bound for the configured 3-executor path. This file
        // must be replaced by benchmark CLI output before the release gate can pass.
        Weight::from_parts(101_000_000, 7_536)
            .saturating_add(T::DbWeight::get().reads(12))
            .saturating_add(T::DbWeight::get().writes(14))
    }

    fn slash_executor() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(3))
    }
}

impl WeightInfo for () {
    fn register_executor() -> Weight {
        Weight::from_parts(45_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn deregister_executor() -> Weight {
        Weight::from_parts(30_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(1))
    }

    fn release_stake() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn submit_heartbeat() -> Weight {
        Weight::from_parts(20_000_000, 2_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }

    fn submit_task() -> Weight {
        Weight::from_parts(50_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn claim_task() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(5))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn submit_result() -> Weight {
        Weight::from_parts(101_000_000, 7_536)
            .saturating_add(RocksDbWeight::get().reads(12))
            .saturating_add(RocksDbWeight::get().writes(14))
    }

    fn slash_executor() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
}
