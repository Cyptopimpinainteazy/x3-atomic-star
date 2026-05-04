---
phase: "03"
name: "delivery-gate-stabilization"
created: 2026-03-15
---

# Phase 3: delivery-gate-stabilization — Context

## Decisions

- **Gate strictness:** Strict in CI only; local runs may warn without failing.
- **Truthfulness policy:** Fix code first when feasible; if docs are stale, update them with explicit notes.
- **Missing tools in CI:** Treat missing tooling as a CI failure.
- **`x3_audit.sh --ci` exit policy:** Fail CI on failures and warnings.
- **Source of truth:** Hybrid mapping between executable gates and checklists.
- **Reconciliation method:** Update scripts and docs together, with rationale notes.
- **Checklist scope:** Tighten scope for v1.1 (prune or clearly defer out‑of‑scope items).
- **CI workflow:** Align `.github/workflows/x3-audit.yml` now with the local gate set.
- **Minimal Rust gates (Phase 3):** `cargo check` + `cargo fmt` only.
- **WASM/runtime build checks:** Not in Phase 3; defer to Phase 4.
- **TypeScript package builds:** Include `npm run build:all-packages --if-present` in minimal gates.
- **Launch validator:** Keep `x3-launch-validator` in Phase 4.
- **Infra/toolchain crashes:** Non‑blocking if clearly infra‑caused, but must be labeled and tracked.
- **Toolchain pinning:** Document versions and setup only (no enforcement yet).
- **Pending live‑env checks:** Allow as pending with explicit labeling.
- **Preflight in `x3_audit.sh`:** Add preflight validation as warn‑only.

## Discretion Areas

- Implementation details for audit script output formatting, as long as pass/warn/fail semantics remain consistent.
- How to present the v1.1 tightened checklist (overlay vs pruning) as long as deferrals are explicit.

## Deferred Ideas

_Ideas to consider later_
