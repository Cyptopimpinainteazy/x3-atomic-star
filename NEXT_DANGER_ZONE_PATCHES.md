# Next Danger-Zone Patches

Danger-zone patches require:
- explicit scoped task
- tests
- audit note
- risk register update
- rollback plan

Do not patch these from a broad prompt. Turn each into one Traycer/Roo task.

## Candidate Queue

- Liquidity Locks: verify whether implementation exists under a different name; if missing, plan lock invariant tests before runtime/DEX changes.
- Anti-Rug Mechanics: verify launchpad/admin bypass protections; if missing, plan tests before implementation.
- Bridge / Router: review mapped replay/nonce/expiry risky files in `.x3/dashboards/INVARIANT_COVERAGE.md`.
- Universal Asset Kernel: review mapped supply invariant risky files before accounting changes.
- SVM Integration: review `pallets/svm-runtime` and `crates/svm-integration` panic/unsafe findings before mainnet claims.
