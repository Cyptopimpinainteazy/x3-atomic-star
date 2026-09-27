# The branch was red at `1fb26c556`, and it was a score that was wrong

## Finding

`1fb26c556` ("the retracted MEV claim had moved to the outbound mail") raised `X3-CLAIM-002` to
`implemented = 80, tested = 75, mainnet_ready = 55` and left `source = "claim_risk"`,
`claim_risk = true`, `launch_scope = "research"` in place. `scripts/feature_matrix.py check` refuses
exactly that combination, twice:

```
$ bash scripts/local-ci.sh --only feature-matrix-check
FAIL feature matrix check  2s
  log: .ai/runlogs/local-ci-20260926T171024Z-feature-matrix-check.log
    ERROR: X3-CLAIM-002: claim-risk feature must remain below 40 mainnet readiness
    ERROR: X3-CLAIM-002: research feature must remain below 40 mainnet readiness
    feature-matrix check FAILED: 2 error(s)
```

`feature matrix check` is in the local-ci default set, so the branch was red — not on a warning,
on two errors.

## Why the score moved and not the flags

The row records that a claim was retracted. A row that is a record of a retraction must not read as
a mainnet capability, which is precisely what the 40-point cap on `claim_risk`/`research` rows
enforces. Dropping `claim_risk` to fit the number would be the fake-green move; lowering the number
to fit the rule is the honest one.

35 puts the row alongside its siblings (`X3-CLAIM-001` 35, `X3-CLAIM-003` 30), and the residue is
real: the scanner's 78-surface list is hand-maintained, roughly 150 assertion sites outside it are
ticketed rather than rewritten (TICKET-139), and the CRM modules the text lives in are dead code —
`crm/mod.rs` does not declare them and the crate is outside the cargo workspace, so nothing
type-checks them (TICKET-138).

A blocker line now says the number cannot be raised again without dropping the risk flags in the
same change, so the next reader does not re-raise it and re-break the gate.

## Evidence

```
$ python3 scripts/feature_matrix.py check
feature-matrix check PASS: 146 features, 20 warning(s)

$ python3 scripts/x3_audit_matrix.py
146 matrix rows (BROKEN=0, NOT INTEGRATED=8, COMPLETE=10, FUNCTIONAL BUT UNHARDENED=57,
                 PARTIAL=61, STUB=10, NOT STARTED=0), 21 registry features

$ bash scripts/local-ci.sh --only \
    feature-matrix-check,audit-matrix-freshness,matrix-tests-exist,matrix-test-evidence,readiness-consistency,claims-hygiene
PASS matrix tests exist       0s
PASS feature matrix check     5s
PASS audit matrix freshness   1s
PASS matrix test evidence    36s
PASS readiness consistency   43s
PASS claims hygiene         100s
local-ci: all gates passed            (run 20260926T171107Z)
```

The regenerated `audit-artifacts/current/feature-status.json` and the two `docs/audit/` tables move
only in the `X3-CLAIM-002` row and the source digests; the diff is 9 insertions and 8 deletions
across three derived files.

## Independently re-verified, not taken on report

The other change in flight when this started, `ff75e82c9` (the trading-IR debt-sequence rules), was
re-run here rather than trusted:

```
$ cargo test --manifest-path x3-lang/Cargo.toml -p x3-lang-compiler --test test_ir_verifier
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Commit

`ad0f8345d fix(matrix): the retracted-claim row stays under the research cap` — four files:
`feature-matrix/claims-hygiene.toml`, `audit-artifacts/current/feature-status.json`,
`docs/audit/X3_AGENT_QUEUE.md`, `docs/audit/X3_FEATURE_COMPLETION_MATRIX.md`.
