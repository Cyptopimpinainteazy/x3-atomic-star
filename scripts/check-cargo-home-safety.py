#!/usr/bin/env python3
"""Refuse to let CI delete the shared toolchain out of `$CARGO_HOME/bin`.

This machine is both the developer workstation and a GitHub Actions
self-hosted runner (systemd unit
`actions.runner.Cyptopimpinainteazy-xxxstar.x3star1.service`), and `$HOME` is
shared between the two. `Swatinem/rust-cache`'s *save* step is only safe on an
ephemeral runner: on save it takes the binaries that existed when the action's
main step ran (`config.cargoBins`) and unlinks every one of them from the live
`$CARGO_HOME/bin` before archiving that directory, so the cache it uploads
contains only binaries the build itself produced. On a persistent HOME that
destroys the user's toolchain -- twice on 2026-09-26, `rustup`, `subkey`,
`cargo-audit`, `cargo-deny` and `srtool` all disappeared from
`/home/lojak/.cargo/bin` (the `-> rustup` shims are symlinks and survived,
which is the signature of `cleanBin`: it only unlinks `dirent.isFile()`
entries). The same save step also unlinks `$CARGO_HOME/credentials.toml` and
prunes the registry index/src/cache.

`cache-bin: false` disables that one destructive behaviour and nothing else,
so every rust-cache step in this repository must set it. This gate is the
thing that says so, because "we remembered" is not a mechanism.

Exit 0 only when every `Swatinem/rust-cache` step sets `cache-bin: false`.
"""

from __future__ import annotations

import argparse
import glob
import os
import sys

try:
    import yaml
except ImportError:  # pragma: no cover - the gate must fail closed, not skip
    print("cargo-home-safety: PyYAML is required to parse workflows", file=sys.stderr)
    sys.exit(2)

CACHE_ACTION = "Swatinem/rust-cache"


def is_false(value: object) -> bool:
    """`false` in YAML is a bool; tolerate the string form too."""
    if isinstance(value, bool):
        return value is False
    if isinstance(value, str):
        return value.strip().lower() == "false"
    return False


def iter_steps(workflow: dict):
    jobs = workflow.get("jobs") or {}
    if not isinstance(jobs, dict):
        return
    for job_name, job in jobs.items():
        if not isinstance(job, dict):
            continue
        for index, step in enumerate(job.get("steps") or []):
            if isinstance(step, dict):
                yield job_name, index, step


def check(workflows_dir: str) -> int:
    paths = sorted(
        set(glob.glob(os.path.join(workflows_dir, "*.yml")))
        | set(glob.glob(os.path.join(workflows_dir, "*.yaml")))
    )
    failures: list[str] = []
    checked = 0
    files_with_action = 0

    for path in paths:
        try:
            with open(path, encoding="utf-8") as handle:
                workflow = yaml.safe_load(handle)
        except Exception as exc:  # a workflow we cannot parse is a failure, not a skip
            failures.append(f"{path}: unparseable YAML ({exc.__class__.__name__}: {exc})")
            continue
        if not isinstance(workflow, dict):
            continue

        found_here = False
        for job_name, index, step in iter_steps(workflow):
            uses = step.get("uses")
            if not isinstance(uses, str) or not uses.startswith(CACHE_ACTION):
                continue
            found_here = True
            checked += 1
            with_block = step.get("with") or {}
            if not isinstance(with_block, dict) or not is_false(with_block.get("cache-bin")):
                failures.append(
                    f"{path}: job '{job_name}' step #{index + 1} "
                    f"({step.get('name') or uses}) must set `with: cache-bin: false` — "
                    "without it rust-cache's save step deletes $CARGO_HOME/bin on this shared HOME"
                )
        files_with_action += 1 if found_here else 0

    if failures:
        for line in failures:
            print(f"cargo-home-safety: FAIL: {line}", file=sys.stderr)
        print(
            f"cargo-home-safety: {len(failures)} of {checked} {CACHE_ACTION} step(s) are unsafe",
            file=sys.stderr,
        )
        return 1

    print(
        f"cargo-home-safety: OK — {checked} {CACHE_ACTION} step(s) across "
        f"{files_with_action} workflow file(s) set cache-bin: false (0 unsafe)"
    )
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--workflows-dir",
        default=os.path.join(".github", "workflows"),
        help="directory holding the workflow YAML files (default: .github/workflows)",
    )
    args = parser.parse_args()
    if not os.path.isdir(args.workflows_dir):
        print(f"cargo-home-safety: {args.workflows_dir} is not a directory", file=sys.stderr)
        return 2
    return check(args.workflows_dir)


if __name__ == "__main__":
    sys.exit(main())
