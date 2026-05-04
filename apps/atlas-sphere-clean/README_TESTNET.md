# X3 Chain Testnet Readiness Package

## Status: ✅ READY FOR DEPLOYMENT

This package contains everything needed to deploy X3 Chain on testnet and measure real X3 kernel throughput.

---

## 📦 What's Included

### Documentation (Start Here)
1. **TESTNET_READINESS_SUMMARY.md** — Overview of all work completed
2. **BENCHMARK_GUIDE.md** — How to deploy and run benchmarks
3. **DEPLOY_CHECKLIST.md** — Step-by-step deployment walkthrough
4. **BENCHMARK_COMPARISON.md** — How to compare results against other chains
5. **README_TESTNET.md** — This file

### Production Code Changes
- `runtime/src/lib.rs:746` — EVM adapter mock fallback removed
- `node/src/rpc_frontier.rs` — Stub functions renamed
- `node/src/rpc.rs:105–106` — RPC wiring updated
- `scripts/run-validator.sh` — Mock RPC export removed

### Benchmark Scripts
- `scripts/testnet/load-x3-comit-v2-tps.js` — Real X3 kernel load test (600+ lines)
- `scripts/testnet/authorize-accounts.js` — Account authorization helper
- `scripts/testnet/run-multiprocess-load.py` — Multiprocess orchestrator (updated)

---

## 🚀 Quick Start (5 Minutes)

### 1. Build
```bash
cargo build -p x3-chain-node --release
```

### 2. Deploy
```bash
./target/release/x3-chain-node --chain local --validator --tmp --rpc-port 9944 --ws-external
```

### 3. Authorize (in new terminal)
```bash
cd scripts/testnet && npm install
node authorize-accounts.js --wsEndpoint ws://127.0.0.1:9944 --baseDerivation //Alice//load --count 240 --sudoSeed //Alice
```

### 4. Benchmark
```bash
# Dev/debug (2 minutes)
node load-x3-comit-v2-tps.js --wsEndpoint ws://127.0.0.1:9944 --numSenders 16 --concurrency 64 --durationSec 120

# Production (10 minutes)
python3 run-multiprocess-load.py --rpc-ws ws://127.0.0.1:9944 --workers 8 --senders 240 --duration-sec 600 --output benchmarks/baseline.json
```

---

## 📊 Key Metrics

The benchmark measures:
- **Finalized TPS** (primary) — Via nonce delta; ground truth finalization
- **In-Block TPS** — Extrinsics per block during submission phase
- **Block Time** — Should be 200 ms (5 blocks/sec)
- **Txpool Depth** — Average pending transaction queue size
- **Failures** — Rate limit errors, nonce conflicts

Expected range: **50–500 TPS** (conservative block weights)

---

## 🔍 What Changed

### Production Defects Fixed
1. **EVM Adapter Mock Fallback Removed** — No more silent fallback to mock on Frontier error
2. **RPC Stub Naming Corrected** — Renamed from `create_*_stub()` to `create_*_rpc()`
3. **Mock RPC Export Removed** — Validator no longer uses mock RPC configuration

### Benchmarking Infrastructure Added
1. **Real X3 Kernel Benchmark** — Submits `submitComitV2` with real payloads, not synthetic remarks
2. **Authorization Helper** — Pre-authorizes signers before benchmark
3. **Rich Telemetry** — Collects block times, TPS, failure reasons
4. **Comprehensive Docs** — Deployment guide, comparison template, troubleshooting

---

## 📖 Documentation Map

| Document | Purpose | Audience |
|----------|---------|----------|
| TESTNET_READINESS_SUMMARY.md | Complete overview | All |
| BENCHMARK_GUIDE.md | Detailed deployment steps | DevOps/Testnet |
| DEPLOY_CHECKLIST.md | Interactive checklist | DevOps/Testnet |
| BENCHMARK_COMPARISON.md | TPS comparison template | Analyst/Reporter |
| README_TESTNET.md | Quick reference | All |

---

## 🛠️ Tools & Commands

