#!/usr/bin/env python3
"""Fail when a change can have altered the runtime WASM but the hash record did not move.

`scripts/mainnet_release_gate.py` stage 6b rebuilds the runtime inside the
pinned srtool image and fails when the result no longer matches
`docs/reports/runtime-wasm-hashes.json`. That check is the real proof, and it is
ten minutes into a release — so a runtime change that lands without the record
turns up as somebody else's red gate later. It has done that three times in one
day.

This is the cheap early warning: if the outgoing diff touches any package in the
runtime's dependency graph, the same change must also touch the record.

    ./scripts/update-runtime-hashes.sh   # rebuild twice, refuse unless they agree,
                                         # write the record, print the doc pairs

The dependency set comes from `cargo metadata` rather than from path prefixes,
so a change to a crate *outside* the runtime's graph (the sidecar, the mobile
SDK, a test-only crate) does not trigger it.

There are two ways a change is seen, and the second one is the load-bearing one:

* `--base` (default `origin/master`), for the pull-request shape where the
  branch under review is measured against its target.
* **the record's own `recorded_revision`**, for the shape this repository
  actually lands in. Commits here go straight to `master`, so `--base
  origin/master` and `HEAD` are the same commit and the first check compares a
  revision against itself: measured on 2026-09-26, sixty-four files in the
  runtime's dependency graph had changed since `recorded_revision 335a27d8c`
  while this gate reported nothing and only the ten-minute stage 6b could have
  caught it. A freshness check that cannot see the commits is not a freshness
  check, so the record's revision is a tripwire in its own right: anything in
  the graph that changed since the revision the record names, and the record
  itself not among them, fails.

Exit 0 → nothing to do, or the record moved with the change.
Exit 1 → the change can alter the runtime and the record did not move, or the
         record cannot be tied to a revision at all.
Exit 2 → the check could not run (no cargo, no metadata); nothing was verified.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
RECORD = "docs/reports/runtime-wasm-hashes.json"
RUNTIME_PACKAGE = "x3-chain-runtime"

# A file that cannot be linked into the wasm target cannot change the runtime,
# but the graph this script walks is a set of *directories*, so it counts every
# `.rs` file under a package the runtime depends on. That misfires on the two
# shapes below, and each misfire demands a ten-minute double `srtool` rebuild:
# measured 2026-09-27, `pallets/x3-invariants/src/tests.rs` — a file whose only
# role is `#[cfg(test)]` — was the sole reason the gate was red, while the WASM
# it demanded a re-attestation of provably could not have changed.
CRATE_ROOTS = ("lib.rs", "main.rs")

# Non-Rust files: a data file can only reach the wasm if something in the graph
# reads or embeds it. Manifests and toolchain files are build inputs by
# construction — `Cargo.toml` decides features, `Cargo.lock` decides revisions —
# and everything else is decided from the tree: a non-Rust file that no `.rs`
# file in the graph names cannot be an input to the build.
BUILD_INPUT_NAMES = frozenset(
    {
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "rust-toolchain",
        "rust-toolchain.toml",
    }
)
BUILD_INPUT_DIRS = frozenset({".cargo"})


def graph_source_texts(root: pathlib.Path, graph_dirs) -> list[tuple[pathlib.Path, str]]:
    """Every `.rs` file under the runtime's dependency-graph directories."""
    texts: list[tuple[pathlib.Path, str]] = []
    for directory in graph_dirs:
        base = root / directory
        if not base.exists():
            continue
        for path in sorted(base.rglob("*.rs")):
            try:
                texts.append((path, path.read_text(encoding="utf-8", errors="replace")))
            except OSError:
                continue
    return texts


def is_not_build_input(relative: str, root: pathlib.Path | None = None, sources=None) -> str | None:
    """Why a non-Rust file cannot reach the wasm target, or `None` if it can.

    `runtime/src/lib.rs` really does embed data files (`include_bytes!` of
    `genesis-presets/dev.json`), so a file under a graph directory is not
    automatically irrelevant. What makes a file an input is that a source names
    it: the rule below is read off the tree rather than off the file's extension,
    and `--self-test` pins both directions — a named JSON stays counted, an
    unnamed record is ignored, and a manifest is always counted.
    """
    root = ROOT if root is None else root
    path = pathlib.Path(relative)
    if path.suffix == ".rs":
        return None
    if path.name in BUILD_INPUT_NAMES:
        return None
    if any(part in BUILD_INPUT_DIRS for part in path.parts[:-1]):
        return None
    needle = path.name
    for _source, text in sources if sources is not None else graph_source_texts(root, [pathlib.Path(".")]):
        if needle in text:
            return None
    return "not a build manifest, and no source in the runtime graph names it"


