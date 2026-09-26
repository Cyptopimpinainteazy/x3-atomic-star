# TICKET-139 — the claim scanner reads the surfaces people actually read

Date: 2026-09-26. Branch `feat/x3-prelaunch-economics-x3lang-cutover`.

## What was wrong

`scripts/ci/check-claims-hygiene.py` scanned root markdown, `docs/testnet-config` and the desktop
CRM. The ledger measured ~150 unqualified figures outside that list and ticketed the widening as
TICKET-139, because the gate's own docstring says its surface list is hand-maintained and "a new
surface can be added without being scanned".

## What changed

1. **Surfaces** (`SURFACES`): `docs/**`, `production/public/**`, `.planning/**`, plus the existing
   root markdown and CRM. `production/public/**` was not in the ticket's list — it is the published
   site, which makes it the highest-risk surface in the repository. `docs/**` subsumes the old
   `docs/testnet-config` entry.
2. **Evidence records** (`EVIDENCE_RECORDS`): `benchmarks/`, `.audit/`, `infra-structure/`,
   `tests_phase4/`, `tests_core/`, `tools/` — each with a written reason, each *also* declared as a
   surface so that deleting an entry re-enables scanning and the gate can fail. `--list` prints the
   list with its reasons and a file count.
3. **Qualification rules**: an unchecked `- [ ]` box is a plan item (a checked `- [x]` stays a
   claim); `acceptance`/`require(d)`/`requirement(s)`/`criteria`/`criterion`/`threshold`/`must`/
   `goal(s)`/`objective(s)`/`versus`/`vs`/`expected`/`projected`/`projection`/`estimate(d)` mark a
   bound or a projection rather than a result.

## What it found (41 claims, all corrected)

* `docs/openspec/changes/p4-solana-gpu-acceleration/P4_IMPLEMENTATION_GUIDE.md` — every success
  criterion ticked `[x]` ("100,000+ TPS on mainnet", "500,000 Ed25519 sig/sec") for a GPU system
  with no `.cu`/`.ptx`, no artifact and no benchmark. Now an unchecked checklist with the honest
  reason. `proposal.py`'s generated figures are targets.
* `production/public/x3-ecosystem.html`, `x3-ecosystem(1).html`, `x3-validators.html` — "zero-fee
  flashloans", "sub-200ms finality", "Sub-200ms finality. Zero partial fills. Always.", "With 200ms
  finality vs ETH's 60s".
* `.planning/README.md` — a dated `Testnet Live | Public testnet running 100+ nodes, 1000 TPS` row
  for a testnet this ledger records as undeployed.
* `docs/runbooks/testing/VALIDATION_INDEX.md` — "1,000 TPS validated / ✅ Achieved", for a figure the
  highest recorded chain-level run does not support (~575 TPS single-host, 30.6 TPS on 7 validators;
  `benchmarks/tps-archive-2026-02/README.md`).
* Plus `docs/current/MASTER_CHECKLIST_STATUS.md`, `docs/root/HARDWARE-ACQUISITION-INTEGRATION.md`,
  two `docs/openspec` proposals, three `docs/runbooks` acceptance/threshold lines and one numbered
  heading (`#### 1. TPS Tracker`) the numeric rule read as a figure.

## Negative controls (measured on this box)

```
$ python3 scripts/ci/check-claims-hygiene.py
claims-hygiene: OK - 5498 file(s) scanned, 476 claim surface(s), 520 declared evidence record(s) excluded, no unqualified claim

# new surface is scanned, plan item is not a claim, ticked item is
$ printf '%s\n' '- [ ] 1,000 TPS demonstrated' '<p>Sub-200ms finality at 100,000 TPS.</p>' > production/public/zzz-probe.html
claims-hygiene: FAIL -> production/public/zzz-probe.html:2 unqualified throughput figure   (1 hit; line 1 silent)
$ ... '- [x] 1,000 TPS demonstrated' ...
claims-hygiene: FAIL -> production/public/zzz-probe.html:1 and :2                           (2 hits)

# the evidence-record skip list is load-bearing
$ (delete the "benchmarks/" key from EVIDENCE_RECORDS)
claims-hygiene: FAIL -> benchmarks/tps-archive-2026-02/README.md:40,44,50                    (5 hits)

# the pre-existing X3-CLAIM-002 control still holds
$ echo 'Probe: MEV-proof ordering with 4,200 TPS finality.' > docs/zzz-hygiene-probe.md
claims-hygiene: FAIL -> ABSOLUTE (MEV protection is not implemented (X3-MEV-001..008)) + unqualified throughput figure
```

All probes removed; `git status` shows only the intended files.

## Registry / gate reconciliation

* `python3 scripts/feature_matrix.py check` -> `PASS: 146 features, 21 warning(s)` (warnings
  pre-existing; `X3-CLAIM-002` is capped below 40 mainnet by its own claim-risk rule).
* `bash scripts/check-readiness-consistency.sh` -> `PASS: All status documents are consistent with
  FEATURE_REGISTRY.toml.`
* `feature-matrix/claims-hygiene.toml` updated: new paths, TICKET-139 closed, the old
  `OK - 6016 file(s) scanned, 78 claim surface(s)` measurement replaced with today's, and the three
  new controls recorded as test evidence.

## Left open (ticketed, not silently rewritten)

* TICKET-140 — `apps/**` outside the CRM is still unscanned (measured: 11 unqualified figures in
  three app files).
* TICKET-141 — every `production/public/` page still carries the hero tag `Now live on testnet`
  while nothing is deployed; no rule matches a bare liveness phrase yet.
