# Round 4 — 2026-09-26 — MEV / privacy depth

Read this file if you were spawned as an agent and received **no task text**: that has happened for
every agent in round 3 and round 4 (measured 2026-09-26 — four agents reported "no task payload
arrived with this session"). Take the first workstream below that is still unowned, say in your
first output which one you took, and follow the ground rules.

Round 3 closed the claims-hygiene, kill-switch and explorer-criterion gaps. What is left is the
shallowest block in the matrix: the MEV/privacy family, where three rows sit at 2%, 8% and 31%
with real code underneath that has never been measured or driven end to end.

## Ground rules (unchanged)

1. **Own only your workstream's files.** Other agents share this working tree; HEAD moves under you.
   Re-check `git status` and `git log --oneline -5` before you edit.
2. **No fake green.** Never weaken, delete or `#[ignore]` a test. Reproduce the failure first, then
   fix it, then show the test going red when you undo the fix (the "break it first" control).
3. **Fail closed.** If something cannot be proven, refuse the operation — do not convert UNKNOWN
   into SUCCESS. Follow `AGENTS.md` §5, §17-§20.
4. **Do not edit** `feature-matrix/*.toml` (report the row delta as text; the primary agent applies
   it), `scripts/local-ci.sh`, `reports/rc6/*`, `reports/panic_unwrap_audit.md`, `FETCH_HEAD`,
   `libproto_lib/`.
5. **No multi-node gate runs.** The serial gates bind fixed ports and the primary agent runs them.
   Unit and integration tests only.
6. You may `git add` **your own files** and commit with a focused message. If the commit fails on
   `index.lock`, wait and retry. Do **not** push and do **not** `git add -A`.
7. Report in the shape below, with real command output. A row delta is only accepted with named
   tests that exist and a command that produces them.

```
FIXED: <files, one line each>
TESTED: <exact commands + pass/fail counts + the break-it-first output>
FINDINGS: <what is real, what is stub, what is unreachable, what fails open>
ROW DELTA for <ID>: implemented=? tested=? mainnet_ready=? source=? paths=[...] blockers=[...]
  required_tests=[<existing test fns by name>] test_evidence=[...]
COMMIT: <sha or "left uncommitted">
REMAINING BLOCKERS: <honest>
NEXT 5 TASKS: <concrete>
Completion percent for this row: <?>% and what it means
```

---

## Workstream A — `X3-MEV-007` encrypted mempool / threshold encryption (row: 5/0/0)

`crates/private-mempool` is a real crate with real threshold crypto — Ristretto Shamir
(`src/threshold.rs`: `split_secret`, `evaluate_polynomial`, `lagrange_coefficient`,
`combine_points`), committee-ECDH + HKDF + AES-256-GCM (`src/encryption.rs`), a queue, and ~19 unit
tests. The matrix still scores it 5/0/0 with `source = "research"` and `paths = []`, which is
stale. `crates/confidential-gpu` consumes it.

Deliverable: measure it honestly, then make the missing adversarial cases real.

* Prove `t` shares decrypt and `t-1` do not, over a freshly generated committee.
* Prove a tampered ciphertext, a tampered nonce, a swapped nonce and a swapped ephemeral key each
  fail closed rather than returning garbage.
* Prove a share with the wrong index, a duplicate index and index 0 are refused.
* Prove a share minted for a different DKG epoch is refused — **if no such check exists, that is the
  defect to fix**, with an explicit typed error.
* Prove `combine_shares` enforces the declared threshold instead of trusting its caller.
* Establish the canonical path: `cargo tree -i private-mempool`, `rg -rn "private_mempool"`. If
  nothing outside `confidential-gpu`'s tests reaches it, say so — an unreachable crate is not an
  implemented feature.

Own: `crates/private-mempool/src/{lib.rs,encryption.rs,threshold.rs}` and its tests. Do **not**
touch `src/queue.rs` (workstream B may hold it).

Commands: `cargo test -p private-mempool`,
`cargo clippy -p private-mempool --all-targets -- -D warnings`, `cargo fmt --all -- --check`.

---

## Workstream B — `X3-MEV-008` fair transaction ordering (row: 15/5/5) + `X3-MEV-006`

Blocker on record: "No production fair-ordering protocol identified". Start by measuring what
exists: `crates/x3-swap-router/src/mev_protection/mod.rs` (`ProtectionStrategy`,
`SandwichProtection`, `MEVProtector`), `crates/private-mempool/src/queue.rs` (fee-ordered queue),
`pallets/private-execution/src/{lib,types,tests}.rs`, `crates/x3-dex/src/tests/attack_liquidation_frontrun.rs`.

Deliverable: the smallest honest, testable core of fair ordering — a commit-reveal ordering lane
where a transaction commits `H(plaintext ‖ nonce ‖ sender)` under a bond and a deadline, and the
canonical order is determined by the fixed window rather than arrival time. Explicit fail-closed
refusals, each with a named test: reveal without commit; reveal after the window closes; reveal that
does not hash to its commit; double reveal; reveal from a different sender; commit never revealed.

Ordering must be deterministic: no wall-clock ordering, no `HashMap` iteration deciding a sequence
(`BTreeMap` or an explicit total order). Repeated runs and permuted arrival order inside a window
must produce the same sequence — if the implementation is first-come-first-served, that is not fair
ordering and you must say so rather than dress it up.

Own: `crates/x3-swap-router/src/mev_protection/**` (and `crates/private-mempool/src/queue.rs` only if
workstream A is not holding it). Report on `X3-MEV-008` and, if you moved it, `X3-MEV-006`.

Commands: `cargo test -p x3-swap-router`, `cargo clippy -p x3-swap-router --all-targets -- -D warnings`,
`cargo fmt --all -- --check`.

---

## Workstream C — `X3-MEV-002` private transaction submission controls (row: 45/20/25, P0)

Blocker on record: "Trading Core hardening still requires private-submission enforcement".

Question to answer with code: **can an `.x3` program demand private submission, and does anything
enforce it at compile time?** Read `x3-lang/compiler/src/{trading_semantic.rs,trading_lowering.rs,verify.rs}`,
the spec under `x3-lang/spec/`, and `pallets/private-execution/src`.

Prefer the smallest real enforcement: a compiled policy field carried through lowering and checked
by the verifier, with a rule that refuses a caller-supplied runtime override flipping the compiled
policy (`AGENTS.md` §11). If the policy already exists and is enforced, prove it by *breaking it*
first and raise the row on that evidence rather than adding machinery.

Note: `x3-lang` is a separate cargo workspace —
`cargo test --manifest-path x3-lang/Cargo.toml -p x3-lang-compiler`. `X3-LANG-007` was raised today
with `required_tests` in `x3-lang/compiler/tests/test_ir_verifier.rs`; extend that file rather than
duplicating it.

Own: `x3-lang/compiler/src/**`, `x3-lang/compiler/tests/**`, and `x3-lang/vm/src/**` only if the
verifier's contract requires it.

---

## Not in scope for round 4 (already ticketed)

* `X3-GPU-001` GPU finalized-TPS proof (7%): blocked on physical hardware — no compute device on this
  box, and `GPU_VALIDATOR_HONEST_AUDIT.md` forbids the claim without one. Do not work it here.
* `apps/**` claim surfaces outside the CRM (TICKET-140) and the `production/public/` deployment copy
  (TICKET-141): claims-hygiene, not MEV. Round 3 landed the scanner widening; extend it if you are
  asked, not by default.
* CRM dead modules (TICKET-138) and the legacy claim pile (TICKET-139): ticketed, recorded, not
  round 4.
