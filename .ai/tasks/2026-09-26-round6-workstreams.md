# Round 6 — 2026-09-26 — the state a chain can actually see, and the wiring nobody ran

Read this file if you were spawned and received no task text (that has happened for every agent in
rounds 3-5). Take the first workstream below that no other agent has claimed, and say in your first
output which one you took.

## Verified facts (measured, do not re-derive)

* `crates/x3-vm` interprets `evm_sstore`/`evm_sload` (0xB4/0xB3) as of `12feeb1ee`: exact
  store/load round trip per `Value` kind, slot keyspace disjoint from globals, typed refusals,
  verifier-matching gas (200/5000), and a store lands in the journal that a reverted atomic window
  does not. `cargo test -p x3-vm` -> 165 passed.
* `pallets/private-execution` runs an ordering window on chain (`9ba1f2c20`), 34 tests passed.
  Independently verified by the primary agent: `cargo check -p x3-chain-runtime --features std` ->
  exit 0, `--live --only local-node-smoke` -> PASS, `--only local-network-smoke` -> PASS at
  `0d903e8b1`.
* Master is green on the matrix family at `d8ededd2c` (`feature matrix check`, `audit matrix
  freshness`, `matrix tests exist`, `matrix test evidence`, `readiness consistency`).
* `crates/x3-integration/src/mini_x3.rs` is dirty under another agent; do not touch it.

## Process rule added this round (a real incident)

Commit `d85f79c85` was made with a broad `git add`, so it swept the primary agent's *regenerated*
`docs/audit/*` and `audit-artifacts/current/feature-status.json` while the source row those
artifacts describe was still uncommitted. That commit is red on `audit matrix freshness`; the tip is
green because the source landed in the next commit. **`git add` only your own paths, and never
`git add -A`.** Generated artifacts (`docs/audit/*`, `audit-artifacts/current/*`) are regenerated
and committed by the primary agent, because they are derived from every row at once.

## Ground rules (unchanged)

1. Own only your workstream's files; re-check `git status` and `git log --oneline -5` before editing.
2. No fake green: no weakening, deleting, skipping or `#[ignore]`ing a test; reproduce, fix, then
   show the test red with the fix removed (break-it-first control).
3. Fail closed. Unknown -> refuse, never -> success (`AGENTS.md` §5, §17-§20).
4. Do not edit `feature-matrix/*.toml`, `FEATURE_REGISTRY.toml`, `scripts/local-ci.sh`,
   `reports/rc6/*`, `reports/panic_unwrap_audit.md`, `FETCH_HEAD`, `libproto_lib/`.
5. No multi-node or fixed-port gate runs (unit/integration only); the primary agent runs the serial
   gates.
6. Everything multi-node runs *through* the gate list, e.g.
   `bash scripts/local-ci.sh --live --only 'local-node-smoke'`, so it is repeatable.
7. Report in the shape the round-5 file asks for, with real command output, and end with the row
   delta as text.

---

## Workstream A — X3VM contract storage has no channel into chain state (TICKET-147, P0)

This is the highest-value open item in the tree, because two headline objectives depend on it:
"`.x3` -> compile -> X3VM -> finalized block -> receipt" and "supply invariants proven under
distributed traffic" both need a state transition the chain can see.

`evm_sstore` writes a slot and journals it (`crates/x3-vm/src/vm.rs`, `drain_storage_journal`), and
the journal has no caller. `X3Executor::execute` (`crates/x3-integration/src/executor.rs`) returns
`state_changes: vec![]`, and the receipt's `state_changes` channel is **balance-shaped**:
`pallets/x3-kernel`'s `apply_canonical_ledger_update_v2` decodes every entry as (address -> account,
key -> asset id, value -> balance) and counts anything else in `DecodeFailureCount`. So the fix is
not "fill that field in" — it is a typed channel.

Deliverable: a versioned, typed X3VM storage-write channel, end to end.

* Extend the integration receipt and the pallet's `ExecutionReceipt` with the drained writes
  (`key: H256`, old/new optional 32-byte value), bump `EXECUTION_RECEIPT_VERSION`, and update every
  constructor the compiler finds — `pallets/x3-kernel/src/lib.rs` and `adapters.rs`, plus the mock
  adapters, which must report empty rather than fabricate.
* `X3VmAdapter::execute` maps the drained journal into it; a *failed* execution reports no writes
  (fail closed: a partial write from a reverted execution must never be applied).
* The kernel must apply them to a real storage map (or refuse them by name) — not to the balance
  decoder, and `DecodeFailureCount` must stay 0 for a storage-writing comit.
* Tests: a store visible after the receipt is stored, a reverted window visible as *no* change, and
  a balance-shaped change still decoding as it does today (the existing behaviour must not regress).
* Versioning: this changes a stored receipt's encoding. Note it in the row and in the release notes;
  no chain is live, so a version bump plus the constructor sweep is the whole migration.

Own: `crates/x3-integration/src/executor.rs`, `crates/x3-integration/src/types.rs`,
`pallets/x3-kernel/src/{lib.rs,adapters.rs}` and the tests for those. Do **not** touch
`crates/x3-integration/src/mini_x3.rs`.

## Workstream B — the ordering window on a live node, and a beacon nobody can grind

Half of `X3-MEV-006`/`X3-MEV-008` is now wired (`pallets/private-execution`), but no window has been
opened on a chain: the 22 tests drive the pallet's mock, and nothing supplies a beacon, so a
participant that can grind its own nonce chooses where its own commitment lands.

Deliverable: (1) a script under `scripts/` that opens a window on a locally booted chain, commits
from two or three funded dev accounts, reveals, settles, and prints the settled order read back from
storage — runnable as a new slug in the gate list, and (2) a beacon source the caller cannot choose
(parent block hash or a VRF), with a test that a commit-then-reveal cannot predict it.

Own: `pallets/private-execution/src/**`, `crates/x3-order-window/**` only if you add a
`no_std`-safe accessor, plus your new script. Report the gate slug to the primary agent, who adds it
to `scripts/local-ci.sh` (which you must not edit).

## Workstream C — `crates/x3-swap-router` still has no caller (P1, X3-MEV-008)

`cargo tree -i x3-swap-router --workspace` lists the crate alone, so the lane orders nothing on any
chain. The pallet now stores settled canonical orders; the router should be the thing that consumes
them, or it should be deleted in favour of the pallet path. Decide from the code which of those is
right and say why. Own: `crates/x3-swap-router/**`.

## Workstream D — economic replay and capability versions (P0, `X3-LANG-002`, `X3-LANG-005`)

Both rows are 30% mainnet_ready for one reason: the artifact binds a chain/protocol version and a
receipt records an economic result, but nothing refuses a *replay* of a receipt and nothing refuses
an artifact compiled for a different chain or capability set at execution time.

Deliverable: a replay guard with a named refusal and a capability/chain-version check in the
execution path, each with a break-it-first control. Own: `x3-lang/compiler/**`, `x3-lang/vm/**`.

## Not in scope this round

* `X3-GPU-001` (hardware) and any GPU measurement; the CPU-reference/parity work is round 5's
  workstream C.
* The seven physical servers, the 72-hour soak, public testnet hosting and the live runtime upgrade:
  external blockers, unchanged.
