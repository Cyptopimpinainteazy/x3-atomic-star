# Feature Gap Report

Status: Phase 1 initialized

Rules:
- Code is proof.
- Markdown is a hint.
- No feature is complete until implementation and validation evidence exist.

Initial evidence:
- Full scan file count: 114435
- Smell report lines: 55881
- Deep file-by-file coverage: 0%

Current gaps:
- Full file-by-file deep scan is not complete.
- Smell report contains `unimplemented!`, `todo!`, `panic!`, `unwrap(`,
  `expect(`, `mock`, `hardcoded`, `unsafe`, and `localhost` matches that need
  classification into test-only, archive-only, generated, or production-path
  blockers.
- Raw scan includes generated/environment-heavy roots such as `target_strict/`
  and `.venv/`; source coverage and artifact inventory need separate ledgers.

First production-code hotspot observed:
- `crates/cross-vm-bridge/src/lib.rs`: 113 smell matches in the initial report.
