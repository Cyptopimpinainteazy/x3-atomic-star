# Public Testnet Follow-up Issues (Post v0.4 Internal RC)

These are known items that are explicitly *not* solved by the internal-only v0.4
scope, but must be closed before public testnet.

## ISSUE-01: Router full-suite CI coverage

- Component: pallets/x3-cross-vm-router
- Previous state: legacy suite had `BadOrigin` failures from fixture drift in
  `bootstrap_x3_asset` using root origin for `mint_canonical`.
- Current state: resolved. Fixture now uses signed governance origin and full
  router lib suite passes (`18/18`).
- Risk: low if coverage remains in CI, medium if only scoped tests are run.
- Required before public testnet:
  - Add CI job that runs the full router test matrix (not only scoped launch blockers)
  - Keep packet+IXL integration tests in the required set

## ISSUE-02: Router-bypass surfaces outside runtime kill-switch

- Component: relayer/off-chain bridge flows
- Symptom: runtime kill-switch (`ExternalBridgesEnabled`) only gates pallet entrypoints.
  Any direct off-chain path that bypasses router dispatch is outside this control.
- Current state: known risk; not yet fully audited
- Risk: high (could reopen external bridge surface while pallet gate remains off)
- Required before public testnet:
  - Enumerate all off-chain submission paths that can affect cross-chain settlement
  - Prove each path checks router gate state before acting, or remove/disable path
  - Add negative integration test: gate off + relayer active must yield zero accepted external roots
  - Add operational control: alert when relayer attempts external submission while gate is off

## ISSUE-03: Balance reconciliation runtime API still missing

- Component: crates/x3-readiness-report + runtime API
- Symptom: `balance_reconciliation` remains `Unknown` until `canonical_ledger_reconcile` runtime API ships
- Current state: intentional and correct; collector is pinned to Unknown with reason
- Risk: low for honesty, medium for launch evidence completeness
- Required before public testnet:
  - Implement runtime API endpoint
  - Wire collector query to live endpoint
  - Add integration test proving `Pass` only when reconciliation actually succeeds
