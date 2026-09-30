# x3-sim — deterministic fault-injection simulator

`x3-sim` runs the **real** cross-VM atomic swap coordinator
(`crates/cross-vm-coordinator`) against a virtual clock, a seeded RNG, a
lossy/partitionable network and a scheduled fault plan. A run is pinned by its
seed, and a failing run prints the exact command that reproduces it.

```bash
cargo run -p x3-sim -- --seed 948218671 --scenario partition-storm
```

It is its own cargo workspace root, for the same reason the coordinator is: it
path-depends on a crate the parent Substrate workspace excludes.

## Why it exists

The coordinator passes 117 unit tests that call one operation at a time. That
cannot express the failures that actually hurt: a claim racing a refund across
a partition, a duplicate delivery after a restart, a persisted write that
disappears. `x3-sim` schedules those orderings deterministically instead of
hoping a human thinks of them.

It does **not** re-implement any transition. If it did, a passing run would only
prove the simulator agrees with itself. Every state change comes from the
coordinator crate; `x3-sim` chooses the order of calls and judges the resulting
state.

## Scenarios

| Scenario | What it schedules |
| --- | --- |
| `happy-path` | Every session walks the lifecycle on an ordered, lossless network. The positive control. |
| `claim-refund-race` | Claims and refunds scheduled against each other, plus a partition. |
| `partition-storm` | Repeated partitions, a slow link and message loss. |
| `crash-recovery` | Two crash-and-restarts from persistence, plus a lost write. |

## Options

| Flag | Meaning |
| --- | --- |
| `--seed <u64>` | Seed for the run. Replay uses the same value. |
| `--scenario <name>` | One of the scenarios above. |
| `--sessions <n>` | Atomic swap sessions in the ledger (default 8). |
| `--steps <n>` | Scheduler steps (default 120). |
| `--nodes <n>` | Node 0 is the coordinator, the rest are clients (default 4). |
| `--out <dir>` | Write an evidence bundle (JSON + trace) to this directory. |
| `--json` | Print the evidence bundle instead of a summary. |

Exit codes: `0` all invariants held, `1` a violation was found, `2` usage error.

## Invariants checked

After every step, and across sessions:

| Code | Meaning |
| --- | --- |
| `CLAIM_REFUND_MIX` | One leg claimed while the other was refunded. |
| `COMPLETE_WITHOUT_BOTH_CLAIMS` | Phase `Complete` without both legs claimed. |
| `REFUNDED_WITH_A_CLAIM` | Phase `Refunded` with a claimed leg still recorded. |
| `PHASE_WITHOUT_FAST_HTLC` | A phase past locking with no fast-chain record. |
| `TIMELOCK_ORDER_INVERTED` | Slow-chain timelock not after the fast one. |
| `DUPLICATE_JOURNAL_ENTRY` | One semantic operation recorded twice with the same evidence. |
| `REFUND_AFTER_CLAIM` | The durable journal records a claim and then a refund. |
| `DOUBLE_SETTLE` | One hash lock completed two swaps. |

`REFUND_AFTER_CLAIM` reads the operation journal rather than the leg statuses,
because `record_refunds` overwrites the statuses — the journal is the only place
the earlier claim survives.

## What it found

`SwapCoordinator::abort` set `phase = Aborting` without consulting the transition
table every other mutator uses. A swap that had already completed (both legs
`CLAIMED`) could be aborted and then refunded, paying both sides twice. The
reproduction lives in `tests/refund_after_claim.rs`; the fix is in
`crates/cross-vm-coordinator/src/state_machine.rs`, with the regression test next
to it.

## Test suite

```bash
cargo test --manifest-path crates/x3-sim/Cargo.toml
```

* `tests/determinism.rs` — the same seed reproduces the same trace and state;
  the seed really does change the schedule; the happy path completes.
* `tests/invariants.rs` — the checker is not vacuous: each invariant fires on a
  hand-built state that a bug would produce.
* `tests/refund_after_claim.rs` — the reproduced defect, plus a positive control
  that a legitimate refund still works.

## Scope

v0 simulates the coordinator lifecycle. It does not yet simulate consensus
across validators, EVM/SVM execution, or the full runtime — those need their own
harnesses, and the simulator's seams (`VirtualNetwork`, `FaultPlan`,
`invariants`) are where they plug in.
