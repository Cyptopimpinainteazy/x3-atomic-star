#!/usr/bin/env python3
"""Bind a green release-candidate run to the genesis it promotes.

`scripts/mainnet/mainnet_rc_gate.sh` answers "did the checks pass?". Nothing answered the next
question a launch actually needs: *which* genesis artifact did a passing run approve, and can a
third party re-read that decision and check it against the bytes on disk?

`[launch_gate]`'s second blocker said exactly that — the launch decision was a human approval with
no record tying it to the deployed genesis. This writes one:

    python3 scripts/mainnet/launch_record.py build \
        --spec chain-specs/x3-mainnet-plain.json \
        --raw  chain-specs/x3-mainnet-raw.json \
        --rc-log reports/rc/mainnet_rc_gate.log \
        --expect-authorities 7 \
        --approved-by "<operator>" \
        --out reports/launch/promotion-<timestamp>.json

    python3 scripts/mainnet/launch_record.py verify --record reports/launch/promotion-....json

`build` refuses unless every input is present and consistent:

  * the plain spec parses, its `chainType` is `Live`, and its Aura authority set has exactly
    `--expect-authorities` members (a 3-authority spec cannot be promoted as a 7-validator chain);
  * the raw spec is next to it, because that is the distribution form;
  * the RC log carries the gate's own finishing marker, not just exit status;
  * an operator is named, and the working tree is clean — the record names the commit it promotes.

`verify` re-hashes every artifact the record names, re-reads the marker out of the log, and exits
non-zero on the first mismatch, so a record cannot quietly outlive the genesis it describes.
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import subprocess
import sys
from pathlib import Path

DEFAULT_RC_MARKER = "== X3 MAINNET RC GATE PASSED =="
SCHEMA = 1


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def git_state(root: Path) -> tuple[str | None, bool]:
    """(commit, dirty) for the tree, or (None, False) when this is not a git work tree."""
    try:
        commit = subprocess.run(
            ["git", "-C", str(root), "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if commit.returncode != 0:
            return None, False
        status = subprocess.run(
            ["git", "-C", str(root), "status", "--porcelain"],
            capture_output=True,
            text=True,
            timeout=30,
        )
        return commit.stdout.strip(), bool(status.stdout.strip())
    except (OSError, subprocess.SubprocessError):
        return None, False


def describe_spec(path: Path, role: str) -> dict:
    """The artifact's identity, hashed. A spec that does not parse cannot be promoted."""
    with path.open(encoding="utf-8") as handle:
        spec = json.load(handle)
    if not isinstance(spec, dict):
        raise ValueError(f"{path}: not a JSON object")
    row = {
        "role": role,
        "path": str(path),
        "sha256": sha256(path),
        "id": spec.get("id"),
        "name": spec.get("name"),
        "chain_type": spec.get("chainType"),
    }
    config = (spec.get("genesis") or {}).get("runtimeGenesis", {}).get("config")
    if isinstance(config, dict):
        row["aura_authorities"] = len((config.get("aura") or {}).get("authorities") or [])
        row["grandpa_authorities"] = len((config.get("grandpa") or {}).get("authorities") or [])
        row["bootnodes"] = len(spec.get("bootNodes") or [])
    return row


def fail(message: str) -> None:
    print(f"FAILED: {message}", file=sys.stderr)
    raise SystemExit(1)


