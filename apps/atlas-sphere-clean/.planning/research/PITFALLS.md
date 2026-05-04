# Domain Pitfalls

**Domain:** Multi-VM blockchain execution layer (EVM + SVM)
**Researched:** 2026-03-15

## Critical Pitfalls

### Pitfall 1: Cross-VM Deadlocks
**What goes wrong:** Concurrent atomic transactions lock accounts in inconsistent order, causing deadlocks and stalled blocks.
**Why it happens:** Without a canonical lock ordering, two transactions can each hold one lock and wait for the other.
**Consequences:** Chain stalls, validators fork, or liveness issues.
**Prevention:** Always acquire locks in deterministic order (e.g., sorted account IDs) in the kernel prepare phase.
**Detection:** High contention metrics, stalled block production, increased `PendingComits` backlog.

### Pitfall 2: Fee Griefing / Resource Drain
**What goes wrong:** Attackers submit transactions that consume compute/gas without paying, or force excessive rollback work.
**Why it happens:** Incomplete fee reservation model or allowing execution before fees are reserved.
**Consequences:** Validator resource exhaustion, DOS on RPC.
**Prevention:** Reserve fees before execution (prepare phase), enforce minimum fee, and apply penalties for failures.
**Detection:** Spike in high-gas/compute transactions, `ComitFailed` events with fee refund anomalies.

### Pitfall 3: Unsafe Rust and Panics in Production
**What goes wrong:** `unwrap()/expect()` or unsafe blocks cause panics or undefined behavior in runtime.
**Why it happens:** Quick prototyping without error handling; reliance on `unwrap` as shortcuts.
**Consequences:** Node crash, chain halt, broken consensus.
**Prevention:** Enforce `Result` handling, scan for `unwrap/expect` in CI (`scripts/x3_audit.sh`), review unsafe blocks.
**Detection:** CI warnings/failures, runtime panics in logs, segfaults in pallet tests.

### Pitfall 4: Inconsistent State Between VMs
**What goes wrong:** EVM and SVM state diverge because of separate storage layers or inconsistent replay.
**Why it happens:** Treating EVM and SVM as separate chains; not collapsing to canonical trie.
**Consequences:** Incorrect cross-VM transfers, long-term state corruption.
**Prevention:** Keep all VM state rooted in Substrate storage; use kernel to apply unified state diffs.
**Detection:** Mismatched balances, failing cross-VM calls, inability to replay blocks deterministically.

## Moderate Pitfalls

### Pitfall 5: Overly Broad RPC Surface without Rate Limiting
**What goes wrong:** RPC endpoints are abused (DDoS, spam) when exposed at scale.
**Prevention:** Implement rate limiting and authentication; provide `rpc` config options.
**Detection:** High request rates, request spikes, node slowdown.

### Pitfall 6: Dependency Drift in Substrate Fork
**What goes wrong:** Custom patches to Substrate dependencies diverge, making upgrades hard.
**Prevention:** Pin Substrate revision and track changes; minimize patches, upstream fixes when possible.
**Detection:** Merge conflicts on Substrate updates, broken builds after dependency updates.

### Pitfall 7: Test Flakiness due to Parallel Execution
**What goes wrong:** Tests pass when run serially but fail under parallel execution (e.g., thread-local storage issues).
**Prevention:** Use reproducible test ordering; isolate shared state; prefer `--test-threads=1` when needed.
**Detection:** Intermittent failures, segfaults, race conditions in test suite.

## Minor Pitfalls

### Pitfall 8: Missing Documentation / Onboarding
**What goes wrong:** New contributors cannot find entry points due to missing README or outdated docs.
**Prevention:** Keep `docs/` and `.planning/` artifacts up to date; include quick start.
**Detection:** Frequent questions about where to start; stale docs.

### Pitfall 9: Ignoring Coverage Requirements
**What goes wrong:** Critical code paths lack tests; regressions slip in.
**Prevention:** Enforce coverage thresholds in `scripts/x3_coverage_gate.sh` and CI.
**Detection:** Coverage report failures; untested critical modules in gaps report.

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Dual VM integration | State divergence + atomicity bugs | Add end-to-end Comits tests, validate final state after rollback |
| Validator scaling | Non-deterministic GPU execution | Ensure GPU execution is deterministic and replayable; use reference CPU path for validation |
| SDK / tooling | API drift vs runtime | Lock RPC API versions, provide SDK regression tests against local node |

## Sources
- `X3_GAPS_REPORT.md` (gap analysis and fixed issues)
- `docs/ARCHITECTURE.md` (atomicity and architecture patterns)
- `docs/X3_SYSTEMS.md` (audit/enforcement practices)
