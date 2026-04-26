# 🎯 X3_ATOMIC_STAR Quick Command Reference

**Print this out!** Essential commands for testnet deployment.

---

## 📂 Navigation

```bash
# Go to project
cd /home/lojak/Desktop/X3_ATOMIC_STAR

# Show directory tree
tree -L 2 -I 'target|.git'

# Check disk space
du -sh .
du -sh target/
```

---

## 🔨 BUILD

```bash
# Build core node (release)
cargo build --release -p x3-chain-node

# Build with GPU acceleration
cargo build --release -p x3-chain-node --features gpu-validator

# Build with debug info
cargo build -p x3-chain-node

# Clean builds
cargo clean
```

---

## 🧪 TEST

```bash
# Run Phase 4 tests (65 tests)
cargo test --lib tests_phase4 -- --nocapture

# Run settlement tests only
cargo test --lib x3_settlement_engine

# Run routing tests only
cargo test --lib x3_cross_vm_router

# Run ALL tests (comprehensive)
cargo test --lib

# Run tests with logging
RUST_LOG=debug cargo test --lib tests_phase4 -- --nocapture

# Run single-threaded (helpful for debugging)
cargo test --lib tests_phase4 -- --test-threads=1
```

---

## 🚀 RUN TESTNET

```bash
# Development chain (quickest)
./target/release/x3-chain-node --chain dev --tmp

# With RPC exposed
./target/release/x3-chain-node --chain dev --rpc-external --ws-external

# On custom ports
./target/release/x3-chain-node --chain dev --rpc-port 9934 --ws-port 9945

# With debug logging
RUST_LOG=debug ./target/release/x3-chain-node --chain dev

# Multi-validator (once keys generated)
./target/release/x3-chain-node --chain testnet/chain-spec.json --validator
```

---

## 💾 UTILITIES

```bash
# Check Rust version
rustc --version

# Update Rust
rustup update

# Show workspace members
cargo metadata --format-version 1 | jq -r '.workspace_members[] | .name' | sort

# Check dependencies
cargo tree -p x3-chain-node

# Verify Solana compatibility
cargo tree -p solana-pubkey@4.2.0

# Show available features
cargo metadata --format-version 1 | jq '.packages[] | select(.name=="x3-chain-node") | .features'
```

---

## 🌐 RPC COMMANDS (After Node is Running)

```bash
# Check node health
curl http://localhost:9933 -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"system_health","params":[],"id":1}' | jq

# Check sync status
curl http://localhost:9933 -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"system_syncState","params":[],"id":1}' | jq

# Get chain info
curl http://localhost:9933 -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"system_chain","params":[],"id":1}' | jq

# Get latest block
curl http://localhost:9933 -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"chain_getHeader","params":[],"id":1}' | jq

# Get account info
curl http://localhost:9933 -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"state_getStorage","params":["0x..."],"id":1}' | jq
```

---

## 📝 LOG COMMANDS

```bash
# Follow logs in real-time
./target/release/x3-chain-node --chain dev 2>&1 | tail -f

# Grep for specific events
./target/release/x3-chain-node --chain dev 2>&1 | grep -i "settlement\|finalized"

# Save logs to file
./target/release/x3-chain-node --chain dev &> testnet.log &
tail -f testnet.log

# Analyze logs
grep "ERROR\|WARN" testnet.log
grep "settlement" testnet.log
grep "finalized" testnet.log
```

---

## 🔧 CONFIGURATION

```bash
# Generate chain spec
./deployment/key-gen-testnet.sh --validator-count 3

# Show config
cat testnet/chain-spec.json | jq .

# Validate chain spec
./target/release/x3-chain-node --chain testnet/chain-spec.json build-spec --raw
```

---

## 🐛 DEBUG / TROUBLESHOOT

```bash
# Check if ports are in use
lsof -i :9933
lsof -i :9944

# Kill process on port
kill -9 $(lsof -t -i :9933)

# View system resource usage
top -p $(pgrep -f x3-chain-node)

# Check compilation errors
cargo check --workspace

# Clean rebuild
cargo clean && cargo build --release -p x3-chain-node

# Get detailed error info
cargo build -p x3-chain-node 2>&1 | grep -A 10 "error"
```

---

## 📊 PERFORMANCE

```bash
# Measure build time
time cargo build --release -p x3-chain-node

# Run benchmarks
cargo bench --lib

# Profile node
perf record -F 99 ./target/release/x3-chain-node --chain dev &
sleep 30 && kill %1
perf report
```

---

## 🔒 SECURITY CHECKS

```bash
# Audit dependencies
cargo audit

# Check for security warnings
cargo clippy -- -W warnings

# Verify no secrets in code
grep -r "private_key\|SECRET\|PASSWORD" --include="*.rs" .
grep -r "sk_\|secret" --include="*.rs" . | grep -v "test\|example"
```

---

## 💬 QUICK HELP

```bash
# Show all cargo commands
cargo --help

# Show specific command help
cargo build --help
cargo test --help
cargo run --help

# List available features
cargo build --release -p x3-chain-node --list-features

# Show workspace info
cargo info

# Get system info
uname -a
lscpu
free -h
```

---

## 🎯 COMMON WORKFLOWS

### Quick Test (2 min)
```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR
cargo test --lib tests_phase4
```

### Quick Build (30-60 min)
```bash
cargo build --release -p x3-chain-node
```

### Launch Testnet (1 min setup)
```bash
./target/release/x3-chain-node --chain dev --rpc-external --ws-external
# Then access http://localhost:9933 or ws://localhost:9944
```

### Full Deployment Flow
```bash
cargo build --release -p x3-chain-node    # Build
cargo test --lib tests_phase4             # Test
./deployment/key-gen-testnet.sh           # Generate keys
./deployment/deploy-testnet.sh            # Deploy
# Monitor: tail -f testnet.log
```

---

## 📞 Emergency Commands

```bash
# If node is frozen
pkill -9 x3-chain-node

# If port is stuck
lsof -i :9933 | grep -v PID | awk '{print $2}' | xargs kill -9

# Full system reset
cargo clean
rm -rf testnet/
rm -rf logs/
cargo build --release -p x3-chain-node
```

---

**Version:** 1.0 | **Date:** 2026-04-24 | **Status:** ✅ Ready for Use
