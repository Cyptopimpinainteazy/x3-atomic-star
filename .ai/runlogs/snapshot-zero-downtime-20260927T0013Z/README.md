# `snapshot zero downtime export` — run artifacts, 2026-09-27

| file | what it is |
| --- | --- |
| `reproduction-number-not-zero.txt` | The defect on its own, in seconds: an otherwise untouched raw genesis spec with `System::Number` set to the producing chain's height panics on every block it tries to author. It also records that a real raw genesis carries no `System::Number` at all — which is exactly the condition `restore --regenesis` restores. |
| `red-without-regenesis.txt` | The gate with `--regenesis` removed from its restore step: it fails, which is what shows the fix carries the gate. |

`.log` files are gitignored repo-wide, so the raw node logs these two were cut
from are left beside them on the box under the same names.

The green run's numbers are recorded in the `X3-OPS-002` row's blockers and in
`TESTNET_GAP_LEDGER.md` (GAP-SNAPSHOT-REGENESIS): anchor height 513 of a live
chain that kept finalizing through the export (523 -> 572 -> 587), 1881 keys, the
recomputed trie root equal to the chain's own published `stateRoot`, the chain's
own GRANDPA justification as the snapshot's finality proof, and a restored
authority that finalizes at height 7 with every one of its 1877 non-bookkeeping
entries byte-identical to the export.
