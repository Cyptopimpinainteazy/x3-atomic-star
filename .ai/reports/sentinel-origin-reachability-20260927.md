# The Sentinel's privileged origin is unreachable on every chain that ships without sudo

Date: 2026-09-27. Lane: `[x3_sentinel]` (`pallets/x3-sentinel`, registry score 50, one blocker:
"No live governance simulation").

## What was asked for, and what the live run found

The blocker reads as missing evidence. A live three-validator run
(`bash scripts/drills/sentinel_privileged_origin_live.sh`, now the
`sentinel privileged origin` gate) shows it is worse than that.

Measured, on a real `local3` chain — three validators, finalized head with two peers, evidence in
`.ai/runlogs/sentinel-privileged-origin-*/`:

```
[PASS] chain_finalizing          finalized #17 with 2 peers
[sentinel] the token was created — asset 0x9bada2470ed719195842d4832b95b8c4893c92fbbb002710cf72c931f48866e8
[sentinel] Alice cannot freeze an authority — BadOrigin
[sentinel] Bob cannot freeze an authority — BadOrigin
[sentinel] the freeze map is empty after the refused attempts
[sentinel] no privileged path exists on this chain — the runtime wires x3Sentinel::FreezeOrigin =
           EnsureRoot, and this chain has no sudo, so no extrinsic can arrive as root: the freeze
           control cannot be engaged by anyone
[sentinel] PASS — privileged_path=unreachable
```

The driver creates a `CappedMintable` token through `x3TokenFactory.createToken`, mints as the mint
authority (the factory consults `type Sentinel = X3Sentinel` before every supply-changing authority
op), then tries to freeze that authority as Alice and as Bob. Both are refused with `BadOrigin`, the
`frozenAccounts` map stays empty, and minting keeps working — the guard is inert because the state
it guards can never be written.

## Why nothing can arrive as root

| runtime variant | `sudo` | sentinel `FreezeOrigin` |
|---|---|---|
| `dev, not frontier` (local devnet, `local3`) | yes | `EnsureRoot` |
| `dev, frontier` | yes | `EnsureRoot` |
| mainnet-rc1 variant | **no** | `EnsureRoot` |
| default (full experimental set) | **no** | `EnsureRoot` |
| `production + frontier` (post-RC1) | **no** | `EnsureRoot` |

Sources: `runtime/src/lib.rs` lines 533/609/691/754/833 (the five `construct_runtime!` variants and
their gates), 553/629 (`Sudo: pallet_sudo` in the two dev variants only), and the single
`impl pallet_x3_sentinel::Config for Runtime` at 3251 — it is **not** cfg-gated, so all six variants
share `type FreezeOrigin = frame_system::EnsureRoot<AccountId>`.

`EnsureRoot` accepts only `RawOrigin::Root`, and an extrinsic can only arrive as root if the chain
has a pallet that dispatches that way (`pallet_sudo` on the two dev variants). The post-RC1 variants
have none by design ("post-RC1: no sudo"), and the collective's `close` dispatches with
`RawOrigin::Members`, which satisfies `EnsureRootOrHalfCouncil` — the origin five other pallets in
this runtime already use — but not `EnsureRoot`.

So on the chains this project intends to launch, nobody can freeze a mint authority, enrol an asset
for review, or grant a guardian approval. The control exists, is wired into the token factory, and
cannot be engaged.

## What closes this

One of two decisions, both of which belong to the operator rather than to an autonomous agent,
because they change who holds a security power on a mainnet-bound runtime:

1. **Wire a governance origin** — `type FreezeOrigin = EnsureRootOrHalfCouncil;`, matching the
   pallet's own doc comment ("MUST be a privileged origin (Root or a governance council)") and the
   convention of `UpdateOrigin`/`FulfillerOrigin`/`GovernanceOrigin` elsewhere in this runtime. Then
   the same drill takes the `hasSudo === true` branch through the path it finds and asserts the
   effect end to end: freeze accepted, `frozenAccounts` written, mint refused with
   `x3TokenFactory.AuthorityFrozenBySentinel`, unfreeze accepted, mint accepted again. This is a
   runtime edit, so it also needs `scripts/update-runtime-hashes.sh` re-attestation and
   `make mainnet-check`.
2. **Keep root-only and say so** — then the pallet must be documented as a root-callable primitive
   (reachable from a runtime upgrade, not from governance), and `[x3_sentinel]`'s row should say that
   the freeze power is not part of the launch chain's live control surface.

Until one of those lands, `[x3_sentinel]` carries this as a mainnet blocker, not as missing evidence.
