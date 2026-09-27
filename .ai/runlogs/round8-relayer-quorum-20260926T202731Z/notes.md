# Round 8 — Workstream Y — `X3-XCHAIN-003` (relayer signs once and cannot submit)

Owner: `/root/round7_storage_read/round8_relayer_quorum`
Base head: `eee53968c` (round-8 brief), branch `feat/x3-prelaunch-economics-x3lang-cutover`

## What the row said, and what changed

`crates/x3-relayer/src/submitter.rs` built every `SvmProof` with
`required_signatures: 1` — a literal, with the comment that quorum enforcement
"belongs at the aggregator layer", and no aggregator exists in this workspace.
`crates/x3-relayer/src/relayer.rs` then checked
`attestations.has_quorum(proof.required_signatures)`, i.e. against a number the
proof carried for itself. A proof with one valid signature therefore satisfied
its own one-signature claim.

Now:

* `x3_validator_attestation::supermajority_threshold(n)` (`floor(2n/3)+1`) is the
  single definition of the quorum rule, used by producer and consumer;
* `x3-relayer::quorum::AuthorizedValidatorSet` decodes the configured key set and
  derives `required_signatures` from it — malformed, wrongly sized and repeated
  keys are refused at startup rather than trimmed;
* `RpcSubmitter::acquire_svm_proof` counts *distinct authorized* signers and
  refuses (`SubmitterError::SvmQuorumUnreachable`) rather than emitting a proof
  it cannot back;
* `RelayerSafetyPipeline::evaluate_svm_proof` enforces the policy supermajority
  and refuses a proof that declares a weaker quorum than policy
  (`attestation_quorum_below_policy`);
* the submission authority decision is written down and enforced: a third-party
  relayer signature is rejected on chain with `NotAuthorized`
  (`who == intent.maker || who == intent.taker`), so both proof legs refuse with
  the typed `SubmitterError::UnauthorizedSubmitter` instead of producing a
  submission the chain will reject.

## Proof

* `green-tests.txt` — `cargo test -p x3-relayer -p x3-validator-attestation`:
  58 + 5 + 15 passed, 0 failed.
* `red-policy-check-removed.txt` — with the policy check removed (the pre-fix
  `has_quorum(proof.required_signatures)` restored), `safety_pipeline_refuses_a_one_of_three_proof_that_declares_itself_satisfied`
  fails: the one-of-three proof is *accepted* (`expect_err` panics with `()`).
* `green-policy-check-restored.txt` — same filter, check restored: 9 passed.
* `cargo clippy -p x3-relayer -p x3-validator-attestation --all-targets -- -D warnings` — clean.
* `cargo check -p x3-relayer --no-default-features` — the `types` module still
  builds for a `no_std` consumer.

## Honest limits (not fixed here)

* `RelayerService` has no production constructor in this workspace: `node` and
  `x3-gateway` depend on `x3-relayer`, but only the latter's `rest.rs` consumes
  it, and only its `types`. The lane this row scores is library-level.
* Aggregating *other* validators' signatures still has no host. With a real
  multi-validator set the submitter now refuses; that refusal is the honest
  outcome, and it is the aggregator that is missing.
* `src/main.rs` is a separate monolith with its own config and its own SVM
  submission path (`build_lock_proof` with a `X3_RELAY_PROOF_SIGNER` default of
  `//Alice`); it does not use `RpcSubmitter`/`RelayerService` and was not
  rewritten by this lane.
