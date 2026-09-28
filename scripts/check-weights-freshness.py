#!/usr/bin/env python3
"""Fail when a pallet's generated weights and the calls it charges have drifted apart.

`X3-GPU-003`'s second open item was that nothing re-ran the weight regeneration, so a
`weights.rs` could go stale without a gate saying so. Re-running the FRAME CLI for every pallet on
every change is not affordable — it needs a `--features runtime-benchmarks` node build per pallet —
so this checks the two things the tree can answer on its own:

* **drift** — a pallet that compiles its `src/weights.rs` must charge exactly the entries that file
  declares. An entry no call charges means the file no longer describes the pallet: a call was
  renamed or removed, or it went back to a typed literal. (The other direction — a call charging
  `T::WeightInfo::x()` with no entry — does not compile, so it needs no check.)
* **unmeasured files** — a file without the benchmark CLI's own record line
  (`THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI` plus `STEPS:`/`REPEAT:`) was not
  produced by a run. The set is recorded in `docs/reports/weights-unmeasured-baseline.json` and is
  shrink-only: a new one fails, and a baselined one that becomes measured is reported so the
  baseline can drop.

Exit 0 → no drift, no new unmeasured file. Exit 1 → drift or a new unmeasured file. Exit 2 → the
check could not run (no tree, no baseline).
"""

from __future__ import annotations

import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BASELINE = ROOT / "docs/reports/weights-unmeasured-baseline.json"

# Pallets where an entry in `weights.rs` is deliberately not charged by any call.
#
# Each of these charges the documented pre-benchmark `T::DbWeight::get().reads_writes(..)` estimate
# on the calls the entry names. That estimate is a *stated* storage-access count rather than an
# invented cost, which is why the repo scanner rates it low rather than high, and the entries stay
# in the file so the measured number is available the moment the call switches to it. The exception
# is the entry set, not the pallet: any *other* dead entry here still fails.
DRIFT_EXCEPTIONS: dict[str, str] = {
    "depin-marketplace": "accept_order/cancel_order/complete_job/deregister_provider/"
    "pause_marketplace/pause_provider charge `T::DbWeight::get().reads_writes`, the documented "
    "pre-benchmark form",
    "private-execution": "the confidential-validator and encrypted-state calls charge "
    "`T::DbWeight::get().reads_writes`, the documented pre-benchmark form",
    "svm-runtime": "allocate/assign/freeze_program/upgrade_program charge "
    "`T::DbWeight::get().reads_writes`, the documented pre-benchmark form",
    "swarm": "update_config charges `T::DbWeight::get().reads_writes`, the documented "
    "pre-benchmark form",
    "x3-settlement-engine": "create_bond/finalize_intent charge "
    "`T::DbWeight::get().reads_writes`, the documented pre-benchmark form",
}

CLI_MARKER = "THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI"
TRAIT = re.compile(r"pub trait WeightInfo \{(.*?)\n\}", re.S)
METHOD = re.compile(r"fn ([A-Za-z0-9_]+)\(")
CHARGES = re.compile(r"WeightInfo::([A-Za-z0-9_]+)\(")


def weights_files(root: pathlib.Path) -> list[pathlib.Path]:
    pallets = root / "pallets"
    if not pallets.is_dir():
        return []
    return sorted(p / "src" / "weights.rs" for p in pallets.iterdir() if (p / "src" / "weights.rs").is_file())


def measured(text: str) -> bool:
    return CLI_MARKER in text and "STEPS" in text and "REPEAT" in text


def drift(root: pathlib.Path, weights: pathlib.Path) -> list[str]:
    """Entries this pallet's calls never charge, for a file the pallet actually compiles."""
    lib = weights.parent / "lib.rs"
    if not lib.is_file() or "pub mod weights" not in lib.read_text(errors="replace"):
        return []
    match = TRAIT.search(weights.read_text(errors="replace"))
    if not match:
        return []
    entries = set(METHOD.findall(match.group(1)))
    charged = set(CHARGES.findall(lib.read_text(errors="replace")))
    return sorted(entry for entry in entries if entry not in charged)


def main() -> int:
    if not (ROOT / "pallets").is_dir():
        print("weights-freshness: could not run (no pallets/ directory)", file=sys.stderr)
        return 2
    if not BASELINE.is_file():
        print(f"weights-freshness: no baseline at {BASELINE.relative_to(ROOT)}", file=sys.stderr)
        return 2

    baseline = set(json.loads(BASELINE.read_text()).get("unmeasured", []))
    unmeasured: list[str] = []
    problems: list[str] = []

    for weights in weights_files(ROOT):
        name = weights.parents[1].name
        if not measured(weights.read_text(errors="replace")):
            unmeasured.append(name)
        dead = drift(ROOT, weights)
        if dead and name not in DRIFT_EXCEPTIONS:
            problems.append(
                f"{name}: {len(dead)} weight entr(ies) that no call charges — "
                f"{', '.join(dead[:6])}{' …' if len(dead) > 6 else ''}"
            )

    new_unmeasured = sorted(set(unmeasured) - baseline)
    for name in new_unmeasured:
        problems.append(
            f"{name}: src/weights.rs carries no benchmark CLI record line, so it is not a "
            "measurement — run scripts/run-frame-benchmarks.sh or record it in "
            "docs/reports/weights-unmeasured-baseline.json with a reason"
        )

    now_measured = sorted(baseline - set(unmeasured))
    if now_measured:
        print(
            f"weights-freshness: {len(now_measured)} baselined file(s) are measured now — drop them "
            f"from {BASELINE.relative_to(ROOT)}: {', '.join(now_measured)}"
        )

    if problems:
        print("weights-freshness: FAIL", file=sys.stderr)
        for line in problems:
            print(f"  - {line}", file=sys.stderr)
        return 1

    print(
        f"weights-freshness: OK — {len(unmeasured)} unmeasured (baseline {len(baseline)}), "
        f"{len(DRIFT_EXCEPTIONS)} documented drift exception(s), no drift elsewhere"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
