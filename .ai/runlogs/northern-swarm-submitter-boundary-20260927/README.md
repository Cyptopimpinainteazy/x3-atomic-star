# Executor submitter boundary tests — proof artifact (2026-09-27)

Lane B part 1. The release prompt named the cases that must be covered:
wrong key, unregistered key, already-claimed task, already-finalised task,
duplicate result submission, bad payload URI, bad payload hash.

## What is provable without a node

The chain-side refusals (unregistered key, already-claimed, already-finalised,
duplicate result) are produced by the pallet and can only be observed against a
running chain. They are exercised in `pallets/northern-swarm/src/tests.rs`
(`resolve_disputed_task_*`, `quorum_requires_matching_results`,
`multiple_executors_can_claim_one_task_up_to_the_bound`, ...). This crate cannot
reach a live node in CI, and the row's first blocker says so rather than
pretending otherwise.

What this crate *can* prove — and now does, in
`crates/northern-swarm/src/result_submitter.rs`:

| case | test |
| --- | --- |
| wrong / malformed key | `a_malformed_executor_key_is_refused_rather_than_defaulted` |
| key selection is real | `the_dev_shorthand_key_derives_a_real_signer` |
| bad task id | `a_bad_task_id_is_refused_before_any_network_call` |
| bad result hash | `a_bad_result_hash_is_refused_before_any_network_call` |
| non-success result not submitted | `a_non_success_result_is_not_submitted` |

The two "before any network call" tests point the config at an unreachable
`ws://127.0.0.1:1`, so an RPC attempt would fail loudly; the parse error still
comes back, proving the boundary refuses locally rather than after connecting.

## Commands and results

```
CARGO_TARGET_DIR=/tmp/x3-target-swarm-b cargo test -p northern-swarm --all-targets
  -> 22 passed; 0 failed; 0 ignored   (was 17; +5)

CARGO_TARGET_DIR=/tmp/x3-target-swarm-b cargo clippy -p northern-swarm \
    --all-targets -- -D warnings
  -> clean

cargo fmt -p northern-swarm -- --check
  -> clean
```

## Break-it-first

Removing the `if result.status != ExecutionStatus::Success { return Ok(()) }`
guard makes the submitter try to reach the chain for a failed result:

```
CARGO_TARGET_DIR=/tmp/x3-target-swarm-b cargo test -p northern-swarm --lib a_non_success
  -> FAILED. a_non_success_result_is_not_submitted
```

Restored byte-identically:

```
sha256sum crates/northern-swarm/src/result_submitter.rs
  91786fb4e7250d504f11820bc09132cda963cadcb1c339b1e4dcadb6cc1c99bf
```

## Not covered here (honest)

The submitter never encodes a call against real metadata in these tests, so the
pallet/call *names* it uses are not validated against a live runtime — that is
the `No live-node integration test` blocker, still open.
