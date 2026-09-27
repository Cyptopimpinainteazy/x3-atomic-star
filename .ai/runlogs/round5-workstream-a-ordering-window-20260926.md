# Round 5 / Workstream A — the ordering window reaches a chain path

Date: 2026-09-26. Agent: workstream A of `.ai/tasks/2026-09-26-round5-workstreams.md`.
Tree state at start: `b51589972 refactor(order-window): the ordering lane moves to a no_std crate so a runtime can enforce it`.

## What changed

`crates/x3-order-window` was reachable only from `x3-swap-router`'s tests. `pallets/private-execution`
— the pallet `X3-MEV-006` points at — now runs the lane on chain: `open_ordering_window`,
`commit_ordering`, `reveal_ordering`, `install_ordering_beacon`, `settle_ordering_window`, over
storage the chain persists, replayed through the same `CommitRevealLane` the tests exercise.

## Commands and results

```
cargo test -p pallet-private-execution
    test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
    (12 pre-existing + 22 new ordering-window tests; the new ones are listed below)

cargo clippy -p pallet-private-execution --all-targets -- -D warnings
    Finished `dev` profile ... in 14.53s      (no warnings, no errors)

cargo fmt --all -- --check
    0 diffs

cargo check -p pallet-private-execution
    Finished `dev` profile ... in 10.24s

cargo check -p pallet-private-execution --no-default-features
    Finished `dev` profile ... in 13.56s      (the lane's no_std build survives the pallet)
```

`cargo check -p x3-chain-runtime --features std` could not be used as a gate: it fails in
`pallets/x3-wallet-pallet` on four `E0061`/`E0600` errors against `crates/x3-wallet/src/social_recovery.rs`.
That crate is not touched by this workstream and was being edited concurrently (`git status` shows it
dirty under another agent), so the runtime's `pallet_private_execution::Config` addition is wired but
not compiled by this workstream. It is two associated types on an existing impl.

## Break-it-first control

Temporary patch to `settle_ordering_window`: replace `settlement.canonical_order()` with a sort of
`(revealed_at_block, commit_hash)` taken from `OrderingReveals` storage — i.e. arrival order.

```
running 1 test
test tests::a_settled_window_is_the_canonical_key_order_and_recomputes_from_storage ... FAILED

thread '...' panicked at pallets/private-execution/src/tests.rs:897:9:
assertion `left == right` failed
  left: [0x8ae814bb..., 0x58c6d438..., 0x3843ce00...]   <- arrival order (descending hash)
 right: [0x3843ce00..., 0x58c6d438..., 0x8ae814bb...]   <- canonical key order (ascending hash)

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 32 filtered out
```

The control produces the exact reverse of the canonical sequence and the test catches it. The patch was
reverted; the reverted tree is the one committed.

An earlier version of the control did **not** bite, and that is worth recording: the test committed and
revealed all three participants in the same block, so "arrival order" and "hash order" coincided and the
arrival-ordered implemention passed. The test now places each commit/reveal in its own block
(`System::set_block_number(OPEN + index)`), which is what makes it able to tell the two orders apart.

## Test transcript (the ordering-window tests; the 12 pre-existing tests passed in the same run)

