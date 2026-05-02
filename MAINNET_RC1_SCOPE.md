# X3 Atomic Star — Mainnet RC-1 Scope

> **Rule**: Anything not listed in RC-1 below is disabled, feature-gated, or excluded from genesis.
>
> This document is machine-verified by `crates/x3-readiness-report`.
> Changing it without updating the readiness gates is a launch blocker.

---

## RC-1 Feature Set (SHIP)

| Feature | Crate / Pallet | Status | Notes |
|---|---|---|---|
| Substrate node / runtime | `node`, `runtime` | REQUIRED | Baseline |
| Universal Asset Kernel | `pallets/x3-kernel`, `pallets/x3-supply-ledger` | REQUIRED | Core invariant layer |
| Asset registry | `pallets/x3-asset-registry` | REQUIRED | |
| Account registry | `pallets/x3-account-registry` | REQUIRED | |
| Cross-VM router | `pallets/x3-cross-vm-router` | REQUIRED | Internal routes only |
| X3-IXL | `crates/x3-ixl` | REQUIRED | Internal execution only |
| Packet standard | `crates/x3-packet-standard` | REQUIRED | Internal packet lifecycle |
| LiquidityCore | `crates/x3-liquidity-core` | REQUIRED | Spot AMM + LP locks only |
| Universal contracts | `crates/x3-universal-contracts` | LIMITED | SDK/facade only — not unlimited execution |
| Readiness report | `crates/x3-readiness-report` | REQUIRED | Gates block launch |
| Launch validator | `crates/x3-launch-validator` | REQUIRED | |

---

## RC-1 Disabled / Gated (DO NOT SHIP as mainnet-critical)

| Feature | Cargo feature flag | Reason |
|---|---|---|
| External gateway | `external-gateway` | Unaudited relayer/finality path |
| Parallel executor | `parallel-executor` | Non-determinism risk under current scheduler |
| AppZone factory deployment | `appzone-factory` | Dev-preview only |
| Post-quantum crypto | `pq-experimental` | Roadmap/scaffold only |
| Advanced DEX (perps / options / flash loans) | `advanced-dex` | Not needed for spot settlement |
| AI route optimizer (consensus path) | `ai-optimizer` | Offchain/non-consensus only |
| GPU validator acceleration | `gpu-acceleration` | Benchmark/dev only |

**Runtime rule**: `mainnet-rc1` feature MUST NOT pull any of the above into the
active binary. CI enforces this via `cargo check --no-default-features
--features mainnet-rc1`.

---

## Critical Transaction Flow (RC-1)

```
User submits intent
  → UniversalContract compiles intent to IXL Bundle
  → X3-IXL Planner validates instruction bundle
  → Cross-VM Router executes bundle
  → LiquidityCore handles swap if needed
  → Packet standard commits / receipts packet
  → Kernel updates canonical accounting (supply ledger)
  → Receipt / proof emitted
```

**Supported instructions (all others are rejected at planner level):**
- `Lock` — debit source, place under router custody
- `Mint` — credit destination from custody
- `Burn` — destroy custody asset (refund/cleanup)
- `Swap` — single internal AMM/spot swap via LiquidityCore
- `Settle` — finalise router escrow into destination account
- `EmitProof` — write packet commitment for inter-VM hop
- `Refund` — release escrow to original payer
- `Abort` — explicit abort on validator hazard detection

---

## Invariants that MUST hold after every RC-1 bundle

1. `total_supply == sum(all_account_balances)` — supply ledger invariant
2. No double-mint: duplicate `(asset_id, message_id)` Mint is rejected
3. No packet replay: `(src_chain, src_port, sequence)` can be consumed once
4. Refund path always restores source balance to pre-bundle state
5. LP share accounting: `lp_shares_outstanding` matches pool reserves at all times

---

## What "done" means for RC-1

The following command group must be **boring** (exit 0, no warnings):

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

And these tests must pass from a clean checkout:

```bash
cargo test -p x3-ixl
cargo test -p x3-packet-standard
cargo test -p x3-liquidity-core
cargo test -p x3-universal-contracts
cargo test -p x3-readiness-report
cargo test -p tests-e2e --test mainnet_rc1
```

---

## Out of scope for RC-1 (explicit list)

- External chain bridges (Ethereum, Solana, Bitcoin mainnet)
- Parallel transaction execution
- AppZone contract deployment
- Flash loans
- Perpetuals / options
- Governance treasury payouts (beyond basic allocation)
- AI/ML route optimisation in the consensus path
- GPU-accelerated signature verification in the consensus path
- Post-quantum signature schemes

---

*Last updated: 2026-05-01 — RC-1 scope lock*
*Enforced by: `crates/x3-readiness-report` gate checks*
