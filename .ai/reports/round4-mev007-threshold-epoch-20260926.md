# Round 4 / Workstream A — `X3-MEV-007` encrypted mempool / threshold encryption

Date: 2026-09-26. Branch `feat/x3-prelaunch-economics-x3lang-cutover`.
Artifact under test: `crates/private-mempool` (and its one consumer,
`crates/confidential-gpu`).

## The defect the brief predicted

A partial decryption is `share_scalar * ephemeral_point`, produced under one DKG ceremony’s
polynomial. `DecryptionShare` carried `validator_index`, `share` and `proof` and **no epoch**, and
`combine_shares(shares, threshold)` took no epoch either. Two consequences, both measured here:

* shares minted under two different ceremonies could be interpolated into a Lagrange combination
  over a mixed index set — a point that belongs to neither committee — and the only symptom was an
  opaque AES-GCM tag failure several steps later;
* a caller that combined first and decrypted later had no way to say which ceremony it was
  combining, so the mismatch could not be named, only discovered.

## FIXED

* `crates/private-mempool/src/lib.rs` — `DecryptionShare` gains `dkg_epoch`, and `MempoolError`
  gains `ShareEpochMismatch { expected, got }`.
* `crates/private-mempool/src/encryption.rs` — `compute_decryption_share(share, ephemeral_pk,
  dkg_epoch)` records the ceremony; `combine_shares(shares, threshold, dkg_epoch)` refuses a share
  whose epoch is not the one being decrypted, before any interpolation; new
  `decrypt_with_shares(tx, shares, threshold)` takes the epoch from the ciphertext
  (`tx.dkg_epoch`), so the honest path has no caller-chosen epoch at all.
* `crates/confidential-gpu/src/{lib.rs,threshold.rs}` — the ripple: `ConfidentialGpuConfig` states
  a `dkg_epoch`, `DkgManager` carries it, and `combine_decryption_shares` requires it of every
  share. This is outside the lane’s own file list and is the smallest change that lets the type
  change compile; it is additive except for `DkgManager::new`, whose three call sites were updated.
* `crates/private-mempool/src/queue.rs` was **not** touched (workstream B held it).

## TESTED

```
$ cargo test -p private-mempool
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (19 before this lane)

$ cargo test -p confidential-gpu
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (9 before this lane)

$ cargo clippy -p private-mempool -p confidential-gpu --all-targets -- -D warnings   -> exit 0
$ cargo fmt --all -- --check                                                         -> exit 0
```

The adversarial set, each case over a committee freshly generated for that case (a new random
secret, so nothing can pass because two ceremonies agreed):

| Case | Test |
| --- | --- |
| `t` shares decrypt, `t-1` do not | `t_shares_decrypt_and_one_short_never_does_over_fresh_committees` — five 3-subsets of a 3-of-5 committee all yield the same plaintext; three 2-subsets fail **even when the caller declares a threshold of 2**, so no length check is doing the work |
| tampered ciphertext | `a_tampered_ciphertext_is_refused` |
| tampered nonce | `a_tampered_nonce_is_refused` |
| swapped nonce | `a_swapped_nonce_between_two_transactions_is_refused` |
| swapped ephemeral key | `a_swapped_ephemeral_key_between_two_transactions_is_refused` |
| wrong / mislabelled index, and an index outside the committee | `a_share_labelled_with_an_index_outside_the_committee_fails_closed` |
| share that is not a point | `a_share_that_is_not_a_point_is_refused` |
| duplicate index, index 0 | `combine_shares_rejects_duplicate_validator_indices`, `combine_shares_rejects_a_zero_validator_index` |
| declared threshold enforced, not trusted | `combine_shares_refuses_fewer_shares_than_the_declared_threshold` |
| share from another ceremony | `a_share_from_a_different_dkg_epoch_is_refused_with_a_typed_error` |
| the label is not the boundary | `the_epoch_label_is_not_the_security_boundary` |

**Break-it-first control.** Disabling the epoch check (`if false && share.dkg_epoch != dkg_epoch`)
and re-running:

