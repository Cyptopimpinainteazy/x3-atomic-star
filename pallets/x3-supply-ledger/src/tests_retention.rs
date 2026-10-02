// SPDX-License-Identifier: Apache-2.0
//
// tests_retention.rs — the historical supply-proof window stays bounded.
//
// `on_finalize` is the only writer of `HistoricalProofs`, and the pruning guard is the only thing
// keeping that storage bounded. Nothing exercised the guard, so a mutation of
// `current_block > HISTORICAL_PROOF_RETENTION_BLOCKS` to `==` survived the whole suite: with that
// change the oldest proofs are never evicted after the boundary block. These tests pin the
// boundary from below and above.

use crate::mock::{asset, new_test_ext, register_asset, Test};
use crate::{HistoricalProofs, Pallet, HISTORICAL_PROOF_RETENTION_BLOCKS};
use frame_support::traits::Hooks;

/// Finalize a block the way the runtime would.
fn finalize(block: u32) {
    <Pallet<Test> as Hooks<u64>>::on_finalize(u64::from(block));
}

/// The window keeps at most `HISTORICAL_PROOF_RETENTION_BLOCKS` blocks of history: nothing is
/// pruned while the oldest proof is still inside it, and every later block evicts exactly the one
/// that has just fallen out.
#[test]
fn the_proof_window_is_pruned_to_the_retention_boundary() {
    new_test_ext().execute_with(|| {
        register_asset(asset(1), 1_000_000, 1_000_000);

        let boundary = HISTORICAL_PROOF_RETENTION_BLOCKS;

        // Up to and including the boundary, the oldest proof is still inside the window.
        for block in 1..=boundary {
            finalize(block);
        }
        assert!(
            HistoricalProofs::<Test>::contains_key(1),
            "block 1 must survive while the window still covers it"
        );
        assert!(HistoricalProofs::<Test>::contains_key(boundary));

        // The next block is the first one that evicts the proof `RETENTION` blocks behind it.
        finalize(boundary + 1);
        assert!(
            !HistoricalProofs::<Test>::contains_key(1),
            "block 1 must fall out of the window when block RETENTION + 1 finalizes"
        );
        assert!(HistoricalProofs::<Test>::contains_key(2));

        finalize(boundary + 2);
        assert!(!HistoricalProofs::<Test>::contains_key(2));
        assert!(HistoricalProofs::<Test>::contains_key(3));
    });
}
