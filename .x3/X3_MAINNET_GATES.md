# X3 Mainnet Gates

X3 is not mainnet-ready unless all gates pass.

## P0 Gates

### Runtime
- cargo check passes
- cargo test passes
- cargo clippy passes
- cargo fmt passes
- no consensus-critical unwrap/panic
- runtime upgrade rehearsal exists
- chain spec generated and reviewed

### Universal Asset Kernel
- canonical supply invariant exists
- native + evm + svm + external_locked + pending accounting tested
- overflow/precision tests exist
- rollback tests exist

### Cross-VM Atomic Execution
- EVM/SVM/X3VM execution paths tested
- atomic commit/rollback tested
- replay protection tested
- expiry/deadline tested
- domain separation tested

### Bridge / Router
- bridge enablement audit gate exists
- nonce uniqueness tested
- message expiry tested
- replay tests exist
- external bridge disabled by default until audited

### DEX / Launchpad
- swap tests
- liquidity lock tests
- anti-rug tests
- fee accounting tests
- slippage and TWAP tests

### Security
- no TODO/FIXME in P0 paths
- no hardcoded mock values in production paths
- weak randomness blocked
- unsafe code reviewed
- panic/unwrap audit clean or justified

### Ops
- testnet launch checklist complete
- genesis review complete
- monitoring plan exists
- rollback plan exists
- validator bootstrap documented
