# v0.4 Internal Mainnet Path — Honest Status

> Scope: minimal, internal-only mainnet readiness. External gateway, parallel
> executor, AppZone runtime, and post-quantum work are explicitly **out of
> scope** for this milestone and remain feature-gated / paused.

## What landed in this sprint

### `crates/x3-readiness-report` (rewritten)

- Tri-state checks: `Pass` / `Fail` / `Unknown`.
  - `Unknown` blocks `overall_ready` — silence is **never** treated as success.
- Real JSON-RPC collector against a running node (`X3_NODE_RPC`, default
  `http://127.0.0.1:9944`) over `ureq` with a 1.5s timeout.
- Offline mode (`X3_READINESS_OFFLINE=1` or unreachable endpoint) emits
  `Unknown` with a reason — never a synthetic `Pass`.
- Honest gaps explicitly marked:
  - `balance_reconciliation`: **always `Unknown`** with reason
    `"requires runtime API canonical_ledger_reconcile; not yet wired"`.
    Runtime API wiring is a follow-up.
- 10 unit tests cover offline behavior, unreachable-endpoint behavior,
  tri-state propagation, and JSON round-trip.

### `crates/x3-packet-standard` (new)

IBC-ICS-04-style packet lifecycle primitives.

- `Packet { src_chain, src_port, dst_chain, dst_port, sequence,
  timeout_height, timeout_timestamp, data }`, `MAX_PAYLOAD = 64 KiB`.
- Domain-separated commitments via `blake2b256(domain_tag || SCALE(payload))`.
- `ReplayGuard` — idempotent re-receipt of the same `(stream, seq, hash)`,
  hard-fails on a different hash at the same sequence.
- `TimeoutPolicy` with height-then-timestamp evaluation; `0` means disabled
  on that axis.
- 11 unit tests including SCALE round-trip and replay matrix.

**Status**: wired into `pallet-x3-cross-vm-router` completion path.

- Router now stores packet commitment at initiation.
- Completion now recomputes and verifies commitment, evaluates timeout, and
  enforces replay-key consistency before destination credit.

### `crates/x3-ixl` (new)

Minimal Instruction-eXecution-Layer (IXL) — 8-opcode VM for atomic bundles.

- Opcodes: `Lock`, `Mint`, `Burn`, `Swap`, `Settle`, `EmitProof`, `Refund`,
  `Abort`. `MAX_BUNDLE = 64`.
- `Planner` validates the bundle as a static DAG: every `Lock` must be
  resolved by exactly one terminator (`Settle`, `Refund`, or `Burn`); slot
  reuse, zero amounts, and self-swaps are rejected.
- `Interpreter` executes the plan against a host-supplied swap function and
  emits `LedgerEffect`s for the host pallet to commit. Custody slots are
  single-asset; swap fully empties+refills. Aborts return a partial
  `Receipt` so the caller can roll back.
- `Rollback` derives the inverse `LedgerEffect`s from a partial `Receipt` —
  it **never** re-runs interpreter logic.
- `AssetKind` is a closed enum (`X3Native`, `X3Evm`, `X3Svm`) — external
  chains are out of scope.
- 12 unit tests cover planner rejection paths, interpreter happy paths,
  swap slippage, abort-with-partial-receipt, and rollback inversion.

**Status**: wired into `pallet-x3-cross-vm-router` completion path as a
deterministic execution gate.

- Router now builds a minimal IXL bundle (`EmitProof` with packet commitment),
  runs planner + interpreter, and requires proof emission in receipt/effects
  before final ledger credit.
- IXL receipt entry count is persisted per message id for auditability.

### `launch-gates/run-all-proofs.sh` (hardened)

- Portable: derives `REPO_ROOT` from the script's own path; the previous
  hardcoded `/home/lojak/...` is gone.
- `STRICT=1` by default — the only proof allowed to be soft is
  `proof-05-hazard-scan` (explicit `optional` flag). Bridge, atomic, clippy,
  fmt, settlement, and release-build proofs no longer end with `|| true`.

## What is **not** done (do not claim otherwise)

- External bridge extrinsics in `pallet-x3-cross-vm-router` remain Phase-C
  stubs. They are runtime-gated by the kill-switch and must remain disabled
  for internal-only RC.
- Runtime API `canonical_ledger_reconcile` does **not** exist; until it
  does, `balance_reconciliation` is permanently `Unknown` and therefore
  permanently blocks `overall_ready`. That is the correct behavior — but it
  also means readiness will report **NOT READY** until that runtime API
  ships.
- Router legacy `BadOrigin` fixture drift has been fixed. Full router lib
  suite now passes (`18/18`) including packet+IXL integration tests.
- Router kill-switch only protects pallet entrypoints; any off-chain relayer
  path that bypasses router dispatch must be audited separately before
  mainnet/public testnet.
- No benchmark weights for the planned router-pallet entry points that
  would consume these crates.

## Verification

- `cargo check --workspace --all-targets` — clean (warnings unchanged from baseline).
- v0.4 critical-path test surface — **48/48 passing**:
  - `x3-packet-standard`: 11 unit + 6 property = **17**
  - `x3-ixl`: 12 unit + 4 property = **16**
  - `x3-readiness-report`: 10 unit
  - `pallet-x3-cross-vm-router`: 5 scope-freeze (new)
- Router full lib suite: **18/18 passing**.
- CI gate: [.github/workflows/v04-ship-gate.yml](.github/workflows/v04-ship-gate.yml)
  runs fmt + check + clippy (deny warnings) + every test above. No
  `continue-on-error`, no `|| true`.

## Sprint additions (this round)

- **Router scope freeze**: `ExternalBridgesEnabled: bool` storage, default
  `false` at genesis. Both `register_external_root` and
  `emergency_pause_bridge` now return `Error::ExternalBridgesDisabled`
  unless governance has explicitly called the new
  `set_external_bridges_enabled(true)` Root extrinsic. 5 dedicated tests
  prove the kill-switch.
- **Property tests** under `crates/x3-packet-standard/tests/properties.rs`:
  SCALE round-trip, commitment determinism, commitment input-sensitivity,
  replay-guard idempotency, replay-guard cross-payload rejection, timeout
  monotonicity.
- **Property tests** under `crates/x3-ixl/tests/properties.rs`: planner
  determinism, Lock+Settle execution conservation, rollback restoration,
  oversize-bundle rejection.
- **Strict v0.4 CI workflow**: `.github/workflows/v04-ship-gate.yml`.

## Public testnet follow-ups

- See [PUBLIC_TESTNET_FOLLOWUP_ISSUES.md](PUBLIC_TESTNET_FOLLOWUP_ISSUES.md):
  legacy router failures, relayer bypass audit, and reconciliation runtime API.
