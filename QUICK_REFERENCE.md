# X3 Substrate Tools — Quick Reference Card

**All tools installed & verified: April 28, 2026**

---

## 🚀 One-Line Quick Starts

```bash
# Start local dev node
./target/release/x3-chain-node --dev --tmp

# Check runtime migrations (in another terminal)
try-runtime --runtime target/release/wbuild/x3-chain-runtime/x3_chain_runtime.compact.compressed.wasm on-runtime-upgrade live --uri ws://127.0.0.1:9944

# Spin up test network
zombienet spawn x3-network-config.toml

# Fork chain locally
chopsticks --endpoint wss://x3.example.com --db /tmp/x3-fork.db

# Generate pallet weights
cargo run -p x3-chain-node --features runtime-benchmarks -- benchmark pallet --pallet pallet-x3-kernel --steps 50 --repeat 20

# Build deterministically
srtool build --profile release

# Run full pre-testnet validation
./x3-testnet-validation.sh all
```

---

## 📋 Tool Versions & Paths

```
try-runtime ..................... /home/lojak/.cargo/bin/try-runtime (0.42.0)
zombienet ........................ /home/lojak/.local/bin/zombienet (1.3.138)
chopsticks ....................... /home/lojak/.local/bin/chopsticks (1.2.8)
srtool ........................... /home/lojak/.cargo/bin/srtool (0.13.2)
FRAME benchmarking ............... Integrated in x3-chain-node
```

---

## ⏱️ Estimated Runtimes

| Task | Time | Phase |
|------|------|-------|
| Workspace format + lint + check | 8-12 min | 1 |
| Unit + integration tests | 5-10 min | 1 |
| Runtime build (benchmarks) | 15-20 min | 2 |
| Single pallet benchmark (50/20) | 3-5 min | 2 |
| Zombienet spawn (3 validators) | 2-3 min | 3 |
| srtool build (dry-run) | 2-3 min | 4 |
| **Full suite (all phases)** | **~1 hour** | all |

---

## 🔗 Pre-Testnet Validation Chain

```
PHASE 1: Build & Compile (2 hrs)
  ↓ Pass/Fail: cargo fmt, clippy, check, test
  ↓
PHASE 2: Runtime Safety (3 hrs)
  ↓ Pass/Fail: try-runtime, benchmarks, weights
  ↓
PHASE 3: Network Testing (4 hrs)
  ↓ Pass/Fail: Zombienet, E2E, consensus
  ↓
PHASE 4: Release Build (1 hr)
  ↓ Pass/Fail: srtool, checksums, reproducibility
  ↓
✅ READY FOR TESTNET
```

---

## 🎯 Critical Pallets to Benchmark

```
1. pallet-x3-kernel ...................... Core bundle execution
2. pallet-x3-atomic-kernel ............... Atomic operations
3. pallet-x3-supply-ledger ............... Supply accounting (check TODOs!)
4. pallet-x3-cross-vm-router ............. Message routing
5. pallet-x3-asset-registry .............. Asset tracking
6. pallet-x3-settlement-engine ........... Settlement finalization (FIXED dependency)
```

---

## 🚦 Status Indicators

```
✅ try-runtime works ..................... try-runtime --help
✅ Zombienet ready ....................... zombienet version
✅ Chopsticks operational ................ chopsticks --help
✅ srtool available ...................... srtool --version
✅ Benchmarking wired .................... cargo run -p x3-chain-node --features runtime-benchmarks -- benchmark pallet --list
```

---

## 📊 Key Metrics to Track

| Metric | Target | Current |
|--------|--------|---------|
| Workspace build time | <30 min | TBD |
| Test suite time | <15 min | TBD |
| Runtime benchmark (50/20) | <300s per pallet | TBD |
| Node startup time | <10s | TBD |
| Validator finality | <30 blocks | TBD |
| Storage state root match | 100% | TBD |

---

## 🔨 Troubleshooting

| Issue | Fix |
|-------|-----|
| Build hangs (file lock) | `killall cargo` |
| try-runtime not found | Already installed at `/home/lojak/.cargo/bin/try-runtime` |
| Zombienet needs Docker | `sudo apt install podman` |
| Benchmarking timeout | Reduce `--steps` to 10, `--repeat` to 3 |
| srtool offline | Use `--dry-run` flag |

---

## 📁 Key Files

```
SUBSTRATE_TOOLS_SETUP.md ........... Comprehensive reference guide (12 KB)
TOOLS_INSTALLATION_COMPLETE.md .... Installation summary (9 KB)
x3-testnet-validation.sh .......... Automated 4-phase validator (8 KB, executable)
pallets/x3-settlement-engine/Cargo.toml .. FIXED (frame-benchmarking added)
```

---

## 🎓 Next Steps

1. **Now:** Read `SUBSTRATE_TOOLS_SETUP.md`
2. **Today:** Run `./x3-testnet-validation.sh 1` (build phase)
3. **This week:** Collect benchmarks for all 6 pallets
4. **Before testnet:** Run full `./x3-testnet-validation.sh all`

---

## 📞 Documentation Links

- Full Guide: `./SUBSTRATE_TOOLS_SETUP.md`
- Installation Report: `./TOOLS_INSTALLATION_COMPLETE.md`
- Automation Script: `./x3-testnet-validation.sh`
- Ship Plan: `./deep-research-report (1).md`

---

**Ready for action! 🚀**

Run: `./x3-testnet-validation.sh 1` to start Phase 1 validation now.