def build(args: argparse.Namespace) -> int:
    spec = Path(args.spec)
    raw = Path(args.raw)
    rc_log = Path(args.rc_log)

    for label, path in (("spec", spec), ("raw spec", raw), ("rc log", rc_log)):
        if not path.is_file():
            fail(f"{label} not found: {path}")

    if not args.approved_by.strip():
        fail("no operator: pass --approved-by '<who approved this launch>'")

    commit, dirty = git_state(Path(args.root))
    if args.commit:
        commit = args.commit
    if commit is None:
        fail("cannot read the commit to promote (not a git work tree?); pass --commit <sha>")
    if dirty and not args.allow_dirty:
        fail("the working tree is dirty: a promotion record names bytes that may not be what is committed")

    try:
        spec_row = describe_spec(spec, "mainnet-plain")
        raw_row = describe_spec(raw, "mainnet-raw")
    except (json.JSONDecodeError, ValueError) as exc:
        fail(f"chain spec does not parse: {exc}")

    if spec_row.get("chain_type") != "Live":
        fail(f"{spec}: chainType is {spec_row.get('chain_type')!r}, not 'Live' — that is not a mainnet spec")
    if raw_row.get("chain_type") != "Live":
        fail(f"{raw}: chainType is {raw_row.get('chain_type')!r}, not 'Live' — that is not a mainnet spec")

    authorities = spec_row.get("aura_authorities")
    if authorities is None:
        fail(f"{spec}: no genesis.runtimeGenesis.config to read an authority set from")
    if authorities != args.expect_authorities:
        fail(
            f"{spec}: {authorities} Aura authorities, but this launch expects "
            f"{args.expect_authorities} — a spec cannot be promoted for a validator set it does not have"
        )
    if spec_row.get("grandpa_authorities") != authorities:
        fail(f"{spec}: Aura and GRANDPA authority counts disagree")
    if not spec_row.get("bootnodes"):
        fail(f"{spec}: no bootNodes — a promoted chain nobody can join is not a launch")

    log_text = rc_log.read_text(encoding="utf-8", errors="replace")
    if args.expect_passed not in log_text:
        fail(f"{rc_log}: no {args.expect_passed!r} in the log — a passing gate has to say so itself")

    record = {
        "schema": SCHEMA,
        "recorded_at": datetime.datetime.now(datetime.timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z"),
        "approved_by": args.approved_by.strip(),
        "commit": commit,
        "expect_authorities": args.expect_authorities,
        "rc_gate": {
            "log": str(rc_log),
            "sha256": sha256(rc_log),
            "passed_marker": args.expect_passed,
        },
        "artifacts": [spec_row, raw_row],
    }

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"wrote {out}")
    print(f"  commit            {commit}")
    print(f"  approved by       {record['approved_by']}")
    print(f"  chain             {spec_row['id']} ({spec_row['chain_type']}), {authorities} authorities")
    for artifact in record["artifacts"]:
        print(f"  {artifact['role']:<14} {artifact['sha256']}  {artifact['path']}")
    return 0


def verify(args: argparse.Namespace) -> int:
    record_path = Path(args.record)
    if not record_path.is_file():
        fail(f"no record at {record_path}")
    record = json.loads(record_path.read_text(encoding="utf-8"))
    if record.get("schema") != SCHEMA:
        fail(f"{record_path}: schema {record.get('schema')!r}, this tool writes {SCHEMA}")
    if not record.get("approved_by"):
        fail(f"{record_path}: no operator recorded")

    problems: list[str] = []
    for artifact in record.get("artifacts", []):
        path = Path(artifact["path"])
        if not path.is_file():
            problems.append(f"{path}: the record names an artifact that is gone")
            continue
        actual = sha256(path)
        if actual != artifact["sha256"]:
            problems.append(f"{path}: sha256 {actual} != recorded {artifact['sha256']}")
        if artifact.get("aura_authorities") not in (None, record.get("expect_authorities")):
            problems.append(
                f"{path}: {artifact['aura_authorities']} authorities, record expects "
                f"{record['expect_authorities']}"
            )

    gate = record.get("rc_gate", {})
    log = Path(gate.get("log", ""))
    if not log.is_file():
        problems.append(f"{log}: the RC log this record cites is gone")
    else:
        if sha256(log) != gate.get("sha256"):
            problems.append(f"{log}: the RC log has changed since the record was written")
        else:
            text = log.read_text(encoding="utf-8", errors="replace")
            if gate.get("passed_marker") not in text:
                problems.append(f"{log}: no {gate.get('passed_marker')!r} in the log any more")

    commit, _ = git_state(Path(args.root))
    if args.commit and commit and commit != args.commit:
        problems.append(f"the tree is at {commit}, not the recorded {args.commit}")

    if problems:
        for problem in problems:
            print(f"FAILED: {problem}", file=sys.stderr)
        return 1
    print(f"{record_path}: verified — {record['approved_by']} promoted {record['commit']}")
    for artifact in record.get("artifacts", []):
        print(f"  {artifact['role']:<14} {artifact['sha256']}  {artifact['path']}")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Record and verify a launch promotion")
    parser.add_argument("--root", default=".", help="repository root (default: .)")
    sub = parser.add_subparsers(dest="command", required=True)

    build_parser = sub.add_parser("build", help="write a promotion record")
    build_parser.add_argument("--spec", required=True, help="plain mainnet chain spec")
    build_parser.add_argument("--raw", required=True, help="raw mainnet chain spec")
    build_parser.add_argument("--rc-log", required=True, help="the RC gate's log")
    build_parser.add_argument("--expect-authorities", type=int, default=7)
    build_parser.add_argument("--expect-passed", default=DEFAULT_RC_MARKER)
    build_parser.add_argument("--approved-by", required=True)
    build_parser.add_argument("--commit", default="", help="override the commit (default: HEAD)")
    build_parser.add_argument("--allow-dirty", action="store_true")
    build_parser.add_argument("--out", required=True)
    build_parser.set_defaults(func=build)

    verify_parser = sub.add_parser("verify", help="re-check a record against the artifacts")
    verify_parser.add_argument("--record", required=True)
    verify_parser.add_argument("--commit", default="", help="also require the tree to be at this commit")
    verify_parser.set_defaults(func=verify)

    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