def is_test_only(relative: str, root: pathlib.Path | None = None) -> str | None:
    """Why `relative` cannot reach the wasm target, or `None` if it can.

    Two shapes, both decided from the tree rather than from the file's name:

    * `<package>/tests/*.rs` is an integration-test target. Cargo never links it
      into `lib`, whatever the file contains.
    * `<package>/src/<stem>.rs` declared as `#[cfg(test)] mod <stem>;` in that
      package's `lib.rs`/`main.rs`. Only an exact `#[cfg(test)]` counts: an
      attribute like `#[cfg(any(test, feature = "runtime-benchmarks"))]` is a
      real feature build and stays in scope, which is the fail-closed direction.

    Anything else — an inline test module, a `src/tests/` directory, a `bench` or
    `example` — is left counted. The exemptions are deliberately narrow enough to
    be read off the file, and `--self-test` pins each one in both directions.
    """
    root = ROOT if root is None else root
    path = pathlib.Path(relative)
    if path.suffix != ".rs":
        return None
    directory = root / pathlib.Path(*path.parts[:-1])
    package = directory
    while package != ROOT and package != package.parent:
        if (package / "Cargo.toml").exists():
            break
        package = package.parent
    else:
        return None
    if not (package / "Cargo.toml").exists():
        return None

    try:
        inside = directory.relative_to(package)
    except ValueError:
        return None

    # `<package>/tests/*.rs` is an integration-test target. The package root is
    # located by walking up to the nearest `Cargo.toml`, so a `src/tests/`
    # module directory is not mistaken for one.
    if inside.parts[:1] == ("tests",):
        return "an integration-test target under tests/"

    # `<package>/src/<stem>.rs` declared as `#[cfg(test)] mod <stem>;`.
    if inside.parts[:1] == ("src",) and path.name not in CRATE_ROOTS:
        stem = path.stem
        for crate_root in CRATE_ROOTS:
            candidate = package / "src" / crate_root
            if not candidate.exists():
                continue
            try:
                text = candidate.read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue
            if re.search(
                rf"#\[cfg\(test\)\]\s*(?:#\[[^\]]*\]\s*)*mod\s+{re.escape(stem)}\s*;",
                text,
            ):
                return f"`#[cfg(test)] mod {stem};` in src/{crate_root}"
    return None


def run(cmd: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)


def git_files(args: list[str]) -> set[str]:
    """Paths a `git` invocation prints, or nothing if it could not run."""
    result = run(["git", *args])
    if result.returncode != 0:
        return set()
    return {line for line in result.stdout.splitlines() if line.strip()}


def worktree_files() -> set[str]:
    """Tracked modifications plus untracked, unignored files."""
    return git_files(["diff", "--name-only", "HEAD"]) | git_files(
        ["ls-files", "--others", "--exclude-standard"]
    )


def recorded_revision() -> tuple[str | None, str | None]:
    """The revision the record was built at, or a reason it cannot be used.

    Returns `(revision, None)` when the record names a commit that exists here,
    and `(None, reason)` when it does not. An unattachable record is a defect,
    not a reason to skip the check.
    """
    path = ROOT / RECORD
    if not path.exists():
        return None, f"{RECORD} does not exist, so nothing ties the runtime source to an attested hash"
    try:
        record = json.loads(path.read_text())
    except (OSError, ValueError) as exc:
        return None, f"{RECORD} is not readable JSON: {exc}"
    revision = record.get("recorded_revision")
    if not isinstance(revision, str) or not revision.strip():
        return None, f"{RECORD} carries no recorded_revision"
    resolved = run(["git", "rev-parse", "--verify", "--quiet", f"{revision}^{{commit}}"])
    if resolved.returncode != 0:
        return None, (
            f"recorded_revision {revision!r} in {RECORD} is not a commit in this "
            f"repository, so the record cannot be tied to any source"
        )
    return revision, None


