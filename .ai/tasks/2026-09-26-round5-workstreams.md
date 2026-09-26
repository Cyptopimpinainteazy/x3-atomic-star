# Round 5 — 2026-09-26 — make the MEV lanes reachable, and fill the hardware-free GPU rows

You were spawned with a one-line pointer to this file because spawn messages have not been
arriving (four agents in round 4 reported "no task payload"). Read this file, take the first
workstream below that no other agent has claimed, and say in your first output which one you took.

Round 4 left three real mechanisms with no reachable consumer, and five GPU rows whose blocker is
hardware rather than code. Round 5 is about closing that gap honestly.

## Ground rules (unchanged, and they matter)

1. **Own only your workstream's files.** Other agents share this tree; HEAD moves under you.
   Re-check `git status` and `git log --oneline -5` before you edit.
2. **No fake green.** Never weaken, delete, skip or `#[ignore]` a test. Reproduce the failure
   first, fix it, then delete the fix and show the test going red (the break-it-first control).
3. **Fail closed.** Unknown -> refuse, never -> success. `AGENTS.md` §5, §17-§20.
4. **Do not edit** `feature-matrix/*.toml` (report the row delta as text; the primary agent applies
   it), `scripts/local-ci.sh`, `reports/rc6/*`, `reports/panic_unwrap_audit.md`, `FETCH_HEAD`,
   `libproto_lib/`.
5. **No multi-node or fixed-port gate runs.** Unit and integration tests only; the primary agent
   runs the serial gates.
6. `git add` **your own files** and commit with a focused message; if `index.lock` blocks you, wait
   and retry. Do **not** push and do **not** `git add -A`.
7. Report with real command output in this shape:

```
WORKSTREAM: <A|B|C>
FIXED: <files, one line each>
TESTED: <exact commands + pass/fail counts + the break-it-first output>
FINDINGS: <real vs stub vs unreachable; anything that fails open>
ROW DELTA for <ID>: implemented=? tested=? mainnet_ready=? source=? paths=[...] blockers=[...]
  required_tests=[<test fns that exist>] test_evidence=[...]
COMMIT: <sha or "left uncommitted">
REMAINING BLOCKERS: <honest>
NEXT 5 TASKS: <concrete>
Completion percent for this row: <?>% and what it means
```

---

## Workstream A — put the ordering window on the chain (X3-MEV-006, and half of X3-MEV-008)

The commit-reveal lane now lives in `crates/x3-order-window` and **builds for the runtime**:

```
cargo check -p x3-order-window --no-default-features --target wasm32-unknown-unknown  -> ok
```

Nothing on a chain calls it yet: `crates/x3-swap-router` re-exports it and only its tests use it.
`pallets/private-execution` is already wired into every runtime variant (`PrivateExecution:
pallet_private_execution` in `runtime/src/lib.rs`, four variants) and is the row `X3-MEV-006`
points at ("MEV-resistant architecture"). It already has `submit_private_transaction`,
`commit_encrypted_state_diff`, confidential-validator registration and an `Enabled` switch.

Deliverable: an ordering window the chain itself runs, using the same algorithm the tests verify.

* Add extrinsics matching the lane's real API (`OrderingWindow`, `CommitRevealLane`,
  `commitment_hash`, `order_key`, `WindowSettlement::canonical_order`) — e.g. open a window, commit
  under a bond, reveal, settle. Signature shape is yours; the semantics are not:
  * one commitment per sender per window, refused by name otherwise;
  * a reveal that does not hash to its commit is refused;
  * a reveal before open / after close is refused;
  * settle before close is refused; settle twice is refused;
  * the settled order is the canonical key order and is stored/emitted so a verifier can recompute
    it — not the arrival order. Store what a third party needs, not just an event line.
* Bonds: reserve on commit, release on reveal, and do something explicit and recorded with the
  unrevealed set (`WindowSettlement::forfeitable_bond()` is what the lane hands you). If you cannot
  move the bond safely, refuse the commit instead of taking one you will not enforce, and say so.
