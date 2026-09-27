#!/usr/bin/env python3
"""Does the halt exemption list cover every dispatchable that can hold funds?

`pallet_x3_invariants::InvariantCheck` refuses every signed extrinsic while `Halted` is set, except
the calls in `RuntimeHaltExemptCalls` (see `scripts/ci/check-halt-exemptions.py`). That exemption
list was written by hand from the calls that were *known* to hold funds. The defect this gate exists
for is the one it cannot see: a pallet that reserves a bond, escrow or deposit whose only exit is a
dispatchable the halt refuses — and, one step worse, a reserve that is never released at all, so the
funds are unreachable even after the halt is cleared. `TICKET-153` records both.

This gate builds the inventory the other checker lacks. It parses each pallet the runtime actually
wires (`construct_runtime!` in `runtime/src/lib.rs`), finds every dispatchable that can reach a
fund-holding primitive — `reserve`, `reserve_named`, `hold`, `hold_named`, `set_lock` — including
through calls to same-file helpers, and requires the call to be classified in
`security/halt-fund-holding.toml`. A new fund-holding dispatchable therefore cannot appear without a
reviewed classification, and a classification cannot outlive the call it names.

Dispositions (recorded per call in the TOML):

  * `exempt`            — the call itself is in `RuntimeHaltExemptCalls` (usable while halted);
  * `recoverable`       — a sibling dispatchable releases the funds; the entry names it and says
                          whether that sibling is exempt (`while_halted`), so a release path that
                          only exists after the halt is cleared is recorded, not assumed;
  * `transient`         — the call releases what it holds before it returns (the checker requires the
                          call to reach a release primitive), so no funds are held once it succeeds;
  * `permanent_charge`  — there is no release path on purpose because the reserved amount is an
                          anti-spam fee, not a refundable bond; the checker requires the call to read
                          a `*Fee`-shaped constant, so the claim is not free text.

Every fund-holding dispatchable is listed in the inventory, including the `transient` ones: a call
that stops releasing in-call, or a new one that starts holding, is drift in both directions and the
gate fails either way. Everything that is not `exempt` or `recoverable` with `while_halted = true` is
an *exception* — a call whose funds the halt keeps locked until governance clears it — and the
exceptions are printed in this gate's output so the state of the world is visible, not buried.

    scripts/ci/check-halt-fund-holding.py
    scripts/ci/check-halt-fund-holding.py --list
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
INVENTORY = ROOT / "security/halt-fund-holding.toml"

# `Variant: module_path,` inside a `construct_runtime!` block. The runtime declares several variants
# (dev / testnet / mainnet-rc1); a pallet counts as wired if any of them names it.
RUNTIME_PALLET = re.compile(r"^\s{8}[A-Z][A-Za-z0-9_]*:\s*([a-z0-9_:]+)\s*,", re.M)

HOLD = re.compile(
    r"::reserve(?:_named)?\(|\.reserve(?:_named)?\(|::hold\(|\.hold\(|::hold_named\(|"
    r"\.hold_named\(|::set_lock\(|\.set_lock\("
)
RELEASE = re.compile(
    r"::unreserve\(|\.unreserve\(|::release\(|\.release\(|::remove_lock\(|\.remove_lock\(|"
    r"::repatriate_reserved\(|\.repatriate_reserved\(|::slash_reserved\(|\.slash_reserved\("
)
# A fee-shaped amount: a `*Fee` constant read through `::get()`. Used so `permanent_charge` cannot be
# a free-text escape hatch for a reserve nobody thought about.
FEE_AMOUNT = re.compile(r"\b[A-Za-z0-9_]*[Ff]ee[A-Za-z0-9_]*::get\(\)")

CALL_INDEX = re.compile(r"#\[pallet::call_index\(")
DISPATCHABLE = re.compile(r"pub fn\s+([a-z_][a-z0-9_]*)\s*[<(]")
FN_DEF = re.compile(r"\bfn\s+([a-z_][a-z0-9_]*)\s*[<(]")
FN_CALL = re.compile(r"\b([a-z_][a-z0-9_]*)\s*\(")


def strip_line_comments(text: str) -> str:
    """Drop `//` line comments so a primitive named only in prose does not count as used."""
    return re.sub(r"//[^\n]*", "", text)


def braced_body(text: str, open_brace: int) -> str:
    """Return the text from the `{` at `open_brace` through its matching `}`."""
    depth = 0
    for i in range(open_brace, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return text[open_brace : i + 1]
    return text[open_brace:]


def helper_bodies(text: str, exclude: set[str] | None = None) -> dict[str, str]:
    """Map every non-dispatchable `fn name(` in a file to its body, for one-level reachability.

    Dispatchables are excluded on purpose: a `#[pallet::weight(T::WeightInfo::other_call())]`
    attribute names a sibling dispatchable, and following that as if it were a helper would make
    every call inherit the funds logic of whichever call its weight function happens to name.
    """
    skip = exclude or set()
    out: dict[str, str] = {}
    for match in FN_DEF.finditer(text):
        if match.group(1) in skip:
            continue
        brace = text.find("{", match.end())
        if brace < 0:
            continue
        out.setdefault(match.group(1), strip_line_comments(braced_body(text, brace)))
    return out


def dispatchables(text: str) -> list[tuple[str, str]]:
    """Return `(name, body)` for every dispatchable in the `#[pallet::call]` impl block."""
    header = re.search(r"#\[pallet::call\][\s\S]*?Pallet<[^>]*>\s*\{", text)
    if not header:
        return []
    body = braced_body(text, text.rindex("{", 0, header.end()))
    out: list[tuple[str, str]] = []
    for chunk in CALL_INDEX.split(body)[1:]:
        name = DISPATCHABLE.search(chunk)
        if name:
            # Start at the `pub fn`, so the chunk's own `#[pallet::weight(...)]` attribute (which
            # names a sibling dispatchable) is not mistaken for a call the body makes.
            out.append((name.group(1), strip_line_comments(chunk[name.start() :])))
    return out


def reaches(body: str, helpers: dict[str, str], pattern: re.Pattern[str]) -> bool:
    """True if `body`, or any same-file helper it calls transitively, matches `pattern`."""
    seen: set[str] = set()
    queue = [body]
    while queue:
        current = queue.pop()
        if pattern.search(current):
            return True
        for callee in FN_CALL.findall(current):
            if callee in helpers and callee not in seen:
                seen.add(callee)
                queue.append(helpers[callee])
    return False


def crate_dirs() -> dict[str, Path]:
    """Map each runtime module name (hyphen-to-underscore crate name) to its source directory."""
    by_crate: dict[str, Path] = {}
    for manifest in ROOT.glob("pallets/*/Cargo.toml"):
        name = re.search(r'^\s*name\s*=\s*"([^"]+)"', manifest.read_text(encoding="utf-8"), re.M)
        if name:
            by_crate[name.group(1).replace("-", "_")] = manifest.parent
    return by_crate


def wired_pallets() -> dict[str, Path]:
    """Runtime-wired pallets that resolve to a local directory, keyed by module (crate) name."""
    modules = set(RUNTIME_PALLET.findall(RUNTIME.read_text(encoding="utf-8")))
    by_crate = crate_dirs()
    return {module: by_crate[module] for module in modules if module in by_crate}


def runtime_variants() -> dict[str, set[str]]:
    """Every `construct_runtime!` variant name each module is wired under."""
    out: dict[str, set[str]] = {}
    for match in RUNTIME_PALLET.finditer(RUNTIME.read_text(encoding="utf-8")):
        variant, _, module = match.group(0).strip().rstrip(",").partition(":")
        out.setdefault(module.strip(), set()).add(variant.strip())
    return out


def scan_pallet(source: Path) -> dict[str, dict[str, bool]]:
    """Fund-holding facts for each dispatchable of one pallet source file."""
    text = source.read_text(encoding="utf-8", errors="replace")
    calls = dispatchables(text)
    helpers = helper_bodies(text, exclude={name for name, _ in calls})
    out: dict[str, dict[str, bool]] = {}
    for name, body in calls:
        if reaches(body, helpers, HOLD):
            out[name] = {
                "transient": reaches(body, helpers, RELEASE),
                "fee_shaped": reaches(body, helpers, FEE_AMOUNT),
            }
    return out


def inventory() -> dict[str, dict[str, bool]]:
    """Every wired, local dispatchable that can reach a fund-holding primitive, keyed `crate/call`."""
    found: dict[str, dict[str, bool]] = {}
    for module, directory in wired_pallets().items():
        source = directory / "src/lib.rs"
        if source.is_file():
            for call, facts in scan_pallet(source).items():
                found[f"{module}/{call}"] = facts
    return found


def releases_funds(module: str, call: str) -> bool:
    """True when `call` is a dispatchable of `module` that reaches a release primitive."""
    directory = crate_dirs().get(module)
    if directory is None:
        return False
    source = directory / "src/lib.rs"
    if not source.is_file():
        return False
    text = source.read_text(encoding="utf-8", errors="replace")
    calls = dispatchables(text)
    helpers = helper_bodies(text, exclude={name for name, _ in calls})
    for name, body in calls:
        if name == call:
            return reaches(body, helpers, RELEASE)
    return False


def runtime_exempt() -> set[str]:
    """The runtime's exempt dispatchables as `PalletVariant::call`, reused from the sibling gate."""
    import importlib.util

    spec = importlib.util.spec_from_file_location(
        "check_halt_exemptions", ROOT / "scripts/ci/check-halt-exemptions.py"
    )
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module.runtime_exemptions()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--list", action="store_true")
    args = parser.parse_args()

    found = inventory()
    if not found:
        print(
            "check-halt-fund-holding: FAIL — no fund-holding dispatchables parsed out of any wired "
            "pallet; the runtime mapping or the parsing pattern has gone stale"
        )
        return 1

    entries: dict[str, dict] = {}
    for entry in tomllib.loads(INVENTORY.read_text(encoding="utf-8")).get("holding") or []:
        key = entry.get("call")
        if not key:
            print("check-halt-fund-holding: FAIL — a [[holding]] entry has no `call`")
            return 1
        if key in entries:
            print(f"check-halt-fund-holding: FAIL — {key} is listed twice")
            return 1
        entries[key] = entry

    exempt = runtime_exempt()
    variants = runtime_variants()
    problems: list[str] = []
    exceptions: list[str] = []
    while_halted_releasable = 0
    transient_calls = 0

    for key in sorted(set(found) - set(entries)):
        problems.append(
            f"{key}: holds funds and is not classified in {INVENTORY.name} — a new fund-holding "
            f"dispatchable has to be reviewed against the halt"
        )
    for key in sorted(set(entries) - set(found)):
        problems.append(
            f"{key}: classified in {INVENTORY.name} and is not a fund-holding dispatchable — "
            f"the entry is stale, or the call was renamed or removed"
        )

    for key, entry in sorted(entries.items()):
        why = (entry.get("why") or "").strip()
        if len(why) < 40:
            problems.append(f"{key}: `why` does not say why this call holds funds or how they exit")
        module = key.split("/", 1)[0]
        variant = entry.get("pallet_variant")
        if not variant:
            problems.append(f"{key}: no `pallet_variant`; the runtime variant name is required")
        elif variants.get(module) and variant not in variants[module]:
            problems.append(
                f"{key}: `pallet_variant = {variant}` is not how {module} is wired in the runtime "
                f"(expected one of {', '.join(sorted(variants[module]))})"
            )
        if key not in found:
            continue
        facts = found[key]
        disposition = entry.get("disposition")
        call = key.split("/", 1)[1]

        if disposition == "transient":
            if not facts["transient"]:
                problems.append(
                    f"{key}: claims `transient`, but reaches no release primitive — the funds are "
                    f"not provably released before the call returns"
                )
            else:
                transient_calls += 1
        elif disposition == "permanent_charge":
            if not facts["fee_shaped"]:
                problems.append(
                    f"{key}: claims `permanent_charge`, but the reserved amount is not read from a "
                    f"`*Fee` constant — a non-refundable reserve has to be a named fee"
                )
            else:
                exceptions.append(f"{key}: non-refundable fee, no release path by design")
        elif disposition == "exempt":
            if f"{entry.get('pallet_variant', '')}::{call}" not in exempt:
                problems.append(f"{key}: claims `exempt`, but it is not in RuntimeHaltExemptCalls")
            else:
                while_halted_releasable += 1
        elif disposition == "recoverable":
            sibling = (entry.get("recoverable_by") or "").strip()
            sibling_module, _, sibling_call = sibling.partition("/")
            if not sibling_module or not sibling_call:
                problems.append(f"{key}: `recoverable_by` must name a dispatchable as `<crate>/<call>`")
                continue
            if sibling_module != key.split("/", 1)[0]:
                problems.append(f"{key}: `recoverable_by` {sibling} is in a different pallet")
                continue
            if not releases_funds(sibling_module, sibling_call):
                problems.append(f"{key}: `recoverable_by` {sibling} is not a release path")
                continue
            sibling_variant = entry.get("recoverable_variant", "")
            if entry.get("while_halted"):
                if f"{sibling_variant}::{sibling_call}" not in exempt:
                    problems.append(
                        f"{key}: claims `while_halted = true`, but {sibling} is not exempt from the halt"
                    )
                else:
                    while_halted_releasable += 1
            else:
                exceptions.append(f"{key}: released by {sibling} only after the halt is cleared")
        else:
            problems.append(
                f"{key}: disposition {disposition!r} is not one of exempt / recoverable / "
                f"transient / permanent_charge"
            )

    if args.list:
        for key in sorted(found):
            mark = "classified" if key in entries else "UNCLASSIFIED"
            print(f"{key}: holds funds ({mark})")

    if problems:
        print(f"check-halt-fund-holding: FAIL — {len(problems)} problem(s)")
        for problem in problems:
            print(f"  - {problem}")
        return 1

    print(
        f"check-halt-fund-holding: OK — {len(found)} fund-holding dispatchable(s) classified; "
        f"{while_halted_releasable} releasable while halted, {transient_calls} transient, "
        f"{len(exceptions)} exception(s)"
    )
    for note in exceptions:
        print(f"  - exception: {note}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
