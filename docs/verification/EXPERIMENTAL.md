# Experimental utilities outside release gates

These source-restored tools remain available for investigation. They are not used by `check-core.sh` as evidence of chain correctness, settlement, quorum or production health.

## Legacy RPC capability probe (`proving/`)

The `x3-prove` implementation probes metadata, health and block information. Labels such as quoting, bundle/submission, rollback and reconciliation do not perform those settlement actions. Its RPC helper can accept a malformed response beginning with `{` as successful. Treat the output as an experimental capability probe, not transaction, atomicity or release proof. Correct response validation and actual execution-backed checks are required before promoting it into a release gate.

## EVM health prototype (`crates/x3-chain-health-daemon/`)

The current implementation polls EVM methods rather than native Substrate health. It ignores the configured chain identity in key paths, can accept a single reachable observation despite a larger requested quorum, and uses placeholder gas/finality display values. Its unit tests do not establish complete metric collection or trustworthy quorum decisions. Do not use it to authorize funds, settlement, automatic recovery or a release.

The source bytes are preserved to avoid silently changing legacy behavior during organization. These limitations are explicit; they are not fixed by moving a directory or by a green compile job. Snapshot/genesis and coordinator/simulator results are separate evidence.
