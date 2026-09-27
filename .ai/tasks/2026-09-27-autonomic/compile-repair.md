# Lane: `x3-autonomic-core` does not compile, and nothing noticed

Found 2026-09-27 while acting on the repo scanner's `ungated-crate` findings
(`python3 scripts/swarm/x3_repo_scan.py | grep ungated-crate`): three crates under
`x3-autonomic-core/` have test attributes and no gate runs them. The reason turned
out to be more basic than "no gate".

## What is actually wrong

1. **Fixed here:** `crates/x3-autonomic-types/Cargo.toml` asked for
   `chrono = { version = "0.4", features = ["derive"] }`. `chrono` has no `derive`
   feature at any 0.4 version, so the whole `x3-autonomic-core` workspace failed
   dependency resolution — `cargo metadata` on it errored out. Changed to
   `features = ["serde"]`, which is what the timestamp fields need. Resolution now
   succeeds and picks `chrono 0.4.45`.
2. **Still open:** with resolution fixed, `cargo test -p x3-autonomic-core
   -p x3-live-auditor -p x3-regression-engine` fails to compile:
   * `x3-autonomic-types`: 12 × `the trait bound f64: TypeInfo is not satisfied`
     (these substrate versions have no `TypeInfo`/`Encode` for `f64`), and
   * an unresolved import of `parity_scale_codec` in the same crate.
   Measured: 14 errors from `x3-autonomic-types` before the rest of the workspace
   is even reached.

## Why it matters

These crates carry the autonomic control plane's live auditor, regression engine,
shadow runner, judge agent and health pallet. Nothing in `scripts/local-ci.sh`
builds or tests any of them, so the breakage is invisible: the repository reports
0 BROKEN rows for a tree that does not compile.

## Deliverable

1. Make `x3-autonomic-types` compile on this substrate version: `f64` fields need
   a representation that is `Encode`/`Decode`/`TypeInfo`-compatible (fixed-point
   integers, or `#[codec(skip)]` with a documented reason), and the
   `parity_scale_codec` import has to name the crate as it is actually declared
   (`parity-scale-codec` is `codec` in most manifests here).
2. Get `cargo test --manifest-path x3-autonomic-core/Cargo.toml` green for the
   packages that have tests, and commit the workspace's `Cargo.lock` (it does not
   exist today, so `--locked` gates cannot be written for it).
3. Add a gate line to `scripts/local-ci.sh` naming each package, so the suites
   run, and re-baseline `docs/reports/repo-scan-baseline.json` — the scanner's
   `ungated-crate` count must fall.
4. If any package is deliberately dead, delete it instead of gating it, and say
   so in the commit.

## Rules

Shared tree: never `git add -A`, carve explicit paths, and do not edit
`scripts/local-ci.sh` while a local-ci run is executing. Use
`CARGO_TARGET_DIR=/tmp/x3-autonomic` so the build does not fight the other lanes.