* Wire it into the pallet's existing guards: the `Enabled` switch and the confidential-validator
  quorum should gate window operations the same way they gate private submission. Read
  `pallets/private-execution/src/lib.rs` before adding storage; follow the file's own conventions
  (weights, `Error` variants, `Event`s, `mock.rs`, `tests.rs`).
* Tests in `pallets/private-execution/src/tests.rs` for every refusal above plus a positive case
  that settles a three-commitment window into the canonical order, with the order recomputed from
  storage and asserted equal. Break-it-first: make the settle use arrival order and show the test
  fail.

Commands: `cargo test -p pallet-x3-private-execution` (check the package name first with
`cargo metadata`), `cargo clippy -p <that package> --all-targets -- -D warnings`,
`cargo build -p x3-chain-runtime --features std` if the runtime needs a feature.

Own: `pallets/private-execution/src/**`. Do **not** touch `crates/x3-order-window` (it is the
primary agent's) — if you need an API change there, write to `/root` and use what exists.

---

## Workstream B — X3-MEV-001 cross-domain MEV protection (row 35/18/20, P0)

Blocker on record: "Needs threat model covering relayers, finality delays, and ordering". The row's
path is `crates/cross-vm-coordinator` (a standalone crate with its own workspace — check how it is
built before assuming `cargo test -p` reaches it).

Deliverable: a threat model *with code that enforces something*, not prose. At minimum: relayer
trust (who can submit a leg and what stops a relayer from choosing when), finality delay (a leg
whose source chain has not finalized must not be claimable), and ordering exposure (what an
observer learns, and whether an intent's legs can be reordered). Where the coordinator currently
assumes good behaviour, make it check; where it cannot check, fail closed and say so in the code.

Every refusal needs a named test, and at least one break-it-first control. If the honest answer is
that the coordinator cannot enforce a class of protection without a caller change, say exactly
that and leave the row where the evidence puts it — an honest 35 with a precise reason beats an
inflated 60.

Own: `crates/cross-vm-coordinator/**` (and its workspace files). Nothing else.

---

## Workstream C — the hardware-free half of the GPU rows (X3-GPU-007/008/009/016/017, each 20/10/10)

`X3-GPU-001` (finalized-TPS proof) is blocked on a physical GPU and must **not** be worked: this box
has no compute device and `GPU_VALIDATOR_HONEST_AUDIT.md` forbids the claim. But five rows score
20/10/10 for "GPU Keccak256 batching", "GPU ed25519 verification", "GPU secp256k1 verification",
"GPU Blake2b batching" and "GPU Merkle-root acceleration" — and what is missing there is not only
silicon. `crates/x3-accel` already has the CPU/GPU parity architecture (`ParityMismatch`, CPU
recomputation) at 80/70/55.

Deliverable: CPU reference implementations plus parity vectors for those five primitives, with the
accelerator path explicitly *unavailable* rather than stubbed:

* a deterministic CPU reference for each primitive over fixed vectors (keccak256, ed25519 verify,
  secp256k1 verify, blake2b, merkle root over a defined leaf encoding);
* a parity harness that compares a candidate backend's output against the reference and **fails
  closed** on disagreement (the accelerator may never be accepted merely because it returned);
* the GPU side must be reported as "no device, not run" — never as passing. If `crates/x3-accel`
  already has a device-detection seam, use it; if not, add the smallest honest one.
* tests: reference vectors are exact (not `is_ok()`), a deliberately wrong backend produces
  `ParityMismatch`, and a missing device produces "unavailable", not success.

Commands: `cargo test -p x3-accel`, `cargo clippy -p x3-accel --all-targets -- -D warnings`,
`cargo fmt --all -- --check`.

Own: `crates/x3-accel/**`. Report row deltas for each of the five rows; do not touch
`X3-GPU-001`, and do not claim any GPU measurement you did not make on a device.

---

## Not in scope for round 5

* `X3-GPU-001` — hardware-blocked (no compute device).
* The private-mempool ingress into a pallet: `crates/private-mempool` is std-only (tokio, chrono,
  parking_lot) so it cannot be linked into a runtime; the honest version of that work starts with
  an extraction like the one done for the ordering lane, and it is not assigned this round.
* Physical validators, 72-hour soak, public testnet hosting, live runtime upgrade: external
  blockers (7 servers), unchanged.
