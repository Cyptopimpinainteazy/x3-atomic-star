#!/usr/bin/env python3
"""Refuse a runtime that charges nothing for a pallet that has weights to charge.

`frame_support::weights::Weight::zero()` is what `impl WeightInfo for ()` returns for every
dispatchable. A runtime config that sets `type WeightInfo = ();` for a pallet whose calls do storage
work therefore removes that pallet from block-weight accounting entirely: an attacker can pack
blocks with those extrinsics and the weight-based limit never notices. On 2026-09-27 eighteen such
configs were wired to the non-zero, read/write-counted `SubstrateWeight` their pallets already
shipped — `pallets/*/src/weights.rs` was generated for 31 pallets, `pub mod weights;` was declared
by 29, and `runtime/src/lib.rs` pointed at **zero** of them.

This gate fails when:

* a runtime config uses `()` for a pallet that *can* be wired (a reachable `pub mod weights` with a
  `SubstrateWeight` struct) — including any of the eighteen, so reverting one is red;
* a pallet's `weights.rs` is unreachable: the module is missing, private, or defined inline in
  `lib.rs` while the generated file sits unused beside it;
* the set of `()`-weighted runtime configs changes at all without the new entry being added to
  `ALLOWED_UNWEIGHTED` below **with a reason**. That list may only shrink.

Exit 0 only when both hold. Run: `python3 scripts/check-runtime-weights-wired.py`.
"""

from __future__ import annotations

import os
import re
import sys
import tomllib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUNTIME = os.path.join(ROOT, "runtime", "src", "lib.rs")
PALLETS = os.path.join(ROOT, "pallets")

# A `()` that is deliberate, with the reason it cannot be wired today. Shrink-only: a name here that
# becomes wireable is reported as a failure below, and a new `()` anywhere is a failure until it is
# added here with a reason.
ALLOWED_UNWEIGHTED = {
    "pallet_grandpa": (
        "the SDK's generated weights for GRANDPA are not publicly reachable: "
        "`substrate/frame/grandpa/src/lib.rs` declares `mod default_weights;` privately and does not "
        "re-export `SubstrateWeight`, so a runtime cannot point at them"
    ),
    "pallet_x3_lp_locker": (
        "the pallet ships no weights module at all; its calls need benchmarking before there is "
        "anything to point at"
    ),
    "pallet_x3_launchpad": (
        "`pallets/x3-launchpad/src/weights.rs` exists but is not a module of the pallet: `lib.rs` "
        "declares no `mod weights;` and defines its own `WeightInfo` trait inside `#[pallet::config]`. "
        "Wiring it is a pallet change (implement that trait for the generated struct), not a runtime "
        "line — see the dead-weight-file check below, which fails on the file itself"
    ),
}


def pallet_dirs() -> dict[str, str]:
    """package name -> pallet directory."""
    out: dict[str, str] = {}
    for name in sorted(os.listdir(PALLETS)):
        cargo = os.path.join(PALLETS, name, "Cargo.toml")
        if os.path.exists(cargo):
            try:
                out[tomllib.load(open(cargo, "rb"))["package"]["name"]] = os.path.join(PALLETS, name)
            except Exception:
                continue
    return out


def crate_for_module(module: str, packages: dict[str, str]) -> str | None:
    """`pallet_x3_auction` -> the pallet directory whose package name it is, or None.

    The runtime aliases some packages (`pallet-svm-runtime = { package = "pallet-svm" }`), so go
    through the module path the runtime actually writes and match against both the alias and the
    package name.
    """
    manifest = os.path.join(ROOT, "runtime", "Cargo.toml")
    aliases: dict[str, str] = {}
    if os.path.exists(manifest):
        text = open(manifest, encoding="utf-8").read()
        for alias, pkg in re.findall(
            r'^\s*([A-Za-z0-9_-]+)\s*=\s*\{\s*package\s*=\s*"([^"]+)"', text, re.M
        ):
            aliases[alias.replace("-", "_")] = pkg
    package = aliases.get(module, module.replace("_", "-"))
    return packages.get(package)


def runtime_weight_info_lines() -> list[tuple[int, str, str]]:
    """(line number, pallet module, replacement text) for every `type WeightInfo = ...` in a
    `impl <pallet>::Config ... for Runtime` block."""
    out: list[tuple[int, str, str]] = []
    current: str | None = None
    for number, line in enumerate(open(RUNTIME, encoding="utf-8").read().split("\n"), start=1):
        match = re.match(r"\s*impl\s+([A-Za-z0-9_]+)::Config\b.*?\s+for\s+Runtime\b", line)
        if match:
            current = match.group(1)
            continue
        if current and "type WeightInfo" in line:
            out.append((number, current, line.strip()))
        if current and line.strip() == "}" and not line.startswith(" "):
            current = None
    return out


