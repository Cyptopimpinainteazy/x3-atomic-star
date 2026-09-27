# AutoBackend: parity, quarantine, telemetry — proof artifact (2026-09-27)

Lane B part 2 of the 2026-09-27 swarm round. The blocker (X3-SWARM-002 /
`[northern_swarm_reactor]`): there was no `AutoBackend`; `TaskExecutor` used
`GpuBackend` directly and accepted whatever it returned.

## The defect

`GpuBackend` wraps `x3_accel::sha256_with_parity`, which recomputes on CPU for
the concrete wgpu backend — but the *selection* path did not verify anything: a
`ComputeBackend` implementation that returned a wrong answer would have its
output returned verbatim, and `TaskExecutor` had no reference to compare
against. AGENTS §18: a GPU that disagrees with the canonical verifier must fail
closed.

## Change

`crates/northern-swarm/src/backend.rs` gains `AutoBackend`:

* always computes the CPU reference (the deterministic canonical output);
* uses the accelerator only when present, not quarantined, and it `supports` the
  kind;
* agreement → returns the accelerator output;
* divergence → quarantines the device, stores a `Divergence { task_id,
  accelerator, accelerator_hash, reference_hash }`, counts it in
  `BackendTelemetry`, and returns the **CPU** result;
* refusal/error → counted fallback (`accelerator_refusals`), not a divergence;
* `execute` fails closed on a kind it cannot serve (`AiInference`), even if the
  caller ignored `supports`.

`TaskExecutor` now runs through `AutoBackend`. The mutex helper recovers a
poisoned lock instead of panicking; the counters decide nothing about the
result (the byte comparison does), so a poisoned lock cannot corrupt output.

## Commands and results

```
CARGO_TARGET_DIR=/tmp/x3-target-swarm-b cargo test -p northern-swarm --all-targets
  -> 17 passed; 0 failed; 0 ignored   (was 13; +4)

CARGO_TARGET_DIR=/tmp/x3-target-swarm-b cargo clippy -p northern-swarm \
    --all-targets -- -D warnings
  -> clean

cargo fmt -p northern-swarm -- --check
  -> clean

bash scripts/mainnet/panic_unwrap_audit.sh
  -> production 450 = baseline 450 (no growth)

python3 scripts/mainnet/swarm_reactor_gate.py
  -> PASS (Swarm 4 now requires AutoBackend + a quarantine test)

python3 scripts/ci/check-matrix-tests-exist.py   -> OK (432 citations)
python3 scripts/ci/check-matrix-test-evidence.py -> OK (206 citations)
bash scripts/check-readiness-consistency.sh      -> PASS
python3 scripts/x3_audit_matrix.py --check       -> PASS
```

## Break-it-first

The selecting guard is the byte comparison. Replacing
`Ok(candidate) if candidate == reference =>` with `Ok(candidate) =>` (accept the
device unconditionally) and re-running:

```
CARGO_TARGET_DIR=/tmp/x3-target-swarm-b cargo test -p northern-swarm --lib auto_backend
  -> FAILED. 3 passed; 1 failed
     auto_backend_quarantines_a_divergent_accelerator_and_returns_the_cpu_result
       left:  [222, 173, 190, 239]        <- the fake's 0xDEADBEEF
       right: [27, 105, 244, 237, ...]    <- the CPU reference
```

Restored from the pre-mutation copy and re-run:

```
sha256sum crates/northern-swarm/src/backend.rs
  5cd57c36d61fe868b94dbf02adb6d2835e777048fcf1ef7a75fd301c70904619
CARGO_TARGET_DIR=/tmp/x3-target-swarm-b cargo test -p northern-swarm --lib auto_backend
  -> 4 passed; 0 failed
```

## Honest residual

The parity check costs a second hash for the current hash-shaped workload, so on
this crate the accelerator is not a speed win today — the interface exists so a
future verifiable-compute backend can be plugged in without ever letting an
unverified device decide a consensus-relevant hash. That trade (Safety over
Performance, AGENTS §28) is intended, not hidden.
