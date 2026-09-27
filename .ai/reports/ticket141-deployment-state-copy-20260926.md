# TICKET-141 — deployment-state copy, and the rule that now reads it

Date: 2026-09-26. Branch `feat/x3-prelaunch-economics-x3lang-cutover`, base `b80604eac`.

## The defect

The claims gate (`scripts/ci/check-claims-hygiene.py`) had rules for capability phrases
(`ABSOLUTE`) and for performance/traction figures (`NUMERIC`), and none for **deployment
state**. Four sites therefore stated a live network on a repository whose own ledger records
that nothing is deployed:

| file | said |
| --- | --- |
| `production/public/x3-ecosystem.html` (hero tag) | `Now live on testnet` |
| `production/public/x3-ecosystem(1).html` (hero tag) | byte-identical twin of the above |
| `production/public/x3-ecosystem*.html` (final CTA) | "Testnet is live. Contracts are deployed." |
| `docs/root/README.md` | "X3 Chain Testnet v1 is NOW LIVE!" — ten lines above the same file's own "Testnet: not deployed" evidence |

## Reproduction — the gate was green with the claim present

```
$ python3 scripts/ci/check-claims-hygiene.py            # before any edit, HEAD b80604eac
claims-hygiene: OK - 5482 file(s) scanned, 460 claim surface(s), 520 declared evidence record(s) excluded, no unqualified claim
exit=0
```

The scanner at `b80604eac` contains no liveness pattern (`git show HEAD:scripts/ci/check-claims-hygiene.py
| grep -c liveness` -> 0) while the page carried the tag. A rule that does not exist cannot
fail, which is what TICKET-141 was filed about.

## The change

* `scripts/ci/check-claims-hygiene.py` — a third rule set, `LIVENESS`, surface-scoped like
  `NUMERIC` and sharing its escape hatches (a qualifier such as `not`/`planned`/`target`, or
  an unchecked `- [ ]` plan box). Patterns: `now live`; `live on (testnet|mainnet|devnet)`;
  `(testnet|mainnet|network|chain) is (now) live`; `we're live`. Deliberately narrow so the
  registry's `LIVE_TESTNET` lifecycle label, "live network" panel headings and "live test"
  gates stay out of it.
* the four copy sites above — each now states the real deployment state.
* `TESTNET_GAP_LEDGER.md` — TICKET-141 moved to CLOSED with its measurement.

The hero's pulsing indicator dot was removed with the claim rather than left decorating an
"in development" tag.

## Proof — the rule is load-bearing, both directions

```
# control 1: reintroduce the phrase on a tracked probe under the public site -> must FAIL
$ git add -N -f production/public/zzz-ticket141-probe.html   # <div class="hero-tag">Now live on testnet</div>
$ python3 scripts/ci/check-claims-hygiene.py
claims-hygiene: FAIL
  production/public/zzz-ticket141-probe.html:1: unqualified deployment-state claim: <div class="hero-tag">Now live on testnet</div>
1 unqualified claim(s) across 5483 scanned file(s).
exit=1

# control 2: the escape hatches, same file -> must pass
#   <p>The public testnet is not live yet.</p>
#   - [ ] Go live on testnet
#   <p>Target: the mainnet is live by Q4.</p>
$ python3 scripts/ci/check-claims-hygiene.py
claims-hygiene: OK - 5483 file(s) scanned, 461 claim surface(s), 520 declared evidence record(s) excluded, no unqualified claim
exit=0

# probe removed; final run on the clean tree
$ python3 scripts/ci/check-claims-hygiene.py
claims-hygiene: OK - 5482 file(s) scanned, 460 claim surface(s), 520 declared evidence record(s) excluded, no unqualified claim
exit=0
```

Adjacent gates, same tree:

```
$ python3 scripts/feature_matrix.py check
feature-matrix check PASS: 146 features, 21 warning(s)

$ bash scripts/check-readiness-consistency.sh
PASS: All status documents are consistent with FEATURE_REGISTRY.toml.

$ git diff --check                # clean for every file this change touches
```

## Still open (not this ticket)

* The public pages' `Launch on Testnet` button links to `#`, and the built bundle
  `production/public/assets/index-*.js` falls back to fabricated dashboard numbers (42
  validators, "99.8%" uptime, `$0.0001` fee) when its API call fails. Both assert the same
  non-existent deployment state, but they are a **surface/bundle** problem (TICKET-140
  shaped) rather than a liveness-phrase problem, so the rule does not read them.
* TICKET-140 (the `apps/**` surfaces) remains open and unowned.

## Files changed

`scripts/ci/check-claims-hygiene.py`, `production/public/x3-ecosystem.html`,
`production/public/x3-ecosystem(1).html`, `docs/root/README.md`, `TESTNET_GAP_LEDGER.md`.
