---
phase: "03"
name: "delivery-gate-stabilization"
created: 2026-03-15
---

# Phase 3 Research: Delivery Gate Stabilization

## Goal Recap
Make the local release gates truthful and repeatable so the repo can be evaluated from a clean baseline. Phase 3 focuses on reliable, minimal gates (not full release readiness).

## Current Gate Artifacts (Reality Check)

- `scripts/x3_audit.sh`
  - Runs repo structure checks, `cargo check --workspace`, safety scans, `cargo deny` if available, and CI workflow presence.
  - Uses `require_cmd` that **skips** checks when tools are missing.
  - `--ci` sets `CI_MODE=1` but does **not** force warnings to fail (STRICT defaults to 0).
- `.github/workflows/x3-audit.yml`
  - Runs **release build**, **clippy**, **full tests**, and **launch validator** in addition to self-audit.
  - This is **Phase 4+ scope**, not Phase 3.
- Checklist/Doc surfaces
  - `X3_COMPLETION.md` uses all-or-nothing ✅/⬜ across broad v1.0.0 scope.
  - `X3_GAPS_REPORT.md` lists large cross-system backlog.
  - `X3_SYSTEMS.md`, `X3_INDEX.md`, `X3_GOLIVE_CHECKLIST.md`, `X3_AUDIT_DASHBOARD.md`, `X3_DEPLOYMENT_SOP.md`
    describe **strict, full-release gating** and imply CI fails on any unchecked checklist item.

## Mismatches vs Phase 3 Decisions

- **Gate strictness**: local should warn, CI should fail on warnings. Current `x3_audit.sh --ci` does not.
- **Missing tools**: should fail CI. Current behavior is skip + warn.
- **Minimal Rust gates**: should be `cargo check` + `cargo fmt` only. CI currently runs release build/test/clippy.
- **TypeScript package builds**: `npm run build:all-packages --if-present` is required but not currently executed by `x3_audit.sh`.
- **Checklist truth**: Docs claim full checklist gating, but Phase 3 requires a **hybrid mapping** (executable gates + scoped checklist items), with explicit deferrals.

## Phase 3 Policy Anchors (from CONTEXT.md)

- Strict gates in CI only; local runs may warn.
- `x3_audit.sh --ci` fails on failures **and warnings**.
- Missing tooling is a CI failure.
- Minimal Rust gates: `cargo check` + `cargo fmt` (no WASM build, no launch validator).
- Include `npm run build:all-packages --if-present`.
- Tighten checklist scope for v1.1 with explicit deferrals.
- Pending live-env checks allowed if explicitly labeled.
- Infra/toolchain crashes are non-blocking **only if clearly infra-caused** and tracked.

## Proposed Phase 3 Minimal Gate Set (Contributor + CI)

1. `bash scripts/x3_audit.sh --ci` (strict warnings in CI)
2. `cargo check --workspace`
3. `cargo fmt --all -- --check`
4. `npm run build:all-packages --if-present`

These must be **consistent** across:
- `scripts/x3_audit.sh`
- `.github/workflows/x3-audit.yml`
- Contributor docs (`X3_INDEX.md`, `X3_SYSTEMS.md`, `X3_GOLIVE_CHECKLIST.md`)

## Files Likely to Update in Phase 3

- Scripts: `scripts/x3_audit.sh`
- CI: `.github/workflows/x3-audit.yml`
- Checklists: `X3_COMPLETION.md`, `X3_GAPS_REPORT.md`
- Docs: `X3_SYSTEMS.md`, `X3_INDEX.md`, `X3_GOLIVE_CHECKLIST.md`, `X3_AUDIT_DASHBOARD.md`, `X3_DEPLOYMENT_SOP.md`

## Risks / Notes

- CI currently enforces Phase 4+ gates; trimming to Phase 3 scope is required to avoid false failures.
- Checklist scope changes must be explicit to avoid “truth” drift.
- Infra/toolchain exception handling should be documented and labeled to avoid silent bypasses.
