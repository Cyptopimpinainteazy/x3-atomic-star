# Round 7 — 2026-09-26 — the chain can now persist a slot; it still cannot read one

Spawn payloads keep failing, so read this file. Take the first workstream that no other agent has
claimed and say in your first output which one you took.

## Verified facts (measured this round — do not re-derive)

* **`mini_x3` is the interpreter a block runs.** `pallets/x3-kernel`'s `WasmX3Adapter` ->
  `X3Executor::execute` (the `#[cfg(not(feature = "std"))]` arm) -> `mini_x3::execute_x3bc`. The
  `std` VM in `crates/x3-vm` is reached only by tools and tests. Any fabricated value in `mini_x3`
  is a fabricated value in consensus.
* **Slot storage exists on both engines and they name the same slot** (`6a3358041`). `mini_x3`
  implements `0xB3`/`0xB4` with the same key derivation (EVM domain tag in the first 8 bytes, slot
  little-endian in the next 8 — outside the `u32` global keyspace) and the same tagged payload
  (tag + length + <=30 bytes) as `crates/x3-vm`. `interpreter_agreement.rs` compares the two
  journals key byte for key byte. An oversized or `Unit` store is refused
  (`UnencodableStorageValue`), a negative or non-integer slot is refused (`InvalidStorageSlot`), and
  a payload this ISA did not write is refused (`CorruptStorageSlot`).
* **The channel from the journal to chain storage is live** (`23350ca0c`). `X3StorageWrite` on the
  receipt, `EXECUTION_RECEIPT_VERSION` 1 -> 2, kernel `X3ContractStorage` + `X3StorageUpdated`,
  `DecodeFailureCount` stays 0 because the writes never touch the balance decoder.
  `cargo test -p pallet-x3-kernel` -> 228 + 2 + 6 passed.
* **The runtime WASM build was red and is green again** (`2b2655b28`). `x3-threshold-core` declared
  `thiserror` 1.0 `default-features = false` without using it; thiserror 1.0 has no `no_std` build,
  so `wasm32v1-none` failed with `can't find crate for std` / `cannot find trait Send`. Removing the
  unused dependency fixed `cargo check -p x3-chain-runtime --features std`.
* **The chain-level X3Lang gate passes with all of the above**: `bash scripts/local-ci.sh --cross
  --only 'x3lang-across-validators'` -> PASS (343s). A real `.x3` program still compiles, executes on
  chain and reports a receipt from a second validator.
* **Composite is 65.42%** over 146 matrix rows, 120 of them under 80 (P0+core mean 65.77). The
  scoreboard only moves when a row's `implemented`/`tested`/`mainnet_ready` are backed by cited
  tests that exist.

## Workstream A — a contract cannot read its own storage (P0, the top gap)

`X3ContractStorage` is written and never read back. `X3VmAdapter::execute(payload, gas_limit)` takes
no storage, and `mini_x3::Vm` starts with an empty slot map, so a *second* comit's `evm_sload`
returns EVM's "never written" zero instead of the slot the chain persisted. Persistence is real;
contract state is not. This is also the reason the X3Lang claim is still one-way.

Deliverable: a storage view seeded from `X3ContractStorage` before `run()`, `old_value` taken from
the chain's value rather than `None`, and a two-comit proof (store in comit 1, load in comit 2)
with a control run on a chain holding no slot. Ownership note, 2026-09-26 (primary agent): the agent that landed `23350ca0c` has finished, so
workstream A is unclaimed. You own `crates/x3-integration/src/{mini_x3.rs,executor.rs}`,
`crates/x3-integration/src/types.rs`, `pallets/x3-kernel/src/{lib.rs,adapters.rs,wasm_adapters.rs}`
and the kernel's tests for this workstream. No other lane holds them.

## Workstream B — the ordering window on a live node, and the mempool ingress

