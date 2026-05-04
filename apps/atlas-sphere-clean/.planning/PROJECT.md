# X3 Chain

## What This Is

X3 Chain is a large blockchain monorepo centered on a Substrate-based execution layer with dual-VM ambitions (EVM + SVM), custom pallets, operator tooling, SDKs, and supporting applications. The project already contains substantial implementation work; the current need is to turn that surface area into a defensible release candidate.

## Core Value

Deliver a reliable, extensible blockchain execution engine that can run both EVM and SVM workloads with predictable performance.

## Requirements

### Validated

- ✓ Planning infrastructure and milestone workflow — v1.0 Foundation
- ✓ Gap-closure lifecycle scaffold — v1.0 Foundation

### Active

- [ ] Green the real release gates across audit scripts, package builds, Rust build/test paths, and launch validation
- [ ] Complete release-critical dual-VM and cross-VM flows
- [ ] Harden runtime, node, RPC, and operator safety paths for production
- [ ] Prove deployment and testnet readiness with repeatable runbooks

### Out of Scope

- Full tokenomics and launch marketing
- Non-critical feature expansion that does not move the release candidate closer to ship

## Context

The current ship-readiness baseline is defined by three sources that must agree: `X3_COMPLETION.md`, `X3_GAPS_REPORT.md`, and the executable validation commands in CI/local scripts. v1.0 established planning structure; v1.1 is the first milestone aimed at converting the repo’s existing implementation into something that can survive a real go/no-go review.

## Constraints

- **Tech stack:** Rust (primary), Substrate/Polkadot SDK, Frontier EVM, Solana/SVM integration, TypeScript/JavaScript workspaces, Python tooling
- **Validation burden:** Release claims must be backed by executable checks, not checklist optimism
- **Operational reality:** The repo is large, partially dirty, and contains multiple active subsystems that need coordination rather than isolated fixes

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Use GSD workflow for planning | Provides structured phase/plan tracking, audit capability, and repeatable execution | ✓ Good |
| Treat `X3_COMPLETION.md` and `X3_GAPS_REPORT.md` as release baselines | They capture explicit ship criteria and known gaps better than the old foundation roadmap | ✓ Good |
| Prioritize release-gate truthfulness before broader feature work | A green process is required before a believable ship decision can exist | ✓ Good |

---
*Last updated: 2026-03-15 after v1.0 archive and v1.1 release-readiness triage*
