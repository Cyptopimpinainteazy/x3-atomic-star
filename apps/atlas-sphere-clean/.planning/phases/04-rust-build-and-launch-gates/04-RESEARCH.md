---
phase: "04"
name: "rust-build-and-launch-gates"
created: 2026-03-15
---

# Phase 4 Research: Rust Build and Launch Gates

## Goal Recap
Get core Rust build, test, WASM/runtime, and `x3-launch-validator` checks to pass on the intended release path.

## Existing Gate References (Repo Reality)

### Runtime/WASM Build Commands
- `.github/workflows/ci.yml` runs:
  - `cargo build -p x3-chain-runtime --release --target wasm32-unknown-unknown --no-default-features`
- `.github/scripts/run_real_vm_checks.sh` runs:
  - `cargo check -p pallet-x3-kernel -p evm-integration -p runtime --features std`
  - `cd runtime && cargo build --release --target wasm32-unknown-unknown --no-default-features`

These are the closest “existing runtime build scripts” and should be used as the Phase 4 gate commands.

### Test Surface References
- `.github/scripts/run_real_vm_checks.sh` runs tests for:
  - `runtime`, `pallet-x3-kernel`, `evm-integration`, plus optional `svm-integration` and `x3-integration`.
- `.planning/codebase/TESTING.md` references `RUN_ALL_TESTS.sh` and `run_e2e_tests.sh` for broader suites.

### Toolchain Baselines
- `rust-toolchain.toml` pins stable `1.88.0` with `rustfmt`, `clippy`, `rust-src`, and `wasm32-unknown-unknown`.
- Known-good nightly references in repo/docs:
  - `docs/OWNER_RUNBOOK.md` → `nightly-2024-12-01` (pinned)
  - `docs/DEPLOYMENT.md` → `nightly-2024-12-01` (override example)
- Phase 4 decision: CI uses a pinned nightly for WASM/runtime; local can keep stable.

### wasm-opt
- `wasm-opt` appears in runtime BOM files (`runtime/x3-chain-runtime.cdx.json`) but no explicit CI step installs it.
- Phase 4 decision: wasm-opt required in CI; should be preflighted and fail if missing.

## Implications for Phase 4 Gates

- Use `.github/scripts/run_real_vm_checks.sh` or the runtime build command it contains as the canonical runtime/WASM build gate:
  - `cargo build --release --target wasm32-unknown-unknown --no-default-features` (from `runtime/`)
- CI should install a pinned nightly toolchain (selected from existing known-good reference) with `rustfmt`, `clippy`, `rust-src`, and `wasm32-unknown-unknown`.
- Enforce `-D warnings` in CI for clippy/build/test.
- Launch validator gate should run:
  - `cargo run -p x3-launch-validator -- --check pre-launch`
  - `cargo run -p x3-launch-validator -- --check failure-conditions`
- Fail hard on toolchain install failures, wasm-opt missing, or WASM build errors.

## Files Likely to Update in Phase 4

- `.github/workflows/x3-audit.yml` (Phase 4 gates)
- `scripts/x3_audit.sh` (Phase 4 build/test/WASM/launch-validator additions)
- `.github/scripts/run_real_vm_checks.sh` (if standardizing command)
- `X3_SYSTEMS.md`, `X3_INDEX.md`, `X3_DEPLOYMENT_SOP.md` (document Phase 4 gate set)
- `X3_COMPLETION.md`, `X3_GAPS_REPORT.md` (Phase 4 status)
