#!/usr/bin/env python3
"""Verify extracted source identity and absence of unrelated source roots."""
import hashlib,json,pathlib,sys,tomllib
ROOT=pathlib.Path(__file__).resolve().parents[1]
manifest=json.loads((ROOT/"audit-artifacts/migration/source-files.json").read_text())
errors=[]
for item in manifest["files"]:
    path=ROOT/item["path"]
    if not path.is_file():
        errors.append("missing: "+item["path"])
    elif hashlib.sha256(path.read_bytes()).hexdigest()!=item["sha256"]:
        errors.append("changed: "+item["path"])
for name in ("apps","services",".rc4-runtime-upgrade-work",".toolchain","node_modules"):
    if (ROOT/name).exists(): errors.append("out-of-scope root: "+name)
workspace=tomllib.loads((ROOT/"Cargo.toml").read_text())["workspace"]
if workspace["members"]!=manifest["root_workspace_members"]:
    errors.append("root workspace membership differs from extraction")
for member in workspace["members"]:
    if not (ROOT/member/"Cargo.toml").is_file():errors.append("missing member: "+member)
print(json.dumps({"ok":not errors,"source_commit":manifest["source_commit"],
 "source_files_checked":len(manifest["files"]),"explicit_workspace_members":len(workspace["members"]),
 "errors":errors,"scope":"file identity and packaging, not runtime behavior"},indent=2))
sys.exit(bool(errors))