The beacon landed (`fd95445d3`: the window's beacon is the chain's block hash, not the caller's
choice) and the committee crypto now lives in a `no_std` crate a pallet can link
(`63e7b5e96`). What is still missing for `X3-MEV-007`: submit -> threshold decrypt -> execute ->
receipt across a real ingress, and a script under `scripts/` that opens a window on a booted chain,
commits from funded dev accounts, reveals, settles, and prints the settled order read back from
storage. Report the gate slug to the primary agent, who adds it to `scripts/local-ci.sh`.

## Workstream C — X3-LANG-002 / X3-LANG-005 rows still at 30% mainnet_ready

The capability gates and the replay guard now have a host that can disagree (`45197e466`,
`f370ffa3a`), but neither row has an execution-path refusal with a break-it-first control. See
round 6's workstream D for the exact deliverable.

## Ground rules (unchanged, and they are load-bearing)

1. Own only your workstream's files; re-check `git status` and `git log --oneline -5` before editing.
2. `git add` your own paths only. **Never `git add -A`.** Three commits this round had to be
   reconciled because a broad add swept another agent's staged files.
3. Do not edit `feature-matrix/*.toml`, `scripts/local-ci.sh`, `reports/rc6/*`,
   `reports/panic_unwrap_audit.md`, `FETCH_HEAD`, `libproto_lib/`, or `crates/x3-integration/src/mini_x3.rs`.
   Report row deltas as text; the primary agent applies them and regenerates `docs/audit/*`.
4. Unit and integration tests only. No multi-node or fixed-port gates — they collide; the primary
   agent runs them.
5. No fake green: never weaken, delete, skip or `#[ignore]` a test. Show each fix red with the fix
   removed, then restored.
6. A dependency that is never named in the source is still built for the target. `env -u
   SKIP_WASM_BUILD cargo check -p x3-chain-runtime --features std` is the cheap proof that the
   runtime still builds, and it is the check that catches the P0 fixed in `2b2655b28`.
7. Push nothing. Commit your own files with a focused message; the primary agent verifies and pushes.

## Not in scope this round

`X3-GPU-001` and any GPU measurement (no compute device on this box), the seven physical servers,
the 72-hour soak, public testnet hosting and the live runtime upgrade: external blockers, unchanged.

### Workstream B — exact ABI, recorded 2026-09-26 by the primary agent

The lane was unreachable on this runtime until `c917917d6`: window open/commit required the
confidential-validator quorum, and `AttestationVerifier = RefuseAllAttestations` means that quorum
can never be met on a chain running this runtime. It now has its own switch, default off.

* `PrivateExecution.set_ordering_windows_enabled(origin, enabled: bool)` — `AdminOrigin =
  EnsureRootOrHalfCouncil`; `scripts/mainnet/runtime_upgrade_governance_driver.cjs` shows the council
  motion path this chain permits.
* `open_ordering_window(origin, open_block: u64, close_block: u64)`
* `commit_ordering(origin, window_id: u64, commit_hash: H256, bond: Balance)`
* `reveal_ordering(origin, window_id: u64, commit_hash: H256, plaintext: Vec<u8>, nonce: [u8;32])`
* `settle_ordering_window(origin, window_id: u64)`; `install_ordering_beacon(origin, window_id)` is
  permissionless and optional (settle derives the beacon from `BlockHash(close_block + 1)`).
* The accepted commit hash is `commitment_hash(sender_label, plaintext, nonce)` with
  `sender_label = H160(blake2_256(SCALE_encode(AccountId32))[..20])` (`blake2AsU8a` in
  `@polkadot/util-crypto`; an `AccountId32` encodes as its 32 bytes). A wrong label is refused, and
  that refusal is worth asserting once.
* Bond >= the runtime's `MinOrderingBond` (10 DOLLARS). Settle requires `now > close_block` **and**
  the beacon block's hash, so advance at least two blocks past `close_block`.
* Read back `orderingSettlements(window_id)` and `orderingWindows(window_id)`, and assert the
  settled order equals the ascending sort of `orderKey(beacon, hash)` recomputed in the driver —
  not just that the chain returned something.
