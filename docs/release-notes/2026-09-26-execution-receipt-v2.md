# Execution receipt v2: the X3VM slot channel — 2026-09-26

## What changed

`ExecutionReceipt` (pallet) and `X3ExecutionReceipt` (integration crate) gained a typed
`storage_writes` channel, and `EXECUTION_RECEIPT_VERSION` moved **1 → 2**.

Each entry is `{ key: H256, old_value: Option<[u8; 32]>, new_value: Option<[u8; 32]> }` — the slot
key and the state the VM journaled, not a balance. `X3Executor::execute` now drains
`VM::drain_storage_journal` into that field on success; a failed execution reports none, and a write
an atomic window rolled back was already dropped by the VM's own journal truncation.

The kernel applies the channel to a new storage map, `X3ContractStorage` (slot key -> 32-byte
value), in `submit_comit_v2`, after the balance ledger update and *outside* it. Slot writes never
enter `apply_canonical_ledger_update_v2`'s (account, asset, balance) decoder, so `DecodeFailureCount`
stays 0 for a storage-writing comit.

## Why it was needed

`evm_sstore`/`evm_sload` (0xB4/0xB3) have worked in `crates/x3-vm` since `12feeb1ee`, and
`drain_storage_journal` had no caller outside that crate's own tests. The only receipt field that
could have carried a slot — `state_changes` — is balance-shaped: the kernel decodes each entry as
(address -> account, key -> asset id, value -> balance). Pushing a slot through it would not have
persisted a byte; it would have corrupted the canonical ledger and inflated the decode-failure
counter.

## Storage-format note

This changes the encoding of a stored `ExecutionReceipt`. No chain is live, so the migration is the
version bump plus the constructor sweep — there is no stored receipt to rewrite. Chains that already
persisted v1 receipts (none, at this commit) would need to re-encode or re-execute before reading
them under the v2 decoder.

## Weight note

`submit_comit_v2`'s benchmark predates both the receipt write and the slot writes, so the call site
declares `writes(2)`. The per-slot count is bounded by the pallet's `MAX_STATE_CHANGES` (1000) and is
**not** individually priced against `DbWeight` — the same precision gap the `CanonicalLedger` update
in that function already has. Recorded as a remaining item, not claimed as covered.

## Evidence

* `cargo test -p pallet-x3-kernel` -> 228 lib tests + 2 (`x3_adapter_route`) + 6
  (`x3_storage_channel`) passed, 0 failed.
* `cargo test -p x3-x3-integration` -> all green.
* `cargo clippy -p pallet-x3-kernel --all-targets -- -D warnings` -> exit 0;
  `-p x3-x3-integration` and `-p pallet-atomic-trade-engine` -> exit 0.
* `cargo check -p pallet-x3-kernel --no-default-features` and
  `cargo check -p x3-x3-integration --no-default-features` -> exit 0 (the wasm/no_std path reports no
  writes rather than inventing them).
* `cargo check -p x3-chain-runtime --features std` -> exit 0.
* Break-it-first controls: (1) removing the journal drain in `X3Executor::execute` reddens
  `a_store_reaches_chain_storage_through_the_receipt` with `one store must produce exactly one typed
  slot write: left 0, right 1`; (2) making the kernel's apply a no-op reddens the same test with `the
  stored value must be readable from chain storage: left None`.
