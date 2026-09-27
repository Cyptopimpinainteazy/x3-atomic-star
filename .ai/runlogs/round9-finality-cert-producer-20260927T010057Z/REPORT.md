# Round 9 — Workstream F — certificate producer (X3-XCHAIN-005 / TICKET-152)

## What changed

`FinalityCertificate` was a checked *shape*: it could not invent depth for an anchor, but nothing
bound its `block_hash` to the chain in `chain`, and the oracle's tip memory died with the process.
This workstream adds the reader that builds a certificate from real chain data and makes the tip
memory durable.

* `crates/x3-atomic-swap/src/finality_producer.rs` — `EvmFinalityProducer`, an EVM reader that:
  * reads `eth_chainId` and refuses a receipt for a *different* chain before considering depth
    (`FinalityChainIdMismatch`);
  * reads the receipt, then the block at the receipt's own height, and refuses unless the node's
    block hash equals the receipt's `blockHash` (`FinalityBlockHashMismatch`) — a bogus anchor is
    refused, never repaired;
  * refuses a node that answers a height with a different block (`FinalityBlockNumberMismatch`);
  * takes `observed_at` from `eth_blockNumber` at read time, never from the caller.
  The RPC reads sit behind the `EvmChainReader` trait (implemented by the existing `RpcClient`), so
  the checks run on RPC-shaped data in tests and on a live node in the drill.
* `crates/x3-atomic-swap/src/finality.rs` — `FinalityTipStore` (trait), `FinalityTipRecord`, and
  `PersistentFinalityOracle<S>`: tips load on construction and persist on every observation. An
  *acceptance* is only reported once it is durable (a store failure on that path fails closed); a
  *refusal* still records the witnessed tip, and a store failure there does not mask the refusal.
* `crates/x3-atomic-swap/src/finality_producer.rs` — `FileFinalityTipStore`, the process-side file
  implementation (atomic temp+rename write).
* `crates/x3-atomic-swap/src/bin/x3_finality_cert.rs` — a CLI that builds and verifies one
  certificate per process, printing a JSON verdict.
* `scripts/drills/finality_cert_evm_live.sh` + `..._driver.cjs` — the live anvil drill.
* `crates/x3-atomic-swap/src/error.rs` — four new typed refusals and their `Display` arms.

## Evidence

`green-focused.txt`, `green-focused-after-restore.txt`, `green-crate-nostd.txt`,
`green-crate-std.txt`, `clippy-atomic-swap.txt` — the passing runs.

`red-01..03` — each guard removed (with the file's sha256 proven identical before and after in
`backups/sha256.{before,after}.txt`):

1. `red-01-block-hash-binding-removed.txt` — remove the `block_hash == block(number).hash` check →
   `test_a_receipt_hash_that_is_not_the_block_at_that_height_is_refused` fails.
2. `red-02-chain-id-binding-removed.txt` — remove the `eth_chainId` check →
   `test_a_receipt_from_another_chain_is_refused_for_the_chain` fails.
3. `red-03-persistence-removed.txt` — stop restoring tips on `load` →
   `test_accepted_tip_survives_a_reload_and_refuses_a_rewind` fails (`accepted_tip` reloaded as
   `None`).

`../finality-cert-live-20260927T012825Z/` — the live drill: deploy AtlasHTLC on anvil, reach 12
confirmations, settle on a certificate built from the chain, rewind the fork with
`evm_snapshot`/`evm_revert`, and a **fresh process** that reloaded the accepted tip refuses the
rewound certificate as `CertificateRewindsAcceptedAnchor`. A second chain-id is refused for the
chain. Driver report: 8/8 checks pass.

## Not proven here

* The block-hash↔height refusal is proven against RPC-shaped data, not by inducing a mid-read race
  on a live node; the live drill proves the *positive* binding (receipt hash == block hash at that
  height) plus the chain-id and rewind refusals from real data.
* Only the EVM family has a producer and a drill. Solana/X3 producers are not part of this row.
