# Round 8 — 2026-09-26 — the rows that are still 50-ish and why

Spawn payloads are unreliable, so read this file. Claim the first workstream that no other agent
has taken and say in your first output which one you took.

## Verified facts (measured this round — do not re-derive)

* `cargo test -p pallet-private-execution` -> **41 passed** and `cargo test -p x3-threshold-core` ->
  **29 passed** at `31dd2ff79`. The pallet's door now decodes the `EncryptedTransaction`, checks the
  id/hash agreement, the committee key, the DKG epoch, and stores the canonical re-encoding.
* **Nothing decrypts on chain**: no committee job collects shares, nothing consumes a pending
  record, and `commit_encrypted_state_diff` still takes an opaque diff from a signed validator.
* **The chain persists a slot and cannot read one back.** `X3ContractStorage` is written by the
  kernel and never read: `X3VmAdapter::execute(payload, gas_limit)` takes no storage, so a second
  comit's `evm_sload` sees the interpreter's empty map. Workstream A of round 7 owns that fix and is
  **in flight** (`/root/round7_storage_read`). Do not touch `crates/x3-integration/src/mini_x3.rs`,
  `executor.rs`, `types.rs` or `pallets/x3-kernel/**` this round.
* Composite is **65.76%** over 146 rows (P0+core mean **69.97**, 51 of 66 P0+core rows under 80).
  `python3 scripts/x3_audit_matrix.py` regenerates `docs/audit/**`; the scoreboard only moves when a
  row's numbers cite tests that exist.
* Two rows are the largest honest gaps in the cross-chain cluster and both are ordinary code work,
  not missing infrastructure: `X3-XCHAIN-003` (51) and `X3-XCHAIN-005` (52).

## Workstream X — a cross-chain finality claim has no certificate (`X3-XCHAIN-005`, 52)

`crates/x3-atomic-swap/src/finality.rs` (415 lines) decides finality from two caller-supplied
numbers: `verify_finality(chain, current_confirms, commitment)` and `is_finalized(chain,
FinalityCheckData { confirmations, commitment_level, .. })`. Nothing binds those to a block: the
caller says "12 confirmations" and the oracle answers `Ok(true)`. `FinalityCheckData.block_height`
is not even read. A swap's safety rests on a value the counterparty types in.

Deliverable:

* a `FinalityCertificate` that carries at least `{ chain, block_height, block_hash, tx_id,
  confirmations, observed_at }` and cannot be constructed with a confirmations count that does not
  agree with its own `block_height`/anchor (`CertificateConfirmationsDisagree` and friends);
* `verify_finality`/`is_finalized` take the certificate (not bare integers) and refuse a certificate
  whose chain does not match the one asked about, whose anchor is older than the tallest anchor the
  oracle has already accepted for that chain (a rollback/rewind must be refused, not accepted as
  fresh), or which is stale for a configured window;
* the bare-count entry points either disappear or are documented as non-production and are refused
  on the swap path;
* tests: the golden path (a certificate at exactly the required depth passes), and the ugly path
  (count inflated to match but a shorter block height — must fail; a certificate for the wrong chain;
  a rewound anchor; a stale certificate). Show at least one test red with the new check removed.

Own `crates/x3-atomic-swap/src/**` for this workstream.

## Workstream Y — the relayer signs once and cannot submit (`X3-XCHAIN-003`, 51)

`crates/x3-relayer/src/submitter.rs:78` sets `svm_required_signatures: 1` with the comment that
quorum enforcement "belongs at the aggregator layer" — and no aggregator exists in this workspace.
`crates/x3-relayer/src/relayer.rs:781` calls `attestations.has_quorum(proof.required_signatures)`
where `required_signatures` is that 1. The same file's `main.rs`/`relayer.rs` comments record that
submission fails `NotAuthorized` because the settlement engine accepts a proof only from the intent's
maker or taker, so the authority path is undecided.

Deliverable:

