# Mint/Burn Permission Audit

## Scope

Audit canonical mint/burn permissions and supply-coupled updates.

Audited files:

- `pallets/x3-kernel/src/lib.rs`
- `pallets/x3-supply-ledger/src/lib.rs`
- `pallets/x3-cross-vm-router/src/lib.rs`
- `pallets/x3-asset-registry/src/lib.rs`
- `runtime/src/lib.rs`

## Findings

### 1. Governance-gated canonical mint/burn

`pallet-x3-supply-ledger` exposes:

- `mint_canonical(origin, asset_id, domain, amount, nonce)`
- `burn_canonical(origin, asset_id, domain, amount)`

Both are guarded by `T::SupplyGovernance::ensure_origin(...)`.

Result: PASS.

### 2. Supply-coupled mutation logic

Mint path (`do_mint_canonical`):

- increments `canonical_supply`
- increments destination domain leg
- enforces `ledger.check_invariant()`

Burn path (`do_burn_canonical`):

- decrements source domain leg
- decrements `canonical_supply`
- enforces `ledger.check_invariant()`

Result: PASS.

### 3. No direct bypass in cross-VM flow

Cross-VM router relies on `SupplyLedgerWrite` trait (`debit_source_to_pending`, `credit_destination_from_pending`, `refund_pending_to_source`) and does not directly mutate canonical ledger storage.

Result: PASS.

### 4. Halt-aware transfer mutation

Supply ledger blocks new transfer legs when halted while allowing refunds, preventing fund stranding during incident response.

Result: PASS.

## Risks / Gaps

- This audit is code-path based; full runtime-origin matrix should be re-validated after any governance origin wiring changes.
- Additional property tests around adversarial origin composition can further harden confidence.

## Conclusion

Canonical mint/burn paths are permission-gated and invariant-coupled in the current implementation. No direct cross-VM bypass path was identified in audited modules.
