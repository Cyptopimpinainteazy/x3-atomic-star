# X3 Swarm Scripts

This folder contains local swarm orchestration helpers for X3 Atomic Star.

- `approve_task.sh`: manage manual swarm tasks and generate a task summary report.
- `swarm_health.sh`: verify the swam API and worker health.
- `swarm_self_test.sh`: run a local functional smoke test, including `approve_task.sh report`.
- `swarm_scan.sh` / `x3_repo_scan.py`: the repository scanner. It reports findings, not a file dump.
- `test_x3_repo_scan.py`: the fixture-driven tests for the scanner (`pytest -q`).

## `swarm_scan.sh` — the repository scanner

```bash
scripts/swarm/swarm_scan.sh              # write reports/swarm_scan_report.md + .json
scripts/swarm/swarm_scan.sh --check      # exit 1 when a ratcheted finding count grew
scripts/swarm/swarm_scan.sh --patches    # also write .ai/patches/<id>.patch for mechanical fixes
scripts/swarm/swarm_scan.sh --json       # findings JSON on stdout
```

Every finding carries `id, severity, kind, path, line, symbol, why, suggested_fix, test_required,
gate_affected`, and the list is sorted so two runs over the same tree are byte-identical. Kinds:

| kind | what it means |
|---|---|
| `stale-registry-test` | `required_tests` names a function that exists nowhere |
| `missing-registry-path` | `crate_or_service` is not in the tree |
| `stale-proof-report` | `proof_report` points at nothing |
| `unregistered-pallet` | a `pallets/*` crate `runtime/src/lib.rs` never mentions |
| `pallet-call-without-weights` | an extrinsic charging a literal or pre-benchmark weight |
| `ungated-crate` | a crate with tests that no gate runs |

It deliberately does not re-report `TODO`/stub markers or reachable panics: those belong to
`scripts/x3_fake_code_scan.py` and `scripts/audit/panic_unwrap_scan.py`, whose verdicts it folds into
its report instead of duplicating their debt with a second ratchet.

`--patches` only proposes; nothing is applied to the tree. Today the only patch-eligible class is
`ungated-crate`, whose fix is one gate line, and the diff is computed from the file's real bytes (the
test asserts the hunk's context lines match, so the patch applies).

The ratchet lives in `docs/reports/repo-scan-baseline.json`: growth fails the `repo scanner` gate,
shrinkage is always allowed. Baseline the debt honestly, then burn it down.

## `approve_task.sh report`

Run:

```bash
scripts/swarm/approve_task.sh report reports/swarm_task_summary.md
```

This command fetches current swarm tasks from the API and writes a markdown task summary to `reports/swarm_task_summary.md`.
