#!/usr/bin/env python3
"""Stage a validator archive and install its contents at the requested target."""
import os
from pathlib import Path, PurePosixPath
import shutil
import sys
import tarfile
import tempfile


def check_target(target):
    if target.is_symlink() or (target.exists() and
            (not target.is_dir() or any(target.iterdir()))):
        raise FileExistsError(f"restore target must be an empty directory: {target}")


def restore(archive, target):
    target = Path(os.path.abspath(target))
    check_target(target)
    with tarfile.open(archive, "r:gz") as source:
        members = source.getmembers()
        roots = set()
        entries = []
        for member in members:
            path = PurePosixPath(member.name)
            if path.is_absolute() or ".." in path.parts or not path.parts:
                raise ValueError(f"unsafe archive path: {member.name}")
            if not (member.isdir() or member.isfile()):
                raise ValueError(f"unsupported archive entry: {member.name}")
            roots.add(path.parts[0])
            relative = Path(*path.parts[1:])
            if not path.parts[1:] and not member.isdir():
                raise ValueError("archive must contain one top-level directory")
            entries.append((member, relative))
        if len(roots) != 1:
            raise ValueError("archive must contain one top-level directory")
        target.parent.mkdir(parents=True, exist_ok=True)
        staging = Path(tempfile.mkdtemp(prefix=".x3-restore-", dir=target.parent))
        try:
            for member, relative in entries:
                destination = staging / relative
                if member.isdir():
                    destination.mkdir(parents=True, exist_ok=True)
                else:
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    with source.extractfile(member) as reader, destination.open("xb") as writer:
                        shutil.copyfileobj(reader, writer)
                    os.chmod(destination, member.mode & 0o777)
            check_target(target)
            if target.exists():
                target.rmdir()
            staging.rename(target)
        finally:
            if staging.exists():
                shutil.rmtree(staging)


def main():
    try:
        restore(sys.argv[1], sys.argv[2])
    except FileExistsError as exc:
        print(f"Restore refused: {exc}", file=sys.stderr)
        return 3
    except (OSError, ValueError, tarfile.TarError, EOFError) as exc:
        print(f"Restore failed: {exc}", file=sys.stderr)
        return 4
    return 0


if __name__ == "__main__":
    sys.exit(main())
