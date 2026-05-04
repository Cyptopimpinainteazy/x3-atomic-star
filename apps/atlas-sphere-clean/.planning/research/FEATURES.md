# Feature Landscape

**Domain:** Modular blockchain execution layer (EVM + SVM) with atomic cross-VM transaction support
**Researched:** 2026-03-15

## Table Stakes
Features users expect. Missing = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|--------|--------------|------------|-------|
| EVM compatibility (Frontier) | Ethereum tooling + smart contract ecosystem | Medium | Already integrated; needs deployment & testing of real contracts |
| SVM/BPF compatibility | Solana program compatibility | Medium | Uses solana-rbpf; needs deployment + state sync |
| JSON-RPC API (eth_*, svm_*) | Standard dApp integration | Medium | Node provides RPC; gaps remain (WebSocket, full method set) |
| Deterministic state transition | Blockchain correctness | High | Substrate gives determinism; cross-VM atomicity adds complexity |
| On-chain fee model + gas accounting | Economic security | High | Unified fee model required for cross-VM operations |
| Full test suite + CI gating | Confidence for distributed systems | High | Enforced via `scripts/x3_audit.sh` and GitHub Actions |
| Security hardening (no unwrap/expect) | Production safety | Medium | Ongoing gaps in repo (see gaps report) |
| Validator node runtime + light client | Run the chain & validate state | High | Core product; node binary exists but needs operational maturity |

## Differentiators
Features that set product apart; not expected but high value.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Atomic cross-VM transactions (“Comits”) | Enables composable apps spanning EVM + SVM | Very High | Unique capability; requires strict locking/rollback semantics |
| Unified account model (EVM H160 + SVM pubkey → single AccountId) | Simplifies UX across VMs | Medium | Reduces friction for cross-VM workflows |
| Self-auditing engineering workflow (“Checklist as Law”) | Enforces completeness and continuous progress | Low | Requires discipline; already scaffolded with audit scripts |
| GPU validator / Parallel proposer | High throughput, low latency blocks | High | Advanced validator design; still in progress (GPU validator phase) |
| Web-native SDKs + tooling (TS, Python, wallets) | Lowers developer onboarding barrier | Medium | Multiple SDKs in repo; still needs packaging + releases |

## Anti-Features
Features to explicitly *not* build (or defer) to avoid scope creep.

| Anti-Feature | Why Avoid | What to Do Instead |
|-------------|-----------|-------------------|
| Own consensus algorithm / fork of Substrate consensus | Massive reimplementation risk | Use Substrate’s Aura/Grandpa and focus on VM/atomic layer |
| Trying to support >2 VMs simultaneously early | Makes atomicity and tooling blow up | Focus on EVM + SVM first, then evaluate additional VMs post‑stability |
| On-chain tokenomics / rewards system before core chain stable | Hard to get right; can destabilize testnets | Defer tokenomics to later phase (out of scope per PROJECT.md) |
| Production-grade wallet UI in repo | Distracts from protocol stack | Provide minimal reference UI; prioritize SDKs and APIs |

## Feature Dependencies
```
EVM support → RPC eth_* endpoints → SDK + frontend dApp support
SVM support → RPC svm_* endpoints → SDK + frontend dApp support
Atomic Comits → Unified account model + fee model + account locking
Validator performance → Parallel proposer + GPU validator + deterministic execution
```

## MVP Recommendation
Prioritize:
1. **Core chain execution correctness** (Substrate runtime + node build + full test pass)
2. **EVM + SVM runtime correctness** (contract/program deployment + basic intercepts)
3. **Cross-VM atomicity (Comits)** - the differentiator; required for any multi-VM dApp

Defer:
- Advanced validator orchestration (GPU validator / parallel proposer) until basic chain + atomicity stable
- Full token economics and governance bundles

## Sources
- `docs/ARCHITECTURE.md`
- `X3_GAPS_REPORT.md` (gap analysis)
- `.planning/PROJECT.md` (scope & constraints)
- `docs/X3_SYSTEMS.md` (audit/enforcement model)
