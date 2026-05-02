# CURRENT MAINNET STATUS

Single source of truth for mainnet RC readiness.

- Target: v0.4 Internal-Only Mainnet RC
- ExternalBridgesEnabled: false
- Last verified commit: UNVERIFIED_IN_THIS_PATCH
- Mainnet readiness score: 68/100
- Launch verdict: NO-LAUNCH

## Enabled Runtime Features

- X3Native, X3Evm, X3Svm internal domains
- Internal cross-VM asset movement
- Supply ledger enforcement
- Cross-VM router internal routes
- Packet standard MVP commitment and timeout checks
- IXL MVP receipt emission gate
- Atomic bundle lifecycle components
- Spot swap path only where already present in existing runtime/pallet code

## Disabled Or Deferred Features

- External Ethereum, Solana, BTC bridges
- External liquidity gateway
- Arbitrary external proof minting
- AppZone factory
- PQ cryptography tracks
- GPU validator as consensus-critical path
- AI agents with fund control authority
- Automatic flashloan or autonomous mainnet strategy systems

## Current Blockers

- Runtime hook invariant policy was previously log-only behavior in supply-ledger finalization path; now patched, requires full workspace gate pass.
- External bridge toggle lacks a documented audit-gate guard in pallet tests and docs; now patched, requires test execution confirmation.
- Mainnet chain-spec artifacts and genesis review were missing dedicated mainnet RC files; now added, require generation and commit of real raw spec from node binary.
- End-to-end rollback and replay tests require full suite execution against current branch.

## Fresh Verification Commands

- cargo fmt --check
- cargo check --workspace
- cargo test --workspace
- cargo build --release -p x3-chain-node
- cargo build --release -p x3-cli
- cargo build --release -p x3-proof
- cargo test -p pallet-x3-cross-vm-router
- cargo test -p pallet-x3-supply-ledger
- cargo test -p pallet-x3-atomic-kernel
- cargo test -p x3-ixl
- cargo test -p x3-proof
- ./scripts/mainnet/panic_unwrap_audit.sh
- ./scripts/mainnet/generate_mainnet_chain_spec.sh
- x3-proof mainnet-rc-report --out reports/mainnet_rc_report.md

## Launch Conditions

Launch is blocked until all quality gates in docs/MAINNET_LAUNCH_CHECKLIST.md are green and reports/mainnet_rc_report.md concludes LAUNCH.
