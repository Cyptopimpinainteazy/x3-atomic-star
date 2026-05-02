# Mainnet Readiness Delta

Status: Phase 1 initialized

This file tracks changes between current repo state and mainnet-ready state.

Delta categories:
- Build and test evidence
- Runtime and pallet readiness
- Proof and receipt coverage
- Benchmark and weights evidence
- Operational runbooks
- Security and custody boundaries
- Remaining P0/P1/P2 blockers

Current delta:
- Full scan baseline now exists.
- Deep scan coverage remains 0%, so readiness must not be promoted from this
  bootstrap alone.
- Initial smell scan shows unresolved production-risk keywords. These need
  classification and targeted fixes before any stronger launch-readiness claim.
- Scanner includes `target_strict/` and `.venv/`, so raw coverage percentage is
  not yet a clean mainnet-readiness metric.
- Roo Legion profiles and roles are configured, but old/current project
  comparison is blocked until the old project path exists or `OLD_PROJECT_ROOT`
  is set.
- Traycer specs and Repomix automation are configured. Repomix packs are not
  proof of file-by-file coverage; use them as context accelerators for scoped
  Traycer/Roo tasks.
