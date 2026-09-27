# Disputed-task refund path — proof artifact (2026-09-27)

Task: the last handoff's "next seed" — `TaskStatus::Disputed` had no resolution
path, so a task whose claim slots filled with non-matching result hashes left
the submitter's reward reserved forever.

## Change

`pallets/northern-swarm` gains `resolve_disputed_task(task_id)` (call_index 8):

* refuses any task not in `TaskStatus::Disputed` (`Error::TaskNotDisputed`),
* zeroes the record's reward and moves the task to the new terminal
  `TaskStatus::Refunded`,
* unreserves the exact reserved reward back to the submitter,
* emits `DisputedTaskRefunded { task_id, submitter, amount }`.

Permissionless by design: the call can only ever move the submitter's own
reserved funds back to the submitter, so gating it on governance would itself be
a way to strand funds. Executor stakes are untouched — hashes alone cannot prove
which executor was wrong, so `SlashReason::QuorumMismatch` still has no
constructor.

## Commands and results

```
cargo test -p pallet-northern-swarm --all-targets
  -> 10 passed; 0 failed; 0 ignored

cargo test -p pallet-northern-swarm --features runtime-benchmarks
  -> 19 passed; 0 failed (incl. bench_resolve_disputed_task)

cargo clippy -p pallet-northern-swarm --all-targets -- -D warnings
  -> clean

cargo fmt -p pallet-northern-swarm -- --check
  -> clean

BENCHMARK_STEPS=50 BENCHMARK_REPEAT=20 \
  bash scripts/run-frame-benchmarks.sh run pallet-northern-swarm
  -> weights.rs regenerated; resolve_disputed_task ref_time 96_270_000 ps,
     proof size 4149, reads 2, writes 2

python3 scripts/mainnet/swarm_reactor_gate.py
  -> PASS (now also requires the dispute/refund tests and the extrinsic)

python3 scripts/ci/check-matrix-tests-exist.py
  -> OK - 428 required_tests citations resolve
python3 scripts/ci/check-matrix-test-evidence.py
  -> OK - 206 test_evidence citations resolve
bash scripts/check-readiness-consistency.sh
  -> PASS
python3 scripts/x3_audit_matrix.py --check
  -> PASS (artifacts regenerated)
```

## Break-it-first

Guard removed (the `ensure!(task.status == TaskStatus::Disputed, ...)` block):

```
cargo test -p pallet-northern-swarm --lib
  -> FAILED. 8 passed; 2 failed; 0 ignored
     resolve_disputed_task_cannot_be_replayed                       FAILED
       left:  Ok(())
       right: Err(Module(ModuleError { index: 2,
               error: [16, 0, 0, 0], message: Some("TaskNotDisputed") }))
     resolve_disputed_task_rejects_tasks_that_are_not_disputed      FAILED
       left:  Ok(())
       right: Err(Module(ModuleError { index: 2,
               error: [16, 0, 0, 0], message: Some("TaskNotDisputed") }))
```

Restored from the pre-mutation copy (kept locally, not committed, to avoid a
duplicate source file) and re-run:

```
sha256sum pallets/northern-swarm/src/lib.rs
  af76769b8d0fc0c80f28336a245e5f51885692beb49d77f6aae2709e62a16e4b
cargo test -p pallet-northern-swarm --lib
  -> 10 passed; 0 failed; 0 ignored
```

The restore was verified by hash, not by eye: the file was byte-identical to the
pre-mutation `af76769b8d0fc0c80f28336a245e5f51885692beb49d77f6aae2709e62a16e4b`.

## Note found along the way

The mock never recorded events: `derive_impl(TestDefaultConfig)` leaves
`frame_system::Config::RuntimeEvent = ()`, and `frame_system::deposit_event`
drops every event at block 0 (genesis), where `new_test_ext` started.
`mock.rs` now sets `type RuntimeEvent = RuntimeEvent;` and starts at block 1, so
the refund's event is asserted rather than assumed.
