# Lane: Tauri OS as an operator console that reads a real node

Registry row: `tauri_os`, currently **15%**, `crate_or_service = apps/tauri-os`.

## What exists

`apps/tauri-os/src-tauri/src/main.rs` holds every `#[tauri::command]` in one file (chain status,
swarm, hardware, logs, …). `apps/tauri-os/src-tauri` is its own cargo workspace with a lockfile, and
`apps/tauri-os/node_modules` is installed, so the frontend can build.

Measured 2026-09-27: **no test attribute exists anywhere under `apps/tauri-os`.** Nothing has ever
executed a command. That is why the row is at 15%.

## The mission's rule for this row

> Function first. UI polish later.

So do not touch styling. Make the console a working operator console for: validator health, swarm
tasks, agents, compute providers, hardware, benchmarks, settlement, chain status, logs, alerts — and
make that readable *headlessly*, so a gate can prove it.

## Deliverable

1. Split the command bodies out of `main.rs` into modules that take an injected client (a small
   trait, e.g. `ChainClient` with `finalized_head`, `peers`, `system_health`, `rpc_call`), leaving
   `main.rs` as wiring. A command that cannot get an answer must return a typed error, never a
   plausible-looking placeholder: check each command for hardcoded or `rand`-generated values and
   either replace them with real RPC reads or delete them.
2. Real reads against the node's HTTP/WS JSON-RPC for chain status and validator health (finalized
   height, peers, node name/role, health) — the same endpoints `scripts/local-node-smoke.sh`,
   `scripts/monitoring/local3-monitoring-check.sh` and the operator CLI already use. Reuse their
   parsing instead of inventing a second format where you can.
3. Tests that a gate runs, in `src/` `#[cfg(test)]` modules plus one integration test under
   `apps/tauri-os/src-tauri/tests/`:
   * the client-injected command layer returns typed errors when the node is unreachable (point it at
     an address nothing listens on — the assertion is on the error, not on a string);
   * one command returns real values against a live local dev node. If that needs a node, make the
     test `#[ignore]`d **for the same reason the repo's other live tests are** and add a wrapper
     script that boots one, exactly like `X3-contracts/svm/programs/x3_htlc/run-expiry-test.sh`.
4. Add both gate lines to `scripts/local-ci.sh` (the unit layer in the fast set, the live one in the
   live set) and prove them green.

## Proof required

* `cargo test --manifest-path apps/tauri-os/src-tauri/Cargo.toml` green, and the live variant green
  with the node up.
* `bash scripts/local-ci.sh --only <your-slugs>` PASS.
* Break-it-first on the fail-closed path: make the unreachable-node test's client return a fake
  success, show the test go red, restore byte-identically, show green.
* Registry row: `implemented*0.35 + tested*0.25 + mainnet_ready*0.40`, with the three numbers
  written out in the comment. `mainnet_ready` stays low without a packaged/notarised app or a public
  testnet run.
* `python3 scripts/x3_audit_matrix.py && python3 scripts/x3_audit_matrix.py --check` after the edit.

## Rules

Shared tree: never `git add -A`, carve explicit paths, do not edit `scripts/local-ci.sh` while a
local-ci run is executing. `apps/tauri-os/node_modules` exists — do not add it, and do not add
`dist/` output to git.
