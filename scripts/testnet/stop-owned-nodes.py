#!/usr/bin/env python3
"""Stop only verified candidate nodes listed in a candidate-owned state directory."""
import argparse
import json
import os
from pathlib import Path
import re
import select
import signal
import sys

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-dir", required=True)
    parser.add_argument("--pid-dir", required=True)
    parser.add_argument("--node-bin", required=True)
    parser.add_argument("--owner", required=True)
    parser.add_argument("--claim-only", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    raw_base = Path(args.base_dir).expanduser()
    base = raw_base.resolve()
    owner = Path(args.owner).resolve()
    pid_dir = Path(args.pid_dir).expanduser().resolve()
    if raw_base.is_symlink() or base in (Path("/"), Path.home(), owner, Path("/tmp")):
        parser.error("refusing unsafe or symlinked base directory")
    if not pid_dir.is_relative_to(base):
        parser.error("PID directory must be within the candidate base directory")
    marker = base / ".x3-atomic-star-owner"
    if marker.is_symlink():
        parser.error("ownership marker must not be a symlink")
    if marker.exists():
        if marker.read_text().strip() != str(owner):
            parser.error("state directory belongs to another checkout")
    elif args.claim_only:
        if base.exists() and any(base.iterdir()):
            parser.error("refusing to claim a nonempty unowned state directory")
        base.mkdir(parents=True, exist_ok=True)
        marker.write_text(str(owner) + "\n")
    else:
        parser.error("candidate ownership marker is missing")
    if args.claim_only:
        return 0
    expected_bin = Path(args.node_bin).resolve()
    verified, skipped = [], []
    for pid_file in sorted(pid_dir.glob("node-*.pid")):
        match = re.fullmatch(r"node-([1-7])\.pid", pid_file.name)
        if not match or pid_file.is_symlink():
            parser.error("invalid or symlinked PID file")
        pid_text = pid_file.read_text().strip()
        if not re.fullmatch(r"[1-9][0-9]*", pid_text):
            parser.error("PID file must contain a positive integer")
        pid = int(pid_text)
        fd = None
        try:
            if not hasattr(os, "pidfd_open") or not hasattr(signal, "pidfd_send_signal"):
                parser.error("safe process cleanup requires Linux pidfd support")
            try:
                fd = os.pidfd_open(pid)
            except ProcessLookupError:
                skipped.append({"pid": pid, "reason": "not running"})
                continue
            process = Path("/proc") / pid_text
            try:
                executable = (process / "exe").resolve(strict=True)
                argv = (process / "cmdline").read_bytes().decode(errors="surrogateescape").rstrip("\0").split("\0")
            except FileNotFoundError:
                skipped.append({"pid": pid, "reason": "process exited"})
                continue
            if executable != expected_bin:
                skipped.append({"pid": pid, "reason": "different executable"})
                continue
            paths = []
            for index, item in enumerate(argv):
                if item == "--base-path" and index + 1 < len(argv):
                    paths.append(argv[index + 1])
                elif item.startswith("--base-path="):
                    paths.append(item.split("=", 1)[1])
            expected_base = base / ("node-" + match.group(1))
            cwd = (process / "cwd").resolve(strict=True)
            actual = [(Path(p) if Path(p).is_absolute() else cwd / p).resolve() for p in paths]
            if actual != [expected_base]:
                skipped.append({"pid": pid, "reason": "different base path"})
                continue
            verified.append(pid)
            if not args.dry_run:
                try:
                    signal.pidfd_send_signal(fd, signal.SIGTERM)
                    poller = select.poll()
                    poller.register(fd, select.POLLIN)
                    if not poller.poll(10000):
                        parser.error("candidate node did not exit; refusing to wipe live state")
                except ProcessLookupError:
                    pass
        except OSError as exc:
            parser.error("cannot verify process ownership: " + str(exc))
        finally:
            if fd is not None:
                os.close(fd)
    print(json.dumps({"dry_run": args.dry_run, "verified_candidate_pids": verified, "skipped": skipped}))
    return 0

if __name__ == "__main__":
    sys.exit(main())
