# X3 FeatureBuiltProof Report

## Verdict
**BLOCKED**

## Summary
| Status | Count |
|---|---:|
| BUILT | 0 |
| PARTIAL | 0 |
| MISSING | 33 |
| UNWIRED | 17 |
| UNTESTED | 8 |
| WEAK | 1 |
| STALE | 0 |
| BLOCKED | 0 |
| REVOKED | 0 |

## Top Blockers
1. x3.accounts: 1 code files missing
2. x3.accounts: 1 negative tests missing
3. x3.accounts: 2 tests missing
4. x3.accounts: proof receipt missing
5. x3.asset_kernel: 1 code files missing
6. x3.asset_kernel: 2 wiring checks failed
7. x3.asset_kernel: 3 negative tests missing
8. x3.asset_kernel: 3 tests missing
9. x3.asset_kernel: proof receipt missing
10. x3.asset_mapping: 1 code files missing

## Built Features
| Feature | Status |
|---|---|

## Partial Features
| Feature | Blockers | Next Command |
|---|---|---|

## Missing Features
| Feature | Missing Code |
|---|---|
| x3.validator_set | pallets/staking/src/lib.rs |
| x3.fee_market | pallets/transaction-payment/src/lib.rs |
| x3.accounts | pallets/system/src/lib.rs |
| x3.balances | pallets/balances/src/lib.rs |
| x3.canonical_supply | pallets/x3-kernel/src/supply.rs |
| x3.asset_registry | pallets/x3-kernel/src/registry.rs |
| x3.asset_mapping | pallets/x3-kernel/src/mapping.rs |
| x3.mint_burn_controller | pallets/x3-kernel/src/mint_burn.rs |
| x3.external_locked_accounting | pallets/x3-kernel/src/external_locked.rs |
| x3.pending_transfer_accounting | pallets/x3-kernel/src/pending_transfer.rs |
| x3.supply_invariant_guard | pallets/x3-kernel/src/invariant.rs |
| x3.x3vm_bytecode | crates/x3-vm/src/bytecode.rs |
| x3.x3vm_gas_metering | crates/x3-vm/src/gas.rs |
| x3.x3vm_storage | crates/x3-vm/src/storage.rs |
| x3.x3vm_events | crates/x3-vm/src/events.rs |
| x3.x3vm_revert | crates/x3-vm/src/revert.rs |
| x3.x3vm_cpu_gpu_parity | crates/x3-vm/src/gpu.rs |
| x3.evm_precompiles | crates/evm-integration/src/precompiles.rs |
| x3.svm_syscalls | crates/svm-integration/src/syscalls.rs |
| x3.vm_isolation | crates/x3-vm/src/isolation.rs |
| x3.vm_state_transition | crates/x3-vm/src/state.rs |
| x3.vm_fallback | pallets/x3-cross-vm-router/src/fallback.rs |
| x3.vm_metering | pallets/x3-cross-vm-router/src/metering.rs |
| x3.x3lang_parser | crates/x3-compiler/src/parser.rs |
| x3.x3lang_typechecker | crates/x3-compiler/src/typechecker.rs |
| x3.x3lang_ir | crates/x3-compiler/src/ir.rs |
| x3.x3lang_optimizer | crates/x3-compiler/src/optimizer.rs |
| x3.x3lang_bytecode_generator | crates/x3-compiler/src/codegen.rs |
| x3.x3lang_abi_generator | crates/x3-compiler/src/abi.rs |
| x3.x3lang_contract_templates | templates/src/ |
| x3.flashloan | pallets/x3-flashloan/src/lib.rs |
| x3.dex | pallets/x3-dex/src/lib.rs |
| x3.dashboard | dashboard/src/ |

