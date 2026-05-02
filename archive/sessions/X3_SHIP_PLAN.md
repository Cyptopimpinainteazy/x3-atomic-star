# X3 Atomic Star v0.4 Mainnet Ship Plan

## Assessment on Completion Percentage to Mainnet

Based on the current codebase audit:

- **Full X3 v0.4 mainnet (all features)**: Approximately 15% complete
- **Minimal internal-only mainnet path**: Approximately 35% complete

The repo has strong foundations in the Substrate-style core (pallets for kernel, asset registry, cross-VM router, etc.) but lacks key components like x3-ixl instruction layer, x3-packet-standard lifecycle, and production-grade external gateway. The fastest path is to focus on the minimal internal path first, with external features as parallel workstreams.

## Prioritized Implementation Plan

### Minimal Fast-to-Mainnet Path (12-Week Aggressive Schedule)

Focus on internal X3Native/X3Evm/X3Svm routing, one atomic bundle pipeline, one spot-swap path, minimal packet semantics, minimal IXL, hardened invariants, reproducible CI, and real readiness reporting.

#### Sprint Breakdown

1. **Foundation Hardening (Weeks 1-2)**: Replace stubs with real code, harden CI, wire missing pallets.
2. **Packet Standard MVP (Weeks 3-4)**: Build x3-packet-standard crate with lifecycle semantics.
3. **IXL MVP (Weeks 5-6)**: Build x3-ixl execution plane.
4. **Internal Mainnet Flow (Weeks 7-8)**: Wire end-to-end internal routing.
5. **Liquidity-Core Consolidation (Week 9)**: Clean spot AMM implementation.
6. **Readiness and Launch Gates (Week 10)**: Real evidence-based checks.
7. **Full Gateway Branch (Weeks 11-12)**: Start external gateway work parallel to mainnet RC.
8. **Services Branch (Ongoing)**: Wrap existing services.
9. **Deferred Features**: Parallel executor, AppZone factory, PQ - post-minimal RC.

### Key Deliverables by Week

- **Week 1 (Apr 27-May 3)**: Scope freeze, CI hardening, readiness-report rewrite.
- **Week 2 (May 4-10)**: Packet standard skeleton, fuzz targets.
- **Week 3 (May 11-17)**: Packet semantics, IXL skeleton.
- **Week 4 (May 18-24)**: IXL interpreter, gateway branch creation.
- **Week 5 (May 25-31)**: Internal wiring, swap path.
- **Week 6 (Jun 1-7)**: Liquidity-core spot subset.
- **Week 7 (Jun 8-14)**: Fuzzing, proofs, try-runtime.
- **Week 8 (Jun 15-21)**: Testnet infra hardening.
- **Week 9 (Jun 22-28)**: Release candidate, launch validator upgrade.
- **Week 10 (Jun 29-Jul 5)**: Audit fixes, full-path branches.
- **Week 11 (Jul 6-12)**: Multi-chain adapters.
- **Week 12 (Jul 13-19)**: Go/no-go decision.

### Critical Implementation Tasks

1. **Scope Freeze**: Modify pallets/x3-cross-vm-router/src/lib.rs, runtime configs, status docs for internal-only scope.
2. **Build x3-packet-standard**: Add crates/x3-packet-standard/ with packet lifecycle, tests, fuzzing.
3. **Build x3-ixl**: Add crates/x3-ixl/ with interpreter, planner, rollback.
4. **Wire Internal Flow**: Integrate kernel, router, ledger, IXL.
5. **Liquidity Core**: Create x3-liquidity-core from x3-dex components.
6. **Readiness Report**: Replace hard-coded stubs with real runtime queries.
7. **Testing Suite**: Add property tests, fuzzing, Kani proofs, E2E tests, benchmarks.
8. **External Gateway**: Parallel branch with x3-external-liquidity-gateway.
9. **Services**: x3-integrated-services wrapper.
10. **CI Gates**: Strict workspace builds, try-runtime, benchmarks.

### Risks and Mitigations

- **Planning Drift**: Freeze target matrix in docs/v0_4_ship_scope.md.
- **Stub Theater**: Fail-closed launch scripts, remove hardcoded paths.
- **Incomplete Wiring**: Audit runtime/src/lib.rs pallet wiring before claims.
- **Over-Scope**: Split minimal-path vs. full-feature tracks.

### Mandatory Pre-Mainnet Gates

- Property tests and fuzzing for critical modules.
- Runtime simulation and benchmarks.
- E2E tests for internal flows.
- Fault injection and chaos testing.
- Operational runbooks and go/no-go review.

This plan prioritizes speed to a credible first mainnet over feature completeness, with parallel tracks for full features.