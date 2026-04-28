# X3 FeatureBuiltProof Report

## Verdict
**BLOCKED**

## Summary
| Status | Count |
|---|---:|
| BUILT | 0 |
| PARTIAL | 1 |
| MISSING | 1 |
| UNWIRED | 42 |
| UNTESTED | 13 |
| WEAK | 2 |
| STALE | 0 |
| BLOCKED | 0 |
| REVOKED | 0 |

## Top Blockers
1. x3.accounts: 1 negative tests missing
2. x3.accounts: 2 tests missing
3. x3.accounts: proof receipt missing
4. x3.asset_kernel: 2 wiring checks failed
5. x3.asset_kernel: 3 negative tests missing
6. x3.asset_kernel: 3 tests missing
7. x3.asset_kernel: proof receipt missing
8. x3.asset_mapping: 1 negative tests missing
9. x3.asset_mapping: 1 wiring checks failed
10. x3.asset_mapping: 2 tests missing

## Built Features
| Feature | Status |
|---|---|

## Partial Features
| Feature | Blockers | Next Command |
|---|---|---|
| x3.dex | 2 tests missing, 1 negative tests missing, proof receipt missing | cargo test -p pallet-x3-dex |

## Missing Features
| Feature | Missing Code |
|---|---|
| x3.x3lang_contract_templates | templates/src/ |

