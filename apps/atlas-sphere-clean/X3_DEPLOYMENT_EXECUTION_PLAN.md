[Overview]
Deliver X3 Chain to a credible 3-validator public testnet first, then harden it for mainnet. The original plan mixed build-out, operations, and launch governance into one stream; this revised version separates them into 3 execution tracks with explicit gates. The goal is to reduce delivery risk by proving correctness and operability in the smallest production-like environment before spending time on mainnet-only work.

This plan is organized as:
- `Track 1: Testnet MVP` — minimum viable public testnet
- `Track 2: Testnet Hardening` — resilience, observability, repeatability
- `Track 3: Mainnet Readiness` — security, compliance, sustained ops

Success is measured with hard gates, not just file creation.

[Exit Criteria]

**Testnet MVP exit criteria**
- 3 validators boot from the same chain spec and stay in consensus
- HTTP and WebSocket RPC both work
- EVM and SVM paths both execute and persist state to the canonical ledger
- Cross-VM happy-path transfer works end-to-end
- Health checks and Prometheus metrics are live
- One-command deployment and one-command health validation exist
- E2E smoke suite passes

**Testnet Hardening exit criteria**
- Failover/restart/disaster scenarios are exercised
- RPC rate limiting and security defaults are enforced
- Load baseline is documented
- Audit and coverage gates pass in CI
- Monitoring dashboards and alerts are actionable
- Re-deploying testnet is repeatable

**Mainnet Readiness exit criteria**
- External security audit complete
- Mainnet chain spec and validator process finalized
- Rollback and incident runbooks approved
- 7-day soak on production-like infra passes
- Governance approval and key management procedures are complete

[Phase 0: Truth Pass]
Do this before any implementation sprint.

1. Reconcile `X3_COMPLETION.md`, `X3_GAPS_REPORT.md`, and actual code state.
2. Mark each gap as one of:
- `missing`
- `partial`
- `implemented but unverified`
- `done`
3. Validate whether these already exist in usable form:
- `node/src/rpc.rs`
- `node/src/rpc_frontier.rs`
- `node/src/service.rs`
- `node/src/chain_spec.rs`
- `crates/cross-vm-bridge/src/lib.rs`
- `crates/evm-integration/src/lib.rs`
- `crates/svm-integration/src/lib.rs`
4. Produce a short truth table with:
- blocker
- owner
- current state
- proof
- next action

Gate:
- no sprint work starts until the blocker list is real and deduplicated

[Track 1: Testnet MVP]

**Phase 1: Build + Repo Integrity**
1. Run workspace compile, format, tests, audit script, and coverage gate.
2. Fix only blockers that prevent shipping a 3-node testnet.
3. Archive or remove `_unused/` only after confirming nothing references it.

Deliverables:
- green workspace baseline
- updated `X3_COMPLETION.md`
- validated blocker list

Gate:
- repo is buildable and auditable from a clean checkout

**Phase 2: RPC Completion**
1. Finish HTTP + WebSocket RPC in `node/src/rpc.rs`.
2. Wire Frontier methods in `node/src/rpc_frontier.rs`.
3. Add a simple health/status endpoint.
4. Add RPC integration tests for:
- websocket subscriptions
- health endpoint
- basic EVM RPC
- basic chain queries

Deliverables:
- working RPC server
- `tests/integration/rpc_websocket_test.rs`

Gate:
- Polkadot.js can connect over WebSocket
- scripted RPC smoke test passes

**Phase 3: Dual-VM Correctness**
1. Implement ledger sync in:
- `crates/evm-integration/src/lib.rs`
- `crates/svm-integration/src/lib.rs`
2. Implement happy-path atomic transfer plus rollback in `crates/cross-vm-bridge/src/lib.rs`.
3. Add integration tests for:
- EVM to ledger
- SVM to ledger
- cross-VM transfer
- rollback on failure

Deliverables:
- `tests/integration/cross_vm_test.rs`

Gate:
- canonical ledger reflects both VM paths correctly
- rollback restores consistent state

**Phase 4: 3-Node Consensus**
1. Add testnet chain spec and genesis artifacts.
2. Add multi-node boot validation and graceful shutdown in `node/src/service.rs`.
3. Stand up a 3-validator local or containerized cluster.
4. Add tests for:
- block production
- finality
- restart recovery
- peer connectivity