* `required_signatures` derived from the configured validator set (a supermajority rule, one
  definition shared with `crates/x3-validator-attestation`), never a literal 1, and a relayer that
  refuses to produce a proof it cannot back with that many *distinct* signers (a repeated signer
  must not count twice — `relayer.rs:765` warns about exactly this);
* the submission authority decision written down and enforced: either the relayer set is an
  authorized submitter (with the chain-side change that admits it, by name) or the relayer refuses
  with a typed error instead of producing a submission the chain will reject;
* tests: an under-quorum proof is refused by name, a repeated signer does not satisfy the quorum,
  and the authority path is exercised rather than described. Show the under-quorum test red with the
  bound removed.

Own `crates/x3-relayer/**`, `crates/x3-validator-attestation/**` for this workstream.

## Ground rules (unchanged, and load-bearing)

1. Own only your workstream's files; re-check `git status` and `git log --oneline -5` before editing.
2. `git add` your own paths only. **Never `git add -A`.** Broad adds have swept other agents' staged
   files twice this session.
3. Do not edit `feature-matrix/*.toml`, `scripts/local-ci.sh`, `reports/rc6/*`,
   `reports/panic_unwrap_audit.md`, `FETCH_HEAD`, `libproto_lib/`, `docs/audit/**`, or anything
   workstream A owns (listed above). Report row deltas as text; the primary agent applies them and
   regenerates `docs/audit/**`.
4. Unit and integration tests only. No multi-node or fixed-port gates — they collide; the primary
   agent runs those.
5. No fake green: never weaken, delete, skip or `#[ignore]` a test. Show each fix red with the fix
   removed, then restored. If a claim is not proven, say so in the row text instead of scoring it.
6. Commit your own files with a focused message and report the hash. Do not push.

## Not in scope this round

GPU measurement (no device on this box), the seven physical servers, the 72-hour soak, public
testnet hosting, the live runtime upgrade: external blockers, unchanged.

## Workstream Z — a compiled `.x3` program cannot persist state (unclaimed, highest value left)

Not dispatched this round: the agent thread limit was full when the brief was written, so it is
recorded here with the facts already verified so the next agent does not re-derive them.

The chain's `.x3` path is `crates/x3-integration/src/compiler_bridge.rs::compile_source` ->
`x3-compiler::Compiler::compile` -> `x3_parser` -> `x3_hir` -> `x3_mir` -> `x3_opt` -> `x3_backend`
-> X3BC, and X3BC is executed on a block by `crates/x3-integration/src/mini_x3.rs`. Both interpreters
implement `evm_sstore` (0xB4) and `evm_sload` (0xB3), both now take the chain's slots as an inherited
view (`19445a914`), and `crates/x3-backend/src/emit.rs` already has `emit_evm_sload` /
`emit_evm_sstore`. What is missing is every stage between the source and the emitter:

* `crates/x3-parser` / `crates/x3-ast` have no node for a VM intrinsic (grep for
  `VmIntrinsic|sstore|sload` there returns nothing);
* `crates/x3-hir/src/hir.rs` has `HirExprKind::VmIntrinsic` with `VmIntrinsic::EvmSload`/`EvmSstore`,
  and nothing produces it;
* `crates/x3-mir/src/lower.rs:451` swallows it (`emit_assignment(Literal(Unit))`);
* `crates/x3-backend/src/lower.rs:604` returns `BackendErrorKind::NotImplemented`.

Deliverable: a syntax a user can write in `.x3` for the two opcodes, lowered through HIR and MIR to
the emitter's instructions with operand checks (two args for a store, one for a load, an integer
slot, a value a 32-byte tagged payload can carry), plus an end-to-end test that compiles **source**
and runs the emitted X3BC on both engines: store writes slot 7, and a load handed that slot as a
seed reads 7 while the same program with no seed reads 0. That is what makes the public-testnet
X3Lang claim carry state instead of a constant.
