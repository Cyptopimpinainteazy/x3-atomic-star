# Migration Inventory

Status: Phase 1 initialized

Purpose:
- Track files, modules, scripts, and docs that need migration, consolidation,
  deletion, or integration into canonical X3 paths.
- Do not mark an item migrated without code proof and a validation command.

Inventory baseline:
- Total enumerated files: 114435
- Legion file list: `.cache/x3_full_file_list.txt`
- Repomix context packs: `.repomix/` after `.scripts/x3_repomix_pack.sh`
- Source-heavy roots: `crates/`, `pallets/`, `runtime/`, `node/`, `proof-forge/`
- Proof/report-heavy roots: `launch-gates/`, `proof/`, `reports/`
- App-heavy roots: `apps/`, `packages/`, `x3fronend/`
- Generated/environment-heavy roots currently included: `target_strict/`, `.venv/`

Migration candidates:
- Classify `target_strict/` and `.venv/` separately before using raw coverage as
  source-readiness proof.
- Deduplicate mirrored paths under `apps/atlas-sphere-clean/` versus root-level
  app/crate/package paths before patching either copy.
- Treat prior generated reports as evidence archives, not active source modules.
