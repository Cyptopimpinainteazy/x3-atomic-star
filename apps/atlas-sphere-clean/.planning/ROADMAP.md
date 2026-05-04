# Roadmap: X3 Chain

## Overview

X3 Chain is a modular blockchain execution platform focused on high throughput, multi-VM support (EVM + SVM), and secure validator coordination. The next milestone is focused on turning the current monorepo into a release candidate by driving the real build, validation, security, and testnet gates to green.

## Milestone: v1.1 Release Readiness (In progress)

- ✅ **v1.1 Release Readiness** — Phases 3-8 (complete 2026-03-22)

## Archived Milestones

- ✅ **v1.0 Foundation** — Phases 1-2 (shipped 2026-03-15)

## Phases

### Phase 3: Delivery gate stabilization
**Goal:** Make the local release gates truthful and repeatable so the repo can be evaluated from a clean baseline.
**Depends on:** Phase 2
**Requirements:** [REQ-101]
**Plans:** 3 plans

Plans:
- [x] 03-01: Repair local audit and packaging scripts
- [x] 03-02: Align checklists and gap reports with current repo reality
- [x] 03-03: Define the minimal release gate command set for contributors

### Phase 4: Rust build and launch gates
**Goal:** Get the core Rust build, test, WASM, and launch-validator checks to pass on the intended release path.
**Depends on:** Phase 3
**Requirements:** [REQ-101, REQ-102]
**Plans:** 3 plans

Plans:
- [x] 04-01: Green `cargo check` / `cargo build` / `cargo fmt` on the release workspace
- [x] 04-02: Green targeted and workspace test suites for node, runtime, and critical crates
- [x] 04-03: Green `x3-launch-validator` and release-critical offline checks

### Phase 5: Dual-VM completion
**Goal:** Close the remaining production-critical EVM, SVM, and cross-VM bridge gaps.
**Depends on:** Phase 4
**Requirements:** [REQ-103, REQ-104, REQ-105]
**Plans:** 3 plans

Plans:
- [x] 05-01: Complete EVM deployment and integration coverage
- [x] 05-02: Complete SVM execution, deployment, and ledger sync coverage
- [x] 05-03: Prove cross-VM atomic flow end to end

### Phase 6: Security and runtime hardening
**Goal:** Remove known production-safety hazards across node, runtime, pallets, and RPC surfaces.
**Depends on:** Phase 5
**Requirements:** [REQ-106]
**Plans:** 3 plans

Plans:
- [x] 06-01: Eliminate critical `unwrap()` / `expect()` / `panic!()` paths in production code
- [x] 06-02: Harden RPC, rate limiting, and abuse controls
- [x] 06-03: Audit pallet permissions, events, and runtime safety invariants

### Phase 7: SDK and app packaging
**Goal:** Ensure the TypeScript packages and supported app surfaces build cleanly and match the release contract.
**Depends on:** Phase 4
**Requirements:** [REQ-101, REQ-107]
**Plans:** 3 plans

Plans:
- [x] 07-01: Green package builds for SDK, connector, and Polkawallet workspaces
- [x] 07-02: Close remaining SDK/API surface gaps required for release
- [x] 07-03: Produce release-ready package artifacts and usage docs

### Phase 8: Testnet proving and go/no-go
**Goal:** Validate X3 Chain under realistic startup and testnet conditions, then assemble the final ship decision package.
**Depends on:** Phase 5, Phase 6, Phase 7
**Requirements:** [REQ-102, REQ-107]
**Plans:** 3 plans

Plans:
- [x] 08-01: Run startup smoke and local multi-validator verification
- [x] 08-02: Validate deployment SOP, rollback, and operator runbooks
- [x] 08-03: Produce the final go/no-go checklist with signed release artifacts