def runtime_graph_dirs() -> set[pathlib.Path]:
    """Workspace directories of every package `x3-chain-runtime` depends on."""
    result = run(["cargo", "metadata", "--format-version", "1"])
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip()[:400] or "cargo metadata failed")

    metadata = json.loads(result.stdout)
    packages = {pkg["id"]: pkg for pkg in metadata["packages"]}
    root = next(
        (pkg["id"] for pkg in metadata["packages"] if pkg["name"] == RUNTIME_PACKAGE),
        None,
    )
    if root is None:
        raise RuntimeError(f"{RUNTIME_PACKAGE} is not in this workspace")

    nodes = {node["id"]: node["deps"] for node in metadata["resolve"]["nodes"]}
    seen: set[str] = set()
    stack = [root]
    while stack:
        package_id = stack.pop()
        if package_id in seen or package_id not in nodes:
            continue
        seen.add(package_id)
        stack.extend(dep["pkg"] for dep in nodes[package_id])

    dirs: set[pathlib.Path] = set()
    for package_id in seen:
        package = packages.get(package_id)
        if package is None:
            continue
        manifest = pathlib.Path(package["manifest_path"])
        try:
            dirs.add(manifest.parent.relative_to(ROOT))
        except ValueError:
            continue  # a registry or git package; not our tree
    return dirs


