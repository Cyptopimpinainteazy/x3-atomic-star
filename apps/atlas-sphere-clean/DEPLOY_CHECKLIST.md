# X3 Chain Testnet Deployment Checklist

Use this checklist to deploy X3 Chain and run the real X3 kernel benchmark.

## Pre-Deployment

- [ ] Read `TESTNET_READINESS_SUMMARY.md` for overview
- [ ] Read `BENCHMARK_GUIDE.md` for detailed deployment steps
- [ ] Verify system has: Rust 1.70+, Node.js 18+, Python 3.8+
- [ ] Clone/pull latest x3-chain-master repo with patches applied
- [ ] Verify patches are in place:
  - [ ] `runtime/src/lib.rs:746` — No mock EVM fallback
  - [ ] `node/src/rpc_frontier.rs` — Functions renamed to `create_frontier_rpc()`
  - [ ] `scripts/run-validator.sh` — No `CCGV_USE_MOCK_RPC=true` export

## Build & Verification

- [ ] Build release node: `cargo build -p x3-chain-node --release` (takes ~5 min)
- [ ] Verify build succeeds with no errors
- [ ] Binary created at: `target/release/x3-chain-node` (check filesize ~150MB)
- [ ] Install Node.js deps: `cd scripts/testnet && npm install`
- [ ] Verify npm packages installed: check `node_modules/@polkadot/api`

## Local Chain Deployment

- [ ] Start validator node:
  ```bash
  ./target/release/x3-chain-node \
    --chain local \
    --validator \
    --tmp \
    --rpc-port 9944 \
    --ws-external
  ```
- [ ] Verify chain starts (watch for "Alice" authority logs)
- [ ] Verify RPC is listening: `curl -s http://127.0.0.1:9944`
- [ ] Let chain run for ~30 sec to stabilize
- [ ] Keep chain running in background terminal

## Account Authorization

- [ ] Run authorization script:
  ```bash
  cd scripts/testnet
  node authorize-accounts.js \
    --wsEndpoint ws://127.0.0.1:9944 \
    --baseDerivation //Alice//load \
    --count 240 \
    --sudoSeed //Alice
  ```
- [ ] Verify all 240 accounts authorized (check for ✓ messages)
- [ ] Wait for finalization to complete

## Benchmark Execution

### Option A: Dev/Debug Run (Fast, Low Load)

- [ ] Run single-process benchmark:
  ```bash
  node load-x3-comit-v2-tps.js \
    --wsEndpoint ws://127.0.0.1:9944 \
    --numSenders 16 \
    --concurrency 64 \
    --durationSec 120 \
    --finalityWaitSec 30 \
    --verbose
  ```
- [ ] Watch output for:
  - [ ] "Starting submission phase..."
  - [ ] "Blocks produced..." (should see ~600 blocks in 120 sec)
  - [ ] "Finality complete..."
  - [ ] Final metrics table
- [ ] Expected result: 50–500 finalized TPS (depending on system)
- [ ] Save output or redirect to file: `... > bench_dev.log 2>&1`

### Option B: Production Run (Full Load, Long Duration)

- [ ] Run multiprocess benchmark:
  ```bash
  python3 run-multiprocess-load.py \
    --rpc-ws ws://127.0.0.1:9944 \
    --workers 8 \
    --senders 240 \
    --duration-sec 600 \
    --finality-wait-sec 45 \
    --concurrency-total 1024 \
    --output benchmarks/x3_chain_baseline_tps.json
  ```
- [ ] Monitor output for:
  - [ ] "Prefund stage completed"
  - [ ] "Worker 1..8 starting..."
  - [ ] Progress percentage increasing
  - [ ] Final JSON output with metrics
- [ ] Wait for full duration (600 sec = 10 minutes)
- [ ] Output saved to: `benchmarks/x3_chain_baseline_tps.json`
- [ ] View results: `jq . benchmarks/x3_chain_baseline_tps.json`

## Results Analysis

- [ ] Open benchmark output JSON
- [ ] Extract key metrics:
  - [ ] `finalized_tps_submit_window` — Primary metric
  - [ ] `in_block_tps` — In-flight metric
  - [ ] `avg_block_time_ms` — Should be ~200
  - [ ] `avg_signed_extrinsics_per_block` — Load distribution
  - [ ] `failure_reasons` — Check for unexpected errors
- [ ] Fill in `BENCHMARK_COMPARISON.md` template:
  - [ ] Paste benchmark command
  - [ ] Paste finalized TPS result
  - [ ] Record block time
  - [ ] Record test duration and sender count
- [ ] Compare against known benchmarks:
  - [ ] Solana non-vote TPS (4,000–10,000 typical)
  - [ ] Ethereum L1 TPS (~15–25 typical)
  - [ ] Polkadot parachain TPS (1,000–3,000 typical)

## Troubleshooting

### Chain Won't Start
- [ ] Check port 9944 is free: `lsof -i :9944`
- [ ] Kill any blocking process: `pkill x3-chain-node`
- [ ] Try again

### Low Finalized TPS
- [ ] Check block time in logs (should be ~200ms)
- [ ] Increase `--finalityWaitSec` by 10 sec and re-run
- [ ] Check chain logs for adapter errors (search for "error\|panic")
- [ ] Try dev run first to isolate issues

### Authorization Fails
- [ ] Verify chain is running
- [ ] Verify Alice account exists (should auto-exist on local chain)
- [ ] Try authorizing manually via Polkadot.js UI
- [ ] Check chain logs for sudo errors

### Benchmark Hangs on Finality
- [ ] May be normal if finality_wait_sec is high
- [ ] Kill with Ctrl+C after 5+ minutes if stuck
- [ ] Check if blocks are still being produced in chain logs
- [ ] Increase finality_wait_sec or reduce benchmark duration

## Documentation & Next Steps

- [ ] Review benchmark results against BENCHMARK_COMPARISON.md expectations
- [ ] Identify any bottlenecks:
  - [ ] Is finalized TPS < 100? Check block weights, adapters
  - [ ] Is error rate > 5%? Check rate limits, increase signers
  - [ ] Is block time > 250ms? Check chain load, CPU utilization
- [ ] If tuning is needed:
  - [ ] Increase `MILLISECS_PER_BLOCK` or `BLOCK_WEIGHT_LIMIT` in `runtime/src/lib.rs`
  - [ ] Rebuild and re-run benchmark
  - [ ] Document performance improvements
- [ ] Publish results:
  - [ ] Copy JSON output to report
  - [ ] Add notes on any issues or tuning applied
  - [ ] Compare finalized TPS against other chains
  - [ ] Share findings with team

## Sign-Off

- [ ] Deployment completed successfully
- [ ] Benchmark executed and results collected
- [ ] Results analyzed and documented
- [ ] Comparison against known benchmarks completed
- [ ] Testnet approved for public deployment

**Completed By:** _________________  
**Date:** _________________  
**Result Status:** ✅ PASS / ⚠️ NEEDS TUNING / ❌ FAILED