```
test encryption::tests::a_share_from_a_different_dkg_epoch_is_refused_with_a_typed_error ... FAILED
test threshold::tests::a_share_from_another_ceremony_is_refused_by_name                   ... FAILED   (confidential-gpu)
test encryption::tests::the_epoch_label_is_not_the_security_boundary                      ... ok
```

The third line is the point: the label check is *not* the security boundary. Two different
ceremonies labelled with the same epoch still cannot open the ciphertext — the AEAD refuses — which
is why the check is a naming and early-refusal improvement, not the guarantee. Restored, both
suites are green.

## FINDINGS

* **Canonical path.** `cargo tree -i private-mempool --workspace` returns exactly one consumer:
  `crates/confidential-gpu`. Nothing in `runtime/` or `node/` depends on `confidential-gpu`, and
  the mempool type itself has no caller anywhere in the tree. The edge that does exist is library
  code, not test-only (`crates/confidential-gpu/src/threshold.rs:109,249`,
  `crates/confidential-gpu/src/lib.rs:221`). So: a real crate, one real consumer, and no path from
  either to a chain.
* **No DLEQ proof.** `compute_decryption_share` returns `proof: Vec::new()`, so a bogus partial is
  caught by the AEAD tag after interpolation — fail-closed, but the combiner learns that something
  was wrong, not which validator lied. Unchanged by this lane, and still the largest gap in the
  crate’s own invariants.
* **The crypto is real, and so is the reason it is Ristretto.** `threshold.rs` explains why
  clamped X25519 scalars would break linearity; shares and secrets are plain Ristretto scalars, so
  `t` shares combine identically whichever `t` you pick. That claim is now exercised per-case.
* The brief’s `cargo tree -i private-mempool` fails on this tree — the root manifest is a virtual
  workspace, so `-i` matches no current package. `--workspace` is required; the coordinator was
  told, and the same trap is recorded in `agent-memory.md`.

## ROW DELTA for `X3-MEV-007`

```
source        = "master"   (was "research")
paths         = ["crates/private-mempool"]   (was [])
implemented   = 45  (was 5)
tested        = 40  (was 0)
mainnet_ready = 10  (was 0)
launch_scope  = "research"  (unchanged: nothing reaches it from a chain)
required_tests = 15 names, all resolving under the row's path
test_evidence  = the two suites with counts, the control, and the reachability measurement
blockers = [
  "one consumer crate that no runtime path consumes; PrivateMempool has no caller at all",
  "no DLEQ proof on a partial decryption",
  "the DKG epoch is configuration, not a ceremony or an epoch transition exercised here",
  "unit and component level only - no live or cross-node test",
]
```

## COMMIT

`dd22bddfd` (the fix) and `ed917cb23` (the row plus the regenerated audit artifacts). Not pushed.

## REMAINING BLOCKERS

Nothing fixable-now inside the lane. What remains is structural: the family has three rows of real
code — `crates/private-mempool`, `crates/x3-swap-router/src/mev_protection`, `pallets/private-execution` —
and no path from any of them to a chain, so the honest family score stays low no matter how good
the unit tests get.

## NEXT 5 TASKS

1. Give the mempool an ingress (a `submit` extrinsic or a node mempool path) and prove one
   encrypted transaction survives submit → committee decryption → execution on a node.
2. Implement the DLEQ proof on a partial decryption and verify it in `combine_shares`, so a liar
   is named rather than merely rejected.
3. Drive `dkg_epoch` from a real DKG ceremony/epoch transition instead of configuration.
4. Add a cross-epoch adversarial case at the node level: an old committee’s shares offered after
   the transition must be refused by the running node, not only by a unit test.
5. Wire the commit-reveal ordering lane (`crates/x3-swap-router`) into the same mempool that this
   crate’s ciphertext would flow through, so ordering and encryption meet in one path.

## Completion percent for this row

**~30% by the blend (45 / 40 / 10); mainnet readiness 10%.** It means: the cryptographic core is
real, its refusals are tested adversarially over fresh committees, and a share is now bound to the
ceremony that produced it. It does **not** mean there is a private mempool on X3 — nothing on the
chain can put a transaction into this crate, and no node has ever decrypted one.
