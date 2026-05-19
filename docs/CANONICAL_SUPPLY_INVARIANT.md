# Canonical Supply Invariant

## Purpose

Define and verify the supply safety invariant for X3 asset accounting.

## Invariant

For each asset, the canonical supply must equal represented supply across all domains and in-flight balance:

```text
canonical_supply
= native_supply
+ evm_supply
+ svm_supply
+ x3vm_supply
+ external_locked_supply
+ pending_supply
```

No operation may violate this equality.

## Enforcement Surfaces

- `pallets/x3-supply-ledger/src/lib.rs`
- `pallets/x3-kernel/src/lib.rs`
- `pallets/x3-cross-vm-router/src/lib.rs`

## Mutation Rules

1. Mint increases canonical + one representation leg.
2. Burn decreases one representation leg + canonical.
3. Route debit moves from representation leg to pending.
4. Route settle moves from pending to destination leg.
5. Timeout refund moves from pending back to source leg.
6. External lock/unlock changes external_locked and canonical in lock-step.

## Verified Tests

- `tests_phase4/invariant_registry_check.rs`:
  - deterministic 10k-op fuzz harness
  - edge coverage for partial settlement and timeout refund

- `pallets/x3-supply-ledger/src/lib.rs`:
  - per-operation invariant checks (`ledger.check_invariant()`)
  - `on_finalize` block-level invariant scan with halt policy

## Failure Behavior

If invariant verification fails:

- operation fails with `InvariantViolation`
- block finalization emits `SupplyInvariantViolation`
- transfer flow is halted under configured invariant policy

## Current Status

Implemented with per-op checks and block-finalization checks. Cross-domain and pending balance paths are covered in runtime logic and targeted tests.
