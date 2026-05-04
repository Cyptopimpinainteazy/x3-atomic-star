# X3 Chain Testing Overview

**Document:** Testing Strategy & Locations  
**Date:** 2026-03-15  
**Scope:** Rust, JS/TS, Python, and EVM/SVM test surfaces

---

## Test Surfaces

### Rust (Core + Runtime)
- **Unit/Integration tests:** scattered across crates and pallets  
  Examples: `crates/x3-compiler/tests/`, `pallets/*/src/tests.rs`
- **E2E tests:** `tests/e2e/` (Rust crate in `tests/e2e/Cargo.toml`)
- **Integration suites:** `integration-tests/`
- **Orchestration:** `RUN_ALL_TESTS.sh`, `run_e2e_tests.sh`

### Frontend (JS/TS)
- **Vitest:** `apps/x3-desktop/package.json` (scripts: `test`, `test:watch`, `test:coverage`)
- **Playwright:** `apps/x3-desktop/package.json` + `apps/x3-desktop/playwright.config.ts`
- **Test files:** `apps/x3-desktop/src/**/*.test.ts(x)`

### Python
- **GPU/validator testing:** `cross-chain-gpu-validator/tests/`
- **Pytest references:** `P4_DAY1_EXECUTION_BLUEPRINT.py`, `tests/` utilities

### Solidity / EVM
- **Hardhat workspaces:** `contracts/core/package.json`, `botchain-tri-vm-genesis/hardhat/package.json`

### SVM / Solana Programs
- **Program crates:** `programs/*/Cargo.toml` (test patterns follow Rust conventions)

---

## Common Commands & Scripts

- **All tests:** `RUN_ALL_TESTS.sh`
- **E2E tests:** `run_e2e_tests.sh`
- **Rust unit tests (typical):** `cargo test --all --locked` (referenced in `X3_DELIVERY_SUMMARY.md`)
- **Desktop app tests:** `npm test` in `apps/x3-desktop/`

---

## Notes & Gaps

- Test coverage is multi-language and fragmented across subprojects.
- Some subsystems (cross-VM execution) rely on ad-hoc or limited integration testing.
- CI orchestration is not centralized in a single runner; scripts are spread across `scripts/` and top-level shells.

