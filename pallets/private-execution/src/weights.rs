//! Weight info for Private Execution pallet.
//!
//! These weights account for TEE attestation verification (CPU-heavy),
//! ZK-proof validation, and threshold cryptography overhead.
//! Re-run benchmarks on TEE-equipped hardware before mainnet.

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions for the private execution pallet.
pub trait WeightInfo {
    fn register_confidential_validator() -> Weight;
    fn deregister_confidential_validator() -> Weight;
    fn refresh_attestation() -> Weight;
    fn submit_private_transaction() -> Weight;
    fn commit_encrypted_state_diff() -> Weight;
    fn set_committee_key() -> Weight;
    fn set_enabled() -> Weight;
    fn open_ordering_window() -> Weight;
    fn commit_ordering() -> Weight;
    fn reveal_ordering() -> Weight;
    /// Settling replays the whole window through the ordering lane, so its cost
    /// is a function of the window capacity rather than a constant.
    fn settle_ordering_window(commitments: u32) -> Weight;
    fn install_ordering_beacon() -> Weight;
}

/// Production weights using runtime-configurable DB costs.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: crate::Config> WeightInfo for SubstrateWeight<T> {
    /// Includes TEE attestation parse + signature verify (~50M extra).
    /// Storage: `PrivateExecution::Validators` (r:1 w:1), `Balances::Reserves` (r:1 w:1).
    fn register_confidential_validator() -> Weight {
        Weight::from_parts(112_000_000, 2_048)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }
    fn deregister_confidential_validator() -> Weight {
        Weight::from_parts(42_000_000, 1_024)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }
    /// Includes TEE attestation re-verification (~50M extra).
    fn refresh_attestation() -> Weight {
        Weight::from_parts(102_000_000, 2_048)
            .saturating_add(T::DbWeight::get().reads(1_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    /// Includes ZK-SNARK proof verify (~80M extra) + threshold signature check.
    /// Storage: `PrivateExecution::PendingTxs` (r:1 w:1), `PrivateExecution::Validators` (r:2 w:0).
    fn submit_private_transaction() -> Weight {
        Weight::from_parts(162_000_000, 2_048)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }
    /// Threshold signature verification over encrypted state diff.
    fn commit_encrypted_state_diff() -> Weight {
        Weight::from_parts(182_000_000, 3_072)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }
    fn set_committee_key() -> Weight {
        Weight::from_parts(22_000_000, 512)
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn set_enabled() -> Weight {
        Weight::from_parts(12_000_000, 128)
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    /// Storage: `OrderingWindows` (r:0 w:1), `NextOrderingWindowId` (r:1 w:1).
    fn open_ordering_window() -> Weight {
        Weight::from_parts(14_000_000, 256)
            .saturating_add(T::DbWeight::get().reads_writes(1, 2))
    }
    /// Storage: `OrderingWindows` (r:1), `OrderingCommitBySender` (r:1 w:1),
    /// `OrderingCommits` (r:1 w:1), `Balances::Reserves` (r:1 w:1).
    fn commit_ordering() -> Weight {
        Weight::from_parts(24_000_000, 512)
            .saturating_add(T::DbWeight::get().reads_writes(4, 3))
    }
    /// Storage: `OrderingWindows` (r:1), `OrderingCommits` (r:1),
    /// `OrderingReveals` (r:1 w:1), `Balances::Reserves` (r:1 w:1).
    fn reveal_ordering() -> Weight {
        Weight::from_parts(24_000_000, 512)
            .saturating_add(T::DbWeight::get().reads_writes(4, 2))
    }
    /// Two reads and one write per possible commitment in the window, plus the
    /// configured plaintext budget, plus the fixed cost of building and settling
    /// the lane. Settling reads every commitment and every reveal, so both the
    /// capacity and the window's byte ceiling set the size of the proof.
    fn settle_ordering_window(commitments: u32) -> Weight {
        let per_commitment = T::DbWeight::get()
            .reads(2_u64)
            .saturating_add(T::DbWeight::get().writes(1_u64));
        let proof_size = (T::MaxOrderingWindowBytes::get() as u64)
            .saturating_add(4_096)
            .saturating_add((commitments as u64).saturating_mul(256));
        Weight::from_parts(60_000_000, proof_size)
            .saturating_add(T::DbWeight::get().reads_writes(2, 3))
            .saturating_add(per_commitment.saturating_mul(commitments as u64))
    }
    /// Storage: `OrderingWindows` (r:1 w:1).
    fn install_ordering_beacon() -> Weight {
        Weight::from_parts(14_000_000, 256)
            .saturating_add(T::DbWeight::get().reads_writes(1, 1))
    }
}

impl WeightInfo for () {
    fn register_confidential_validator() -> Weight {
        Weight::from_parts(112_000_000, 2_048).saturating_add(RocksDbWeight::get().reads_writes(2, 2))
    }
    fn deregister_confidential_validator() -> Weight {
        Weight::from_parts(42_000_000, 1_024).saturating_add(RocksDbWeight::get().reads_writes(2, 2))
    }
    fn refresh_attestation() -> Weight {
        Weight::from_parts(102_000_000, 2_048).saturating_add(RocksDbWeight::get().reads_writes(1, 1))
    }
    fn submit_private_transaction() -> Weight {
        Weight::from_parts(162_000_000, 2_048).saturating_add(RocksDbWeight::get().reads_writes(3, 2))
    }
    fn commit_encrypted_state_diff() -> Weight {
        Weight::from_parts(182_000_000, 3_072).saturating_add(RocksDbWeight::get().reads_writes(2, 2))
    }
    fn set_committee_key() -> Weight {
        Weight::from_parts(22_000_000, 512).saturating_add(RocksDbWeight::get().writes(1))
    }
    fn set_enabled() -> Weight {
        Weight::from_parts(12_000_000, 128).saturating_add(RocksDbWeight::get().writes(1))
    }
    fn open_ordering_window() -> Weight {
        Weight::from_parts(14_000_000, 256).saturating_add(RocksDbWeight::get().reads_writes(1, 2))
    }
    fn commit_ordering() -> Weight {
        Weight::from_parts(24_000_000, 512).saturating_add(RocksDbWeight::get().reads_writes(4, 3))
    }
    fn reveal_ordering() -> Weight {
        Weight::from_parts(24_000_000, 512).saturating_add(RocksDbWeight::get().reads_writes(4, 2))
    }
    fn settle_ordering_window(commitments: u32) -> Weight {
        let per_commitment = RocksDbWeight::get()
            .reads(2_u64)
            .saturating_add(RocksDbWeight::get().writes(1_u64));
        let proof_size = (crate::MAX_ORDERING_WINDOW_PLAINTEXT_BYTES as u64)
            .saturating_add(4_096)
            .saturating_add((commitments as u64).saturating_mul(256));
        Weight::from_parts(60_000_000, proof_size)
            .saturating_add(RocksDbWeight::get().reads_writes(2, 3))
            .saturating_add(per_commitment.saturating_mul(commitments as u64))
    }
    fn install_ordering_beacon() -> Weight {
        Weight::from_parts(14_000_000, 256).saturating_add(RocksDbWeight::get().reads_writes(1, 1))
    }
}
