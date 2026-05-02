# Integration Plan

Status: Phase 1 complete, Phase 2 ready

Patch order:
1. Enumerate all files. Done: 114395 files in `.cache/file_list.txt`.
2. Build the coverage tracker. Done: `CODE_COVERAGE_TRACKER.md` populated.
3. Scan for smells, stubs, mocks, panics, unsafe code, and hardcoded local paths.
   Done: `.reports/smells.txt` generated.
4. Classify smell matches by path class: production, test-only, generated,
   archive, report evidence, vendored patch, environment artifact.
5. Extract implemented features and gaps from code.
6. Prioritize P0 launch blockers.
7. Patch only after a concrete file-backed gap is identified.
8. Run targeted checks after each patch.

Immediate Phase 2 target:
- Start with source roots: `runtime/`, `node/`, `pallets/`, `crates/`,
  `proof-forge/`, and `launch-gates/`.
- Keep `target_strict/`, `.venv/`, previous reports, and archive evidence in
  the ledger, but classify them separately from source readiness.

Do not start broad architectural mutation before the blocker list is real and
deduplicated.
