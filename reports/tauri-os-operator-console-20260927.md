# Tauri OS operator console — the read path is real (2026-09-27)

Registry row: `tauri_os`. Lane: `.ai/tasks/2026-09-27-tauri/operator-console.md`.
Runlog: `.ai/runlogs/tauri-os-console-20260927T135044Z/`.

## What was wrong

`apps/tauri-os` had never been built or tested. Measured before this change:

* **No test attribute anywhere under `apps/tauri-os`** — nothing had ever
  executed a command.
* **The crate did not compile.** `tauri.conf.json` sat at the app root instead of
  `src-tauri/`, so `tauri-build` could not find it; once moved, the icon set the
  config names did not exist; and `pkg-config` could not resolve GTK/WebKit.
* **Commands answered questions they could not answer.** `launch_node` and
  `stop_node` returned `Ok("node_launch_requested")` / `Ok("node_stop_requested")`
  without spawning anything. `get_node_status` returned a cached struct whose
  `block_height` was never written by any code path. `swarm_get_tasks` fell back
  to a cache when the service did not answer, so a dead swarm API looked like an
  empty queue.
* **The swarm panel could never have worked.** It posted to `/approve/{id}` and
  `/reject/{id}`; `services/x3-swarm-api` serves `/tasks/{id}/approve` and
  `/tasks/{id}/reject`. It also decoded `/tasks` into
  `{id,name,status,agent,priority,created_at}` while the service sends
  `{id,title,feature,agent,permission_tier,allowed_paths,forbidden_paths,
  required_commands,status,approval_required,risk}` — so the parse failed and the
  command silently returned the empty cache.

## What changed

`main.rs` is now the desktop entry point only. The console lives in a library, so
`tests/` can link against it:

| module | what it does |
| --- | --- |
| `chain.rs` | `ChainClient` trait (`rpc_call`, `finalized_head`, `peers`, `system_health`, `node_identity`, `node_role`) + the real `RpcChainClient` |
| `swarm.rs` | `SwarmClient` trait + `HttpSwarmClient` against the service's real routes |
| `probe.rs` | `ServiceProbe` for the local lane services |
| `commands.rs` | command bodies that take the client they need, and the `#[tauri::command]` wrappers |
| `monitor.rs` | the 5-second tick; a failed chain read is emitted as a failure envelope |
| `error.rs` | typed `ServiceError` → `IpcError` (`SERVICE_UNREACHABLE` / `SERVICE_REJECTED` / `SERVICE_BAD_RESPONSE`) |

`launch_node` and `stop_node` were deleted rather than re-worded: their only
behaviour was a fabricated success string, and nothing in the frontend invoked
them. Replacing them with a real spawn belongs in a follow-up with a tracked
child handle (see the ledger at the end of this file).

Reads are the surface `scripts/local-node-smoke.sh`, `scripts/mainnet/local3_lib.sh`
and the operator CLI already use: `chain_getFinalizedHead`, `chain_getHeader`,
`system_health`, `system_name`, `system_version`, `system_chain`, and
`system_nodeRoles` (optional — a node that does not serve it reports no role
instead of being assumed to be a full node).

## Evidence

```
bash apps/tauri-os/src-tauri/run-tests.sh          # 42 unit + 6 integration tests
cargo clippy --locked --all-targets --features live-node -- -D warnings   # clean
bash apps/tauri-os/src-tauri/run-live-test.sh     # boots a dev chain, live test green
```

Live read against a real `x3-chain-node --dev` (see the runlog):

```
live console read: X3 Chain Node 0.1.0 chain=X3 Chain Development
  finalized=38 (0x7fb560edf38e52464f8c949e16f61d6e6ed462c313c43b412d4c5865da5ba2d8)
  peers=0 syncing=false role=Some("Authority") verdict=Healthy
```

Break-it-first on the fail-closed path: `RpcChainClient::rpc_call` was changed to
return a healthy-looking JSON-RPC result when the transport failed. The
unreachable-node test went red (`SERVICE_BAD_RESPONSE` where it asserts
`SERVICE_UNREACHABLE`), the file was restored byte-identically
(`sha256 dfeac8863dd6fb30ddaeb56253fa81523c03cfcea942a040ab8c21dbee12a46a`) and
went green again. See `break-it-first.log` in the runlog.

## Build repairs required to compile at all

* `tauri.conf.json` moved from the app root to `src-tauri/` (where the Tauri CLI
  and `tauri-build` look for it), which also makes its `frontendDist: "../dist"`
  resolve to `apps/tauri-os/dist` instead of `apps/dist`.
* `src-tauri/icons/` generated from the repository's own
  `apps/x3-desktop/src-tauri/icons/icon-1024.png` with `tauri icon`.
* `src-tauri/capabilities/default.json` added (`core:default`, `fs`, `http`,
  `notification`). Without a capability the webview has *no* permissions and the
  panels' `listen()` for `os:node_status` is refused. The permission identifiers
  are validated by `tauri-build`, which regenerated `gen/schemas/capabilities.json`.
* `build-pkgconfig/shared-mime-info.pc`: Debian's `shared-mime-info` ships no
  `.pc` file while `gdk-pixbuf-2.0.pc` lists it in `Requires.private`, which
  breaks every GTK/WebKit build script on this box. `run-tests.sh` puts it on
  `PKG_CONFIG_PATH` together with the system pkgconfig directory (this box's
  default path is Homebrew-only).

## Not done

* Six console domains named by the lane have no command at all: agents, compute
  providers, benchmarks, settlement, logs, alerts.
* No packaged or notarised app, and no public-testnet run of the console.
* The swarm path is proven against the service's real bodies and routes by a
  local server in the integration test; the gate does not boot `x3-swarm-api`.
* The frontend has no tests of its own; `npm run build` (tsc + vite) is the only
  check on it.
