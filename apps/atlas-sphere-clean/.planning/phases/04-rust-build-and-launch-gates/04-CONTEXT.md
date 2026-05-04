---
phase: "04"
name: "rust-build-and-launch-gates"
created: 2026-03-15
---

# Phase 4: rust-build-and-launch-gates — Context

## Decisions

### Toolchain Strategy
- **CI toolchain for WASM/runtime:** Use a pinned nightly aligned to an existing known‑good nightly in repo/docs (to be identified and documented).
- **CI components:** Install `rustfmt`, `clippy`, and `rust-src`.
- **Toolchain failures:** Hard fail on nightly install issues (no exceptions).

### Gate Strictness
- **CI warnings:** Treat warnings as errors (`-D warnings`) for clippy and build/test gates.
- **Flaky tests:** Fail CI immediately; no retries.
- **Local strictness:** Document `--strict` for local runs to fail on warnings.

### WASM / Runtime Build
- **Build command:** Use the existing runtime build script/command in the repo (to be identified and standardized).
- **wasm-opt:** Required in CI; fail if missing.
- **WASM build failures:** Hard fail CI on toolchain/LLVM/WASM build issues (no exceptions).

### Launch Validator
- **Commands:** 
  - `cargo run -p x3-launch-validator -- --check pre-launch`
  - `cargo run -p x3-launch-validator -- --check failure-conditions`
- **Where to run:** CI + locally documented command set.
- **Environment dependencies:** Require default config paths; fail if missing.

## Discretion Areas

- Identifying the exact existing runtime/WASM build command in the repo, and documenting it consistently across scripts and CI.

## Deferred Ideas

_None — discussion stayed within Phase 4 scope._

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Release Gates + Docs
- `X3_COMPLETION.md` — phase scope markers and gate checklist alignment
- `X3_GAPS_REPORT.md` — release readiness gaps, including build/test and WASM items
- `X3_SYSTEMS.md` — gate documentation and enforcement narrative
- `X3_INDEX.md` — contributor gate command set and CI references
- `X3_DEPLOYMENT_SOP.md` — operational gating steps

### Toolchain + CI
- `rust-toolchain.toml` — documented stable toolchain baseline
- `.github/workflows/x3-audit.yml` — current CI gate workflow
- `scripts/x3_audit.sh` — audit runner and gate enforcement

### Codebase Maps
- `.planning/codebase/STACK.md` — stack summary and Rust/WASM constraints
- `.planning/codebase/CONVENTIONS.md` — coding/diagnostics conventions
- `.planning/codebase/TESTING.md` — test surfaces and scripts

## Code Context

### Reusable Assets
- `scripts/x3_audit.sh` — primary local/CI gate runner to extend for Phase 4 gates.
- `.github/workflows/x3-audit.yml` — CI entry point to align with new gate set.
- `rust-toolchain.toml` — stable baseline; nightly pin must be added/linked explicitly.

### Established Patterns
- CI runs should be strict and fail on warnings; local runs can be lenient unless `--strict` is used.
- Gate definitions should be mirrored in docs (`X3_SYSTEMS.md`, `X3_INDEX.md`, `X3_DEPLOYMENT_SOP.md`).

### Integration Points
- CI workflow → `scripts/x3_audit.sh` (entry point).
- Runtime/WASM build command(s) → to be sourced from existing scripts or Make targets.
- Launch validator executed via `cargo run -p x3-launch-validator`.

## Specific Ideas

- Keep CI hard‑fail semantics for nightly/toolchain/WASM/launch‑validator. No infra exceptions.
- Document local `--strict` path alongside CI commands.

---
*Phase: 04-rust-build-and-launch-gates*
*Context gathered: 2026-03-15*