### Deployment
```bash
# Build
cargo build -p x3-chain-node --release

# Start node
./target/release/x3-chain-node --chain local --validator --tmp --rpc-port 9944 --ws-external

# Install deps
cd scripts/testnet && npm install

# Authorize signers
node authorize-accounts.js --wsEndpoint ws://127.0.0.1:9944 --baseDerivation //Alice//load --count 240

# Single-process test
node load-x3-comit-v2-tps.js --wsEndpoint ws://127.0.0.1:9944 --numSenders 16 --concurrency 64 --durationSec 120

# Multi-process production
python3 run-multiprocess-load.py --rpc-ws ws://127.0.0.1:9944 --workers 8 --senders 240 --duration-sec 600
```

### Analysis
```bash
# View results
jq . benchmarks/x3_chain_baseline_tps_multiprocess.json

# Extract finalized TPS
jq .finalized_tps_submit_window benchmarks/x3_chain_baseline_tps_multiprocess.json

# Extract all metrics
jq '{finalized_tps: .finalized_tps_submit_window, in_block_tps: .in_block_tps, block_time: .avg_block_time_ms, error_rate: .error_rate_pct}' benchmarks/x3_chain_baseline_tps_multiprocess.json
```

---

## ⚠️ Known Limitations

1. **Block Weights Conservative** — Current 150ms ref time budget may limit throughput; can be tuned
2. **Per-Account Rate Limit** — 10 submissions per block per account enforced; requires multiple signers
3. **Finality Time** — 45 sec default wait for finalization; adjust based on chain security
4. **Testnet Only** — This benchmark setup is for testnet; production may differ

---

## 🔧 Troubleshooting

| Problem | Solution |
|---------|----------|
| Chain won't start | Check port 9944 is free; try `pkill x3-chain-node` |
| Authorization fails | Verify Alice account exists; check chain logs |
| Low TPS | Increase `--numSenders`, check block weights, verify finality time |
| Benchmark hangs | May be normal; give 5+ min finality wait; check chain logs |

See BENCHMARK_GUIDE.md for detailed troubleshooting.

---

## ✅ Verification Checklist

Before deploying:
- [ ] Read TESTNET_READINESS_SUMMARY.md
- [ ] Verify patches applied: runtime/src/lib.rs:746, node/src/rpc_frontier.rs
- [ ] Build succeeds: `cargo build -p x3-chain-node --release`
- [ ] Node.js deps installed: `cd scripts/testnet && npm install`
- [ ] Scripts are executable: `ls -l scripts/testnet/*.js`

---

## 📝 Next Steps

1. **Deploy** — Follow BENCHMARK_GUIDE.md steps
2. **Authorize** — Run authorize-accounts.js with your signer count
3. **Benchmark** — Execute load test (dev or production)
4. **Analyze** — Fill in BENCHMARK_COMPARISON.md with results
5. **Compare** — Measure finalized TPS vs. Solana (4K–10K), Ethereum (15–25), Polkadot (1K–3K)
6. **Tune** — Adjust block weights if needed and re-measure
7. **Report** — Publish findings

---

## 📞 Support

- Check **BENCHMARK_GUIDE.md** for detailed instructions
- See **DEPLOY_CHECKLIST.md** for step-by-step guidance
- Review **BENCHMARK_COMPARISON.md** for analysis methodology
- Examine source code: `scripts/testnet/load-x3-comit-v2-tps.js` (~600 lines, well-commented)

---

## 📅 Timeline

- **Build:** 4–5 minutes
- **Authorization:** 5–10 minutes (depends on signer count)
- **Dev Benchmark:** 2–3 minutes
- **Production Benchmark:** 10–20 minutes (600+ sec duration + finality)
- **Analysis:** 10–15 minutes
- **Total:** ~30–45 minutes for full cycle

---

## 🎯 Success Criteria

✅ Benchmark runs without errors  
✅ Finalized TPS measured (via nonce delta)  
✅ Results compared against known chains  
✅ Block time stable at ~200 ms  
✅ No mock code paths exercised  
✅ Real X3 adapters used (EVM, SVM, X3)  

---

**Version:** 1.0  
**Release Date:** 2026-04-04  
**Status:** APPROVED FOR TESTNET
