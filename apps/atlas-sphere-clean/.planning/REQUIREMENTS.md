# Requirements: X3 Chain

**Defined:** 2026-03-15
**Core Value:** A modular, high-throughput blockchain node that supports EVM and SVM execution securely and reliably.

## v1.1 Requirements

### Release Gates

- [ ] **REQ-101**: Local release gates are trustworthy and repeatable, including `scripts/x3_audit.sh --ci`, package builds, and the documented contributor command set.
- [ ] **REQ-102**: Core Rust release gates pass on the intended path, including cargo build/test/fmt targets, WASM/runtime checks, and `x3-launch-validator`.

### Product Completeness

- [ ] **REQ-103**: EVM support includes contract deployment plus integration coverage for release-critical flows.
- [ ] **REQ-104**: SVM support includes Sealevel execution, deployment validation, and canonical-ledger synchronization.
- [ ] **REQ-105**: Cross-VM atomic transfer and message flow is proven end to end with repeatable tests.

### Security and Operations

- [ ] **REQ-106**: Critical production-safety gaps from `X3_GAPS_REPORT.md` and `X3_COMPLETION.md` are closed or explicitly accepted as deferred debt.
- [ ] **REQ-107**: Operators can boot, validate, recover, and assess release readiness using the documented startup, testnet, and deployment runbooks.

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| REQ-101 | Phase 3, Phase 4, Phase 7 | In progress (Phases 3-4 complete) |
| REQ-102 | Phase 4, Phase 8 | In progress (Phase 4 complete) |
| REQ-103 | Phase 5 | Pending |
| REQ-104 | Phase 5 | Pending |
| REQ-105 | Phase 5 | Pending |
| REQ-106 | Phase 6 | Pending |
| REQ-107 | Phase 7, Phase 8 | Pending |

**Coverage:**
- v1.1 requirements: 7 total
- Mapped to phases: 7
- Unmapped: 0

---
*Requirements defined: 2026-03-15*
*Last updated: 2026-03-15*
