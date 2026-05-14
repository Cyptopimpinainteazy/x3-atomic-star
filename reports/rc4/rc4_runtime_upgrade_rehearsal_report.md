# RC4 Runtime Upgrade Rehearsal Report

## Verdict

FAIL

## Scope

- local3 live runtime upgrade rehearsal
- old runtime -> new runtime
- internal settlement only
- external bridges disabled

## Refs

| Field | Value |
|---|---|
| OLD_REF | x3-atomic-star-rc2-internal-settlement-smoke |
| NEW_REF | HEAD |
| Old commit | 3f03f019b0686ba9450692e9824b1b1dd6fd0a95 |
| New commit | 4a5a7c7d38bac89293c66e062b794c125a905ddf |

## Step Results

| Check | Result | Detail |
|---|---:|---|
| build_old_node | PASS | using existing old binary /home/lojak/Desktop/X3_ATOMIC_STAR/target/rc2-current-node/debug/x3-chain-node |
| build_old_runtime_wasm | PASS | /home/lojak/Desktop/X3_ATOMIC_STAR/target/rc2-current-node/debug/wbuild/x3-chain-runtime/x3_chain_runtime.wasm |
| build_new_node | PASS | using existing current binary /home/lojak/Desktop/X3_ATOMIC_STAR/target/rc4-current/debug/x3-chain-node (BUILD_CURRENT=0) |
| build_new_runtime_wasm | PASS | /home/lojak/Desktop/X3_ATOMIC_STAR/target/rc4-current/debug/wbuild/x3-chain-runtime/x3_chain_runtime.wasm |
| local3_boot | PASS | /ip4/127.0.0.1/tcp/30543/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp |
| pre_upgrade_blocks | PASS | advanced |
| pre_upgrade_finality | PASS | finalized head advanced |
| pre_upgrade_settlement | PASS | test_x3_native_evm_svm_roundtrip_preserves_supply |
| runtime_upgrade_submission | FAIL | live setCode/governance upgrade extrinsic rejected; WASM payload 6802200 bytes exceeds old runtime block length cap 5242880 bytes (preimage max 4194304 bytes); see /home/lojak/Desktop/X3_ATOMIC_STAR/reports/rc4/runtime_upgrade_submission.json |
| post_upgrade_settlement | NOT_RUN | not run because live runtime upgrade did not pass |
| post_upgrade_refund_halt | NOT_RUN | not run because live runtime upgrade did not pass |
| post_upgrade_halt_reject | NOT_RUN | not run because live runtime upgrade did not pass |

## Blockers

- runtime_upgrade_submission: live setCode/governance upgrade extrinsic rejected; WASM payload 6802200 bytes exceeds old runtime block length cap 5242880 bytes (preimage max 4194304 bytes); see /home/lojak/Desktop/X3_ATOMIC_STAR/reports/rc4/runtime_upgrade_submission.json
- post_upgrade_settlement: not run because live runtime upgrade did not pass
- post_upgrade_refund_halt: not run because live runtime upgrade did not pass
- post_upgrade_halt_reject: not run because live runtime upgrade did not pass

## Generated

2026-05-14T10:24:55Z
