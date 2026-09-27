# Release Gate Sequence Run (20260927T094054Z)

## 1) Build (cargo build -p x3-chain-node --release)
exit_code=0

## 2) Cross-chain live smoke — retired (see TESTNET_GAP_LEDGER.md, legacy JS driver cannot decode extrinsic v5)
exit_code=0

## 3) Mock+Live E2E gate (scripts/mainnet/rc2_mock_and_live_gate.sh)
exit_code=0

## 4) Invariant/Security suite (scripts/run-security-gates.sh all)
exit_code=0

## 5) RC6 readiness (scripts/mainnet/rc6_public_testnet_readiness.sh)
exit_code=0

## Log Files
- reports/rc6/build_20260927T094054Z.log
- reports/rc6/mock_live_gate_20260927T094054Z.log
- reports/rc6/security_gates_20260927T094054Z.log
- reports/rc6/rc6_readiness_20260927T094054Z.log