```
test tests::an_unrevealed_commitment_is_excluded_and_its_bond_forfeited ... ok
test tests::a_beacon_cannot_be_installed_while_the_window_is_open_and_only_once ... ok
test tests::a_bond_below_the_minimum_is_refused ... ok
test tests::a_commit_outside_the_window_is_refused ... ok
test tests::a_reveal_outside_the_window_is_refused ... ok
test tests::a_reveal_that_does_not_hash_to_its_commit_is_refused ... ok
test tests::a_reveal_without_a_commit_is_refused ... ok
test tests::a_second_reveal_of_the_same_commitment_is_refused ... ok
test tests::a_settled_window_is_the_canonical_key_order_and_recomputes_from_storage ... ok
test tests::a_settled_window_uses_the_installed_beacon ... ok
test tests::a_window_beyond_its_capacity_is_refused ... ok
test tests::a_window_beyond_its_total_byte_budget_is_refused ... ok
test tests::an_inverted_or_already_closed_window_is_refused ... ok
test tests::an_oversized_reveal_is_refused ... ok
test tests::disabling_private_execution_does_not_trap_committed_bonds ... ok
test tests::one_sender_commits_once_per_window ... ok
test tests::only_the_committer_may_reveal ... ok
test tests::opening_a_window_fixes_its_bond_and_range ... ok
test tests::opening_and_committing_need_the_same_guards_as_private_submission ... ok
test tests::settling_an_open_window_is_refused_and_settling_twice_is_refused ... ok
test tests::the_same_commit_hash_is_used_once_per_window ... ok
test tests::unknown_windows_and_settled_windows_refuse_further_work ... ok
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Two decisions worth flagging

1. **Reveal and settle are not gated on `Enabled`/quorum; open and commit are.** The workstream asked for
   the same guards private submission has on window operations. Applying them to reveal and settle would
   mean that an admin toggling `set_enabled(false)`, or the confidential set dropping below quorum,
   freezes bonds the chain already took — the disable switch becomes a fund trap. Opening a window and
   committing to one carry both guards; closing one out does not. There is a test for both halves
   (`opening_and_committing_need_the_same_guards_as_private_submission`,
   `disabling_private_execution_does_not_trap_committed_bonds`).
2. **A window has a total plaintext budget (`MaxOrderingWindowBytes`), not just a per-reveal one.**
   Settling reads every reveal in one transaction, so a window grown past what fits in a block would be
   unsettlable and every bond in it unresolvable. The budget is what keeps "settle is always possible"
   a property of the pallet rather than of the participants' restraint, and `settle_ordering_window`'s
   weight proof size is sized from the configured value.

## Row delta

```
X3-MEV-006 (private-execution / MEV-resistant architecture)
  implemented=yes  tested=yes  mainnet_ready=no
  source=runtime-reachable pallet extrinsics  paths=[pallets/private-execution/src/{lib,types,weights,mock,tests}.rs, runtime/src/lib.rs]
  required_tests=[opening_a_window_fixes_its_bond_and_range,
    a_settled_window_is_the_canonical_key_order_and_recomputes_from_storage,
    a_settled_window_uses_the_installed_beacon, an_unrevealed_commitment_is_excluded_and_its_bond_forfeited,
    a_window_beyond_its_total_byte_budget_is_refused, a_window_beyond_its_capacity_is_refused,
    one_sender_commits_once_per_window, the_same_commit_hash_is_used_once_per_window,
    only_the_committer_may_reveal, a_reveal_that_does_not_hash_to_its_commit_is_refused,
    a_second_reveal_of_the_same_commitment_is_refused, a_reveal_outside_the_window_is_refused,
    a_commit_outside_the_window_is_refused, a_bond_below_the_minimum_is_refused,
    settling_an_open_window_is_refused_and_settling_twice_is_refused,
    a_beacon_cannot_be_installed_while_the_window_is_open_and_only_once,
    a_reveal_without_a_commit_is_refused, an_oversized_reveal_is_refused,
    unknown_windows_and_settled_windows_refuse_further_work,
    an_inverted_or_already_closed_window_is_refused,
    opening_and_committing_need_the_same_guards_as_private_submission,
    disabling_private_execution_does_not_trap_committed_bonds]
  test_evidence=.ai/runlogs/round5-workstream-a-ordering-window-20260926.md
  blockers=[no runtime-compile gate (blocked by pallet-x3-wallet's in-flight breakage);
            weights are hand-sized, not benchmarked]

X3-MEV-008 (fair ordering)
  implemented=yes  tested=yes  mainnet_ready=no
  Note: the lane's canonical-order property is now enforced by a chain path, not only by
  crates/x3-order-window's own tests. The beacon slot (`install_ordering_beacon`, admin-gated,
  refused while the window is open) is the on-chain half of the grinding-resistance the lane's
  docs describe as an off-chain assumption; no beacon source is wired in this workstream.
```

## Remaining blockers (honest)

* The runtime's `Config` impl addition is uncompiled: `cargo check -p x3-chain-runtime` dies earlier in
  `pallet-x3-wallet` (four errors, another agent's in-flight edit). Re-run the runtime gate once that
  crate compiles.
* Weights for the five new calls are hand-sized from the storage accesses each makes, the way the rest of
  this pallet's weights are. They are not benchmarked; the file already says to re-benchmark before
  mainnet and that now applies to these rows too.
* No beacon source. The extrinsic exists and the lane keys the order from it, but nothing supplies a
  value; without one the placement hole the lane documents stays open.
* No live-node run: this is a unit-test gate only, per the round rules (no multi-node or fixed-port runs).

## Next 5 tasks

1. Re-run `cargo check -p x3-chain-runtime --features std` once `pallet-x3-wallet` compiles; confirm the
   private-execution `Config` addition type-checks in the runtime.
2. Add a `try-runtime`/`on_runtime_upgrade` check for `OrderingWindows`/`OrderingCommits` consistency.
3. Source the beacon: wire a real value (parent-block hash beacon or a VRF pallet output) into
   `install_ordering_beacon` behind an admin call, and test that a participant cannot predict it.
4. Benchmark the five new extrinsics and replace `SubstrateWeight<T>` constants.
5. Give the lane an ordered-consumer: the swap router should read `OrderingSettlements` instead of
   calling the lane directly, so the on-chain window and the router's execution share one order.