Deliverables:
- `testnet/genesis.json`
- `testnet/chain-spec.json`
- `tests/integration/multi_node_consensus_test.rs`

Gate:
- 3 validators stay in consensus for a sustained run
- restart of one validator does not break finality

**Phase 5: Deployable Testnet Stack**
1. Create a minimal deployment stack first with Docker Compose.
2. Add:
- node image
- rpc image
- monitoring services
3. Create:
- `scripts/deploy-testnet.sh`
- `scripts/health-check.sh`

Deliverables:
- `deployment/docker/Dockerfile.node`
- `deployment/docker/Dockerfile.rpc`
- `deployment/docker/docker-compose.testnet.yml`

Gate:
- fresh environment can deploy a 3-node testnet and pass health checks

[Track 2: Testnet Hardening]

**Phase 6: Security Defaults**
1. Add CORS restrictions and RPC limits.
2. Remove or reduce panic-prone paths.
3. Add SAFETY comments to unsafe blocks.
4. Run clippy, audit, and unwrap/expect scans.

Gate:
- no known critical insecure defaults on public-facing RPC
- production code panic scan is acceptable

**Phase 7: Observability**
1. Add blockchain-specific Prometheus metrics.
2. Add Grafana dashboard for:
- peer count
- block time
- finality lag
- RPC latency
- TPS
3. Add alerting rules.

Deliverables:
- updated `prometheus.yml`
- dashboard JSON or YAML
- alert rules

Gate:
- operators can detect consensus stall, peer collapse, and RPC degradation quickly

**Phase 8: Load + Recovery**
1. Add throughput benchmark.
2. Add concurrent RPC stress.
3. Add memory and state accumulation checks.
4. Add disaster recovery and validator restart scenarios.

Deliverables:
- `tests/load/tps_benchmark.rs`
- `tests/e2e/full_lifecycle_test.rs`
- `tests/e2e/disaster_recovery_test.rs`

Gate:
- baseline TPS documented
- node crash and restart tested
- full lifecycle smoke passes

**Phase 9: CI/CD + Repeatability**
1. Wire testnet deploy workflow in GitHub Actions.
2. Ensure audit and coverage gates run before deployment.
3. Make testnet deployment reproducible.

Deliverables:
- `.github/workflows/deploy-testnet.yml`

Gate:
- one approved pipeline can build, test, and prepare testnet deployment artifacts

[Track 3: Mainnet Readiness]

**Phase 10: Mainnet Spec + Ops Controls**
1. Create separate mainnet genesis and chain spec.
2. Finalize validator identity, bootnodes, telemetry, backups, persistence, and firewall rules.
3. Document rollback and incident response.

Gate:
- mainnet config is distinct from testnet and operationally reviewable

**Phase 11: External Audit + Soak**
1. Complete external security audit.
2. Fix critical findings.
3. Run 7-day soak on production-like infrastructure.
4. Validate monitoring, paging, and recovery runbooks.

Gate:
- no unresolved critical findings
- soak run passes with acceptable stability

**Phase 12: Governance Launch Readiness**
1. Finalize key custody and signer procedures.
2. Finalize approval workflow.
3. Dry-run launch and rollback.
4. Obtain governance signoff.

Gate:
- launch can proceed without hidden operational dependencies

[Priority Order]
If we want the real critical path, it is this:

1. Truth pass
2. RPC completion
3. Dual-VM state sync
4. 3-node consensus
5. Deployable testnet stack
6. Health and metrics
7. E2E smoke
8. Security defaults
9. Load and recovery
10. Mainnet-only work

[What To Cut From The First Sprint]
These should not block Testnet MVP:
- Kubernetes manifests
- HPA
- mainnet tokenomics
- 7-day soak
- external audit
- governance approval
- enterprise-grade TPS target
- full GPU reward distribution unless testnet-critical

[Recommended Ownership]
Use 5 workstreams max:
- `Core Node`: RPC, service, chain spec
- `Dual-VM`: EVM/SVM sync, cross-VM bridge
- `Consensus/Testnet`: 3-node cluster, genesis, deploy scripts
- `Ops`: monitoring, alerts, health checks, CI
- `Security/Quality`: clippy, panic scan, unsafe review, audit gates
