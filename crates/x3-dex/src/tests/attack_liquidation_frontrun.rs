//! Two liquidations in one batch: the batch's declared order, and what happens when a caller
//! hands the router the same swaps in a different order.
//!
//! This file used to contain a single test named
//! `liquidation_frontrun_eliminated_by_fair_ordering` whose whole assertion was
//! `assert!(50u64 > 0u64)` on two hardcoded locals. It built a batch, executed it, and then
//! compared two literals — so it would have passed with the router ordering by arrival, by
//! reverse arrival, or by nothing at all, which is what it did (measured 2026-09-26:
//! `execute_batch_swap` summed the caller's outputs in vector order and never read
//! `SwapInstruction::sequence`). The test's *name* was the claim, and the name was unearned.
//!
//! What this router can honestly enforce is narrower, and that is what is asserted below: the
//! batch is executed in the order it declares, and a batch whose vector disagrees with its own
//! `sequence` fields is refused before any output is accepted. Fair *ordering* — the thing the old
//! name claimed — is a separate, newer mechanism in
//! `crates/x3-swap-router/src/mev_protection/fair_ordering.rs` (commit-reveal window, canonical
//! key order), and it is not wired into this router.

use crate::batch_swap_router::SwapInstruction;
use crate::BatchSwapRouter;

fn liquidation(sequence: u32) -> SwapInstruction {
    SwapInstruction {
        pool_id: [1u8; 32],
        token_in: 10,
        token_out: 20,
        amount_in: 1_000,
        min_amount_out: 900,
        sequence,
    }
}

#[test]
fn a_batch_in_the_order_it_declares_executes() {
    let mut batch =
        BatchSwapRouter::create_batch_swap([7u8; 32], vec![liquidation(0), liquidation(1)], 1_000)
            .expect("a batch in sequence order must be created");

    let executed_total = BatchSwapRouter::execute_batch_swap(&mut batch, vec![950, 900])
        .expect("batch execution must succeed");

    assert_eq!(executed_total, 1_850);
    assert_eq!(batch.status, 1);
}

#[test]
fn a_front_runner_cannot_reorder_the_batch_by_handing_it_over_differently() {
    // The same two swaps, with the front-runner's (sequence 1) placed first and the victim's
    // (sequence 0) second. This is the shape the old test's name was about: if the router executed
    // the vector, the front-runner would take the victim's place in the batch.
    let front_runner = liquidation(1);
    let victim = liquidation(0);

    assert_eq!(
        BatchSwapRouter::create_batch_swap([7u8; 32], vec![front_runner, victim], 1_000),
        Err("Batch swaps are not in sequence order"),
        "a batch whose vector disagrees with its declared sequence must be refused"
    );
}

#[test]
fn the_refusal_happens_before_any_output_is_accepted() {
    // A hand-reordered batch is refused even when every output would satisfy its minimum, so a
    // caller cannot buy execution by paying everyone enough. `BatchSwap`'s fields are public and
    // the type is `Decode`, so `execute_batch_swap` has to make this check itself.
    let mut out_of_order = crate::batch_swap_router::BatchSwap {
        batch_id: [3u8; 32],
        initiator: [7u8; 32],
        swaps: vec![liquidation(1), liquidation(0)],
        total_input: 2_000,
        total_output: 0,
        status: 0,
        created_block: 1_000,
    };

    assert_eq!(
        BatchSwapRouter::execute_batch_swap(&mut out_of_order, vec![1_000, 1_000]),
        Err("Batch swaps are not in sequence order"),
        "a batch carrying one order while declaring another must be refused"
    );
    assert_eq!(
        out_of_order.status, 0,
        "a refused batch must not be marked executed"
    );
    assert_eq!(
        out_of_order.total_output, 0,
        "a refused batch must not book an output"
    );
}
