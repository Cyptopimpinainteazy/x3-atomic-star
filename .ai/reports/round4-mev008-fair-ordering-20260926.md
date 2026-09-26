# Round 4 / Workstream B — `X3-MEV-008` fair transaction ordering

Agent: `/root/round4_mev002`. Date: 2026-09-26. Base commit when the work started:
`a5a87895a`; HEAD moved to `c85dba7f1` (another agent's Workstream C commit) while this
pass ran. Nothing in this pass touches `x3-lang/**`, so that commit and this change are
disjoint.

## The question this row was blocked on

Blocker on record: *"No production fair-ordering protocol identified."* Row on master:
`implemented = 15 / tested = 5 / mainnet_ready = 5`, `source = "research"`, `paths = []`.

## What was actually on master before this pass

1. **`crates/x3-swap-router/src/mev_protection/mod.rs` is advisory, not ordering.** It
   never decides a sequence. Inside it:
   * `calculate_random_delay(min, max)` returns `(min + max) / 2` — the midpoint, not a
     random value, and the retry is idempotent.
   * `success_probability` is a chain of hardcoded constants (`0.95`, `0.98`, `0.92`,
     `0.99`, `0.97`) — a claim about the future dressed as a measurement.
   * `detect_sandwich_attacks` fills `victim_tx_hash` / `front_run_tx_hash` /
     `back_run_tx_hash` with `H256::zero()` and the comment "Would be actual transaction
     hash".
   * `update_protection_metrics` is a no-op whose body is `let _ = (overhead_ms,
     success_probability);` with the comment "In a real implementation, this would update
     persistent metrics".
   `rg -n "MEVProtector|MEVProtectionConfig|mev_protection::"` finds **no consumer outside
   `crates/x3-swap-router` itself**; `lib.rs` re-exports it and `routing.rs` imports only
   the `Hop`/`Route` types.

2. **`crates/private-mempool/src/queue.rs` is insertion-ordered.** Its own doc says
   "Ordered list of transactions (insertion order)". First-come-first-served is not fair
   ordering, and the round-4 brief says to say so rather than dress it up. (That file
   belongs to Workstream A and was not touched here.)

3. **`crates/x3-dex/src/tests/attack_liquidation_frontrun.rs` contains a false green.**
   `liquidation_frontrun_eliminated_by_fair_ordering` executes a batch and then asserts

   ```rust
   let first_bonus = 50u64;
   let second_bonus = 0u64;
   assert!(first_bonus > second_bonus);
   ```

   Both figures are locals defined in the test. The comment calls it "Fair-ordering
   invariant"; it asserts `50 > 0`. It proves nothing about the router's ordering and
   would keep passing if `execute_batch_swap` ordered by anything at all. Out of this
   workstream's ownership; reported, not edited.

4. **The whole crate is unreachable.** `cargo tree -i x3-swap-router --workspace` prints
   only `x3-swap-router v0.1.0` itself: no node, runtime, pallet or other crate depends on
   it. So even a perfect ordering lane inside it is not yet on any execution path.

## What was added

`crates/x3-swap-router/src/mev_protection/fair_ordering.rs` (new, 1399 lines, sha256
`96e0f42d7f9c655aa31099b8e948fbad7bf187d932915b034f61e6f87c145fbc`), plus 3 additive
lines of re-export in `mev_protection/mod.rs` and 4 in `lib.rs` so the lane is reachable
from outside the crate.

A window-bounded commit–reveal ordering lane:

* `commitment_hash(sender, plaintext, nonce)` =
  `BLAKE2b-256("X3:FAIR_ORDER:V1" ‖ plaintext ‖ nonce ‖ sender)`. One constructor, used by
  the committer and by the lane, so the thing committed and the thing checked cannot
  drift. The sender is bound in, so a commitment lifted from the mempool cannot be
  revealed by anyone else.
* `commit(sender, commit_hash, bond, block)` inside `OrderingWindow { open_block,
  close_block }` (both inclusive; the close block is the deadline). Refused when settled,
  outside the window, underbonded, from a sender that already committed, on a duplicate
  hash, or past `MAX_COMMITMENTS`.
* `reveal(sender, commit_hash, plaintext, nonce, block)` — re-derives the hash and
  compares. Refused in this fixed order: oversize payload, before the window, after the
  window, already settled, duplicate reveal, unknown commit, wrong sender, hash mismatch.
  A refusal never records anything.
* `install_beacon(beacon, block)` — accepted only **after** the window has closed, so no
  participant can choose a commit hash with the beacon known. One beacon per window.
* `settle(block)` — refused while the window is open, refused twice. Produces
  `WindowSettlement { order, unrevealed, beacon }`; the canonical order is
  `sort_by(order_key, commit_hash)` and `WindowSettlement::canonical_order()` recomputes it
  from the settlement alone.

Determinism: `BTreeMap` throughout for anything that participates in ordering, an explicit
total order with a tie-break, no clock, no `HashMap` iteration. Nothing reads
`committed_at_block` or `revealed_at_block` for ordering — they are stored for audit only.

## Honest limits (in the module docs and repeated here)

* With **no beacon**, the order key *is* the commit hash, so a participant that can grind
  nonces can choose where its *own* commitment lands among the commitments it can already
  see. It cannot move, drop or reorder anyone else's commitment. Sybil addresses are not
  prevented — one commitment per *sender address*, not per entity.
* With a beacon, placement is closed **only if** the beacon is genuinely unpredictable.
  The module enforces just one thing about it — that it cannot be installed while
  participants can still commit — and that is not the whole property: whoever calls
  `install_beacon` can grind the beacon and bias the entire order, so the beacon has to
  come from a source the caller cannot choose (a future block hash, a VRF, a threshold
  signature). Provenance is out of scope here, and named in the module docs. That is the
  remaining protocol-level gap.
* The lane moves no funds. Unrevealed commitments are reported in
  `WindowSettlement::unrevealed` with `forfeitable_bond()`; applying the bond is the
  caller's business.
* Nothing in the node calls the lane yet (see finding 4).

## Row delta

```
ROW DELTA for X3-MEV-008:
  name = "Fair transaction ordering"
  implemented = 45   (was 15) — a real, deterministic commit-reveal ordering lane exists
  tested      = 40   (was 5)  — 26 named tests over the lane, incl. every named refusal
  mainnet_ready = 10 (was 5)  — unchanged in substance: the crate has no consumer at all
  source = "master"  (was "research")
  paths = ["crates/x3-swap-router/src/mev_protection"]
  blockers = [
    "No execution path consumes x3-swap-router: cargo tree -i x3-swap-router --workspace returns only the crate itself, so the lane orders nothing on any chain yet",
    "Beacon unpredictability is an off-chain assumption; the lane only enforces that a beacon may not be installed while participants can still commit",
    "Sybil addresses are not prevented (one commitment per sender address)",
    "Bond forfeiture is reported, never applied — no pallet integration",
  ]
  required_tests = [
    <26 names, see the runlog>
  ]
  test_evidence = [".ai/runlogs/round4-mev008-fair-ordering-20260926.txt"]
```

`X3-MEV-006` ("MEV-resistant architecture", currently carrying `pallets/private-execution`
as its path) was **not** moved: this change is in the swap-router crate, which the pallet
does not depend on, and the row's own blocker ("No complete formal MEV threat model") is
untouched by it. Reporting that honestly rather than claiming a second row.

## Commands run

See `.ai/runlogs/round4-mev008-fair-ordering-20260926.txt` for the raw output, including the
four break-it-first controls.