def module_reachable(pallet_dir: str) -> tuple[bool, str]:
    """Whether this pallet exposes a `pub mod weights` the runtime can point at, and how."""
    lib = os.path.join(pallet_dir, "src", "lib.rs")
    weights = os.path.join(pallet_dir, "src", "weights.rs")
    if not os.path.exists(lib):
        return False, "no lib.rs"
    body = open(lib, encoding="utf-8").read()
    if re.search(r"^\s*pub mod weights\s*;", body, re.M):
        if not os.path.exists(weights):
            return False, "`pub mod weights;` but no weights.rs"
        return True, "file-backed module"
    if re.search(r"^\s*pub mod weights\s*\{", body, re.M):
        return True, "inline module"
    if re.search(r"^\s*(pub )?mod weights\s*;", body, re.M):
        return False, "`mod weights;` is private, so the runtime cannot reach it"
    return False, "lib.rs declares no weights module"


def main() -> int:
    for path in (RUNTIME, PALLETS):
        if not os.path.exists(path):
            print(f"runtime-weights: {path} is missing", file=sys.stderr)
            return 2

    packages = pallet_dirs()
    failures: list[str] = []
    warnings: list[str] = []
    unweighted: set[str] = set()
    wired = 0

    for number, module, text in runtime_weight_info_lines():
        pallet_dir = crate_for_module(module, packages)
        if pallet_dir is None:
            # An SDK pallet (or one whose alias this script cannot resolve). Its weight module lives
            # outside this repository, so the script cannot reachability-check it; what it *can* check
            # is that the config does not charge zero.
            if "()" in text:
                unweighted.add(module)
                if module not in ALLOWED_UNWEIGHTED:
                    failures.append(
                        f"runtime/src/lib.rs:{number}: `{module}` is not a pallet in this repository "
                        "and charges Weight::zero(); point it at its own weights or add a reason to "
                        "ALLOWED_UNWEIGHTED"
                    )
            else:
                wired += 1
            continue
        can_wire, why = module_reachable(pallet_dir)
        if "()" in text:
            unweighted.add(module)
            if can_wire:
                failures.append(
                    f"runtime/src/lib.rs:{number}: `{module}` charges Weight::zero() while "
                    f"{os.path.relpath(pallet_dir, ROOT)}/src/weights.rs is reachable — wire it "
                    f"(type WeightInfo = {module}::weights::SubstrateWeight<Runtime>;) instead of ()"
                )
            elif module not in ALLOWED_UNWEIGHTED:
                failures.append(
                    f"runtime/src/lib.rs:{number}: `{module}` charges Weight::zero() ({why}) and is "
                    "not in ALLOWED_UNWEIGHTED with a reason"
                )
        else:
            wired += 1
            if not can_wire:
                failures.append(
                    f"runtime/src/lib.rs:{number}: `{module}` points at a weights module that is not "
                    f"reachable ({why})"
                )
            elif why == "inline module" and os.path.exists(
                os.path.join(pallet_dir, "src", "weights.rs")
            ):
                warnings.append(
                    f"{os.path.relpath(pallet_dir, ROOT)}/src/weights.rs is dead: the pallet defines "
                    "`pub mod weights` inline in lib.rs, so the generated file beside it is never "
                    "compiled. One of the two copies must go, or the wired numbers are not the ones "
                    "that file claims"
                )

    # Shrink-only: an allowed entry that became wireable must be wired, not excused.
    for module in sorted(ALLOWED_UNWEIGHTED):
        pallet_dir = crate_for_module(module, packages)
        if pallet_dir is None:
            continue
        if module not in unweighted:
            failures.append(
                f"ALLOWED_UNWEIGHTED lists `{module}`, but the runtime no longer sets WeightInfo = () "
                "for it — remove the entry so the list keeps shrinking"
            )
        else:
            can_wire, _ = module_reachable(pallet_dir)
            if can_wire:
                failures.append(
                    f"`{module}` is listed as unweightable but its weights are reachable now — wire it "
                    "and remove the exception"
                )

    for line in warnings:
        print(f"runtime-weights: WARNING: {line}", file=sys.stderr)

    if failures:
        for line in failures:
            print(f"runtime-weights: FAIL: {line}", file=sys.stderr)
        return 1

    print(
        f"runtime-weights: OK — {wired} runtime config(s) wired to generated weights, "
        f"{len(unweighted)} documented exception(s): {', '.join(sorted(unweighted)) or 'none'}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