def self_test() -> int:
    """Pin the test-only classification in both directions, on a synthetic crate.

    The exemption exists so a change to a `#[cfg(test)]` module does not demand a
    ten-minute double `srtool` rebuild. An exemption that is too wide would let a
    real runtime change through, so every case below is asserted: three shapes
    that must be ignored, and five that must stay counted.
    """
    import tempfile

    with tempfile.TemporaryDirectory() as work:
        root = pathlib.Path(work)
        (root / "src").mkdir()
        (root / "src" / "tests").mkdir()
        (root / "tests").mkdir()
        (root / "src" / "lib.rs").write_text(
            "#[cfg(test)]\nmod tests;\n"
            '#[cfg(any(test, feature = "runtime-benchmarks"))]\nmod test_helpers;\n'
            "#[cfg(test)]\n#[allow(clippy::redundant_clone)]\nmod counted_tests;\n"
        )
        for name in ("tests.rs", "test_helpers.rs", "counted_tests.rs", "lib_impl.rs"):
            (root / "src" / name).write_text("")
        (root / "src" / "tests" / "deep.rs").write_text("")
        (root / "tests" / "live.rs").write_text("")
        (root / "Cargo.toml").write_text("")

        want_ignored = {
            "src/tests.rs",
            "src/counted_tests.rs",
            "tests/live.rs",
        }
        want_counted = {
            "src/lib.rs",
            "src/test_helpers.rs",
            "src/lib_impl.rs",
            "src/tests/deep.rs",
            "Cargo.toml",
        }
        failures = []
        for rel in sorted(want_ignored):
            reason = is_test_only(rel, root)
            if reason is None:
                failures.append(f"{rel} must be recognised as test-only")
            else:
                print(f"  ignored  {rel} — {reason}")
        for rel in sorted(want_counted):
            reason = is_test_only(rel, root)
            if reason is not None:
                failures.append(f"{rel} must stay counted, but was ignored ({reason})")
            else:
                print(f"  counted  {rel}")
        for failure in failures:
            print(f"[runtime-hash self-test] FAIL: {failure}", file=sys.stderr)
        if failures:
            return 1

        # The second exemption: non-Rust files. Same shape — read off the tree,
        # pinned in both directions.
        (root / "src" / "genesis-presets").mkdir()
        (root / "src" / "genesis-presets" / "dev.json").write_text("{}")
        (root / "src" / "records").mkdir()
        (root / "src" / "records" / "identity.baseline.json").write_text("{}")
        (root / "src" / "NOTES.md").write_text("")
        (root / "src" / "lib.rs").write_text(
            (root / "src" / "lib.rs").read_text()
            + '\nconst DEV: &[u8] = include_bytes!("genesis-presets/dev.json");\n'
        )

        non_rust_ignored = {"src/records/identity.baseline.json", "src/NOTES.md"}
        non_rust_counted = {
            "Cargo.toml",
            "src/genesis-presets/dev.json",
            "src/lib.rs",
            "src/test_helpers.rs",
        }
        for rel in sorted(non_rust_ignored):
            reason = is_not_build_input(rel, root, graph_source_texts(root, [pathlib.Path(".")]))
            if reason is None:
                failures.append(f"{rel} must be recognised as not a build input")
            else:
                print(f"  ignored  {rel} — {reason}")
        for rel in sorted(non_rust_counted):
            reason = is_not_build_input(rel, root, graph_source_texts(root, [pathlib.Path(".")]))
            if reason is not None:
                failures.append(f"{rel} must stay counted, but was ignored ({reason})")
            else:
                print(f"  counted  {rel}")
        for failure in failures:
            print(f"[runtime-hash self-test] FAIL: {failure}", file=sys.stderr)
        if failures:
            return 1
    print(
        "[runtime-hash self-test] OK — 3 test-only shape(s) ignored, "
        "5 that can reach the wasm counted; 2 non-build-input file(s) ignored, "
        "4 that can reach the wasm counted"
    )
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--base",
        default="origin/master",
        help="ref to diff against (default: origin/master)",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="check the test-only classification against a synthetic crate, then exit",
    )
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    revision, unattached = recorded_revision()

    vs_base = git_files(["diff", "--name-only", f"{args.base}...HEAD"])
    worktree = worktree_files()
    # Committed work since the revision the record names. This is the check that
    # survives the direct-to-master flow, where `--base` is `HEAD` and the first
    # two sets would compare a revision against itself.
    since_record = git_files(["diff", "--name-only", f"{revision}..HEAD"]) if revision else set()
    changed = since_record | vs_base | worktree
    described = (
        f"since the record was taken at {revision}"
        if since_record
        else f"against {args.base} and since the record at {revision}"
    )

    if unattached is not None:
        print(f"[runtime-hash] {unattached}", file=sys.stderr)
        print(
            "\n    Run ./scripts/update-runtime-hashes.sh to rebuild and re-record it.",
            file=sys.stderr,
        )
        return 1

    if not changed:
        print(
            f"[runtime-hash] nothing changed {described} and the record's revision "
            f"is current; nothing to check"
        )
        return 0

    try:
        graph_dirs = runtime_graph_dirs()
    except RuntimeError as exc:
        print(f"[runtime-hash] could not compute the runtime dependency graph: {exc}", file=sys.stderr)
        return 2

    ignored: list[tuple[str, str]] = []
    graph_sources: list[tuple[pathlib.Path, str]] | None = None

    def sources() -> list[tuple[pathlib.Path, str]]:
        nonlocal graph_sources
        if graph_sources is None:
            graph_sources = graph_source_texts(ROOT, graph_dirs)
        return graph_sources

    def in_graph(paths: set[str]) -> list[str]:
        found = []
        for path in sorted(paths):
            if not any(parent in pathlib.Path(path).parents for parent in graph_dirs):
                continue
            reason = is_test_only(path)
            if reason is None and pathlib.Path(path).suffix != ".rs":
                reason = is_not_build_input(path, ROOT, sources())
            if reason is not None:
                # Named, never silently dropped: a check that quietly ignores
                # input is the failure mode this whole script exists to catch.
                ignored.append((path, reason))
                continue
            found.append(path)
        return found

    # The record's revision is the decisive tripwire: if it was not moved, a
    # runtime change after it is stale, and touching the record's text later
    # does not fix that. Only the two change sets that a re-attestation *is*
    # allowed to accompany — the outgoing branch diff and the working tree —
    # are excused by a record edit in the same set.
    offenders = in_graph(since_record)
    excused: list[str] = []
    for files in (vs_base, worktree):
        found = in_graph(files)
        if found and RECORD in files:
            excused.extend(found)
        else:
            offenders.extend(found)

    if not offenders:
        detail = f"{len(graph_dirs)} packages"
        if ignored:
            print(
                f"[runtime-hash] {len(ignored)} changed file(s) in the graph cannot reach "
                f"the wasm target and are not counted:"
            )
            for path, reason in sorted(set(ignored)):
                print(f"    {path} — {reason}")
        if excused:
            print(
                f"[runtime-hash] {RECORD} moved with {len(excused)} runtime-graph "
                f"file(s) in the working tree or the outgoing diff — the release gate's "
                f"rebuild is what proves the new hashes ({detail})"
            )
        else:
            print(
                f"[runtime-hash] nothing in the runtime's dependency graph changed "
                f"({detail}) — nothing to do"
            )
        return 0

    print(
        f"[runtime-hash] {len(offenders)} changed file(s) {described} can alter the "
        f"runtime that mainnet governance attests to, but {RECORD} did not move:",
        file=sys.stderr,
    )
    for path in offenders[:20]:
        print(f"    {path}", file=sys.stderr)
    if len(offenders) > 20:
        print(f"    … and {len(offenders) - 20} more", file=sys.stderr)
    print(
        "\n    Run ./scripts/update-runtime-hashes.sh and commit the record with this\n"
        "    change. It builds the runtime twice, refuses to write unless the two\n"
        "    builds agree, and records the revision. If the WASM turns out to be\n"
        "    unchanged (a code path the runtime never instantiates, for example), the\n"
        "    hashes stay the same and only the revision line moves.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
