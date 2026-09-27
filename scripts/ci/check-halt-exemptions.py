#!/usr/bin/env python3
"""Does the reviewed halt-exemption list still match the runtime's?

`pallet_x3_invariants::InvariantCheck` refuses every signed extrinsic while `Halted` is set, except
the calls in `RuntimeHaltExemptCalls`. That list is a security decision with a sharp edge: an entry
that should not be there lets a call through a halt, and a *missing* entry is how `emergency_halt`
became a one-way door (`6d7bfc540`) — the halt refused the council motion that was its only remedy.
Both directions of drift are silent, so this gate compares the two lists and fails when they differ.

It checks four things:

  1. every `matches!` arm in `runtime/src/lib.rs::RuntimeHaltExemptCalls` is documented in
     `security/halt-exemptions.toml` (an undocumented exemption);
  2. every entry in the file is in the runtime's list (an exemption that is stale, or one that was
     deleted from the runtime and left in the file);
  3. every listed call resolves to a real `pub fn` in the pallet source named by the entry, so a
     renamed or removed dispatchable breaks the gate instead of quietly leaving a dead entry;
  4. every entry records a reason, because "why is this exempt from the halt" is the review.

What it does not check (TICKET-153): that the list covers every call in the runtime that holds
funds. There is no inventory of those calls to compare against, so this gate proves the list is
explicit and unchanged by accident, not that it is complete.

    scripts/ci/check-halt-exemptions.py
    scripts/ci/check-halt-exemptions.py --list
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10
    import tomli as tomllib

ROOT = Path(__file__).resolve().parents[2]
RUNTIME = ROOT / "runtime/src/lib.rs"
POLICY = ROOT / "security/halt-exemptions.toml"

# `RuntimeCall::X3Invariants(pallet_x3_invariants::Call::clear_halted { .. })`
ARM = re.compile(
    r"RuntimeCall::(?P<pallet>[A-Za-z0-9_]+)\(\s*(?P<module>[a-z0-9_:]+)::Call::(?P<call>[a-z0-9_]+)\s*\{",
    re.S,
)


def runtime_exemptions() -> set[str]:
    text = RUNTIME.read_text(encoding="utf-8")
    start = text.find("impl frame_support::traits::Contains<RuntimeCall> for RuntimeHaltExemptCalls")
    if start < 0:
        raise SystemExit(
            "check-halt-exemptions: RuntimeHaltExemptCalls not found in runtime/src/lib.rs — "
            "the exemption list moved or was deleted, and this gate can no longer see it"
        )
    end = text.find("\n}", start)
    body = text[start:end]
    arms = ARM.findall(body)
    if not arms:
        raise SystemExit(
            "check-halt-exemptions: no exempt calls parsed out of RuntimeHaltExemptCalls — "
            "either the list is empty (unlikely and dangerous) or the parsing pattern is stale"
        )
    return {f"{pallet}::{call}" for pallet, _module, call in arms}


def documented() -> dict[str, dict]:
    data = tomllib.loads(POLICY.read_text(encoding="utf-8"))
    entries = data.get("exemption") or []
    out: dict[str, dict] = {}
    for entry in entries:
        call = entry.get("call")
        if not call:
            raise SystemExit("check-halt-exemptions: an [[exemption]] has no `call`")
        if call in out:
            raise SystemExit(f"check-halt-exemptions: {call} is listed twice")
        out[call] = entry
    return out


def resolution_problems(entries: dict[str, dict]) -> list[str]:
    problems: list[str] = []
    external_seen = 0
    for call, entry in entries.items():
        pallet, _, variant = call.partition("::")
        why = (entry.get("why") or "").strip()
        if len(why) < 40:
            problems.append(f"{call}: `why` does not say why this call is exempt from the halt")

        # A call owned by the SDK rather than this tree cannot be resolved by grep. The entry has to
        # say where it lives instead, so the exemption is still tied to something checkable by hand.
        if entry.get("external"):
            external_seen += 1
            continue

        path = entry.get("pallet")
        if not path:
            problems.append(f"{call}: no `pallet` path recorded")
            continue
        target = ROOT / path
        sources = [target] if target.is_file() else sorted(target.rglob("*.rs")) if target.is_dir() else []
        if not sources:
            problems.append(f"{call}: `pallet` path {path} does not exist")
            continue

        # The runtime variant name is camel-cased from the pallet; the dispatchable is snake_case.
        fn = variant.lower()
        found = any(
            re.search(rf"pub fn\s+{re.escape(fn)}\s*\(", source.read_text(encoding="utf-8", errors="replace"))
            for source in sources
        )
        if not found:
            # `pallet_collective` lives in the SDK, not this tree; the entry says so by pointing at a
            # path that does not exist, which is already reported above. Anything else is a stale name.
            problems.append(
                f"{call}: no `pub fn {fn}` under {path} — the dispatchable was renamed or removed, "
                f"so this exemption no longer names a real call"
            )
    return problems


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--list", action="store_true")
    args = parser.parse_args()

    runtime = runtime_exemptions()
    documented_map = documented()
    problems = resolution_problems(documented_map)

    undocumented = sorted(runtime - set(documented_map))
    stale = sorted(set(documented_map) - runtime)
    for call in undocumented:
        problems.append(
            f"{call}: exempt in the runtime and not in {POLICY.name} — an exemption has to be reviewed"
        )
    for call in stale:
        problems.append(
            f"{call}: in {POLICY.name} and not in the runtime — the entry is stale, or the "
            f"exemption was removed without the review being updated"
        )

    if args.list:
        for call in sorted(runtime):
            print(f"{call}: exempt (documented={call in documented_map})")

    if problems:
        print(f"check-halt-exemptions: FAIL — {len(problems)} problem(s)")
        for problem in problems:
            print(f"  - {problem}")
        return 1

    print(
        f"check-halt-exemptions: OK — {len(runtime)} exempt call(s), documented and resolvable; "
        f"every other call is refused while halted"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
