# Phase 5 Quick Reference: Deployment & Verification

**Status:** ✅ REMEDIATION COMPLETE  
**Last Updated:** 2026-04-26  
**Next:** Execute and validate fixes

---

## 🚀 One-Command Launch

```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR
bash PHASE_5_COMPLETE_LAUNCHER.sh
```

This single command will:
- ✅ Start settlement flow tests (Phase 5a)
- ✅ Build and deploy indexer (Phase 5b)
- ✅ Trigger validator network (Phase 5c)
- ✅ Monitor consensus and generate report

---

## 📊 What Gets Started

| Component | Command | Port | Log File | Status |
|-----------|---------|------|----------|--------|
| **Settlement Tests** | Python script | N/A | `settlement-tests.log` | Process tracked |
| **Indexer** | x3-indexer binary | 4000 | `indexer.log` | Verified + health-checked |
| **Validators** | x3-chain-node | 30333-30335 | `validator1-3.log` | Process verified |

---

## 🔍 Real-Time Monitoring

**In Terminal 1 - Run launcher:**
```bash
bash PHASE_5_COMPLETE_LAUNCHER.sh
```

**In Terminal 2 - Watch all logs:**
```bash
tail -f /tmp/x3-testnet-logs/{settlement-tests,indexer,validator1}.log
```

**In Terminal 3 - Check processes:**
```bash
watch -n 2 'ps aux | grep -E "x3-indexer|x3-chain-node|p4_p5" | grep -v grep'
```

---

## ✅ Quick Validation Checklist

After running launcher, verify with:

### Settlement Tests
```bash
grep -E "test result:|passed|failed" /tmp/x3-testnet-logs/settlement-tests.log
```
**Expected:** Test results with pass/fail counts

### Indexer Status
```bash
pgrep -f "x3-indexer" && echo "✅ Running" || echo "❌ Not running"
curl -s http://127.0.0.1:4000/graphql -X POST -H "Content-Type: application/json" -d '{"query":"{ __typename }"}' | head -20
```
**Expected:** Process exists and GraphQL responds

### Validators Running
```bash
ps aux | grep "x3-chain-node.*validator" | grep -v grep | wc -l
```
**Expected:** 3 validators running

### RPC Ports Responding
```bash
for port in 9933 9934 9935; do
  curl -s http://127.0.0.1:$port -X POST -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"system_health","params":[],"id":1}' | grep -q "result" && echo "Port $port: ✅" || echo "Port $port: ❌"
done
```
**Expected:** All three ports responding

---

## 🛠️ Manual Component Testing

### Test 1: Manual Settlement Tests

```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR/tests_phase4

python3 -u p4_p5_production_release.py \
  --validators 3 \
  --testnet-enabled \
  --verbose 2>&1 | tee settlement-manual.log
```

### Test 2: Manual Indexer Start

```bash
# In Terminal A:
/home/lojak/Desktop/X3_ATOMIC_STAR/target/release/x3-indexer \
  --listen 0.0.0.0:4000 \
  --rpc-urls http://127.0.0.1:9933 http://127.0.0.1:9934 http://127.0.0.1:9935

# In Terminal B:
curl http://127.0.0.1:4000/graphql -X POST \
  -H "Content-Type: application/json" \
  -d '{"query":"{ __typename }"}'
```

### Test 3: Manual Validator Network

```bash
# Start validators manually
bash scripts/start-validator-network.sh

# Or individually:
bash scripts/bootstrap-validator.sh 1 30333
bash scripts/bootstrap-validator.sh 2 30334
bash scripts/bootstrap-validator.sh 3 30335
```

---

## 🔧 Key Fixes Applied

### 1. Binary Path Correction
```bash
# Before (BROKEN):
./crates/x3-indexer/target/release/x3-indexer

# After (FIXED):
$WORKSPACE/target/release/x3-indexer
```

### 2. Process Verification
```bash
# Before (FALSE POSITIVE):
... & INDEXER_PID=$!
echo "✅ Indexer deployed"  # Reports success even if it crashes

# After (HONEST):
... & INDEXER_PID=$!
if verify_process_started $INDEXER_PID "Indexer"; then
  echo "✅ Indexer deployed"
else
  echo "❌ Indexer failed"
  exit 1
fi
```

### 3. Safe Arithmetic
```bash
# Before (CRASH):
PERCENTAGE=$((PASSED * 100 / TOTAL))  # Crashes if TOTAL=0

# After (SAFE):
if [ "$TOTAL" -gt 0 ]; then
  PERCENTAGE=$((PASSED * 100 / TOTAL))
else
  PERCENTAGE=0
fi
```

---

## 🚨 Troubleshooting: Quick Fixes

| Problem | Quick Diagnosis | Quick Fix |
|---------|-----------------|-----------|
| "Indexer not found" | `ls -lh target/release/x3-indexer` | `cd crates/x3-indexer && cargo build --release` |
| "Port already in use" | `lsof -i :4000` | `pkill -f x3-indexer` and retry |
| "Settlement tests not logging" | `cat /tmp/x3-testnet-logs/settlement-tests.log` | Run `python3 -u` with manual test |
| "Validators not connecting" | `ps aux \| grep validator` | Run `bash scripts/start-validator-network.sh` |
| "RPC not responding" | `curl http://127.0.0.1:9933 -X POST ...` | Check validator logs for errors |

---

## 📈 Success Metrics

**Phase 5 is WORKING when you see:**

✅ Settlement tests output  
✅ Indexer GraphQL endpoint responding  
✅ 3 validators running with block production  
✅ No errors in any logs  
✅ All processes stayed running for 30+ seconds  

**Phase 5 has ISSUES when you see:**

❌ Processes crash immediately  
❌ Logs empty or show errors  
❌ False success reports  
❌ Ports blocked  
❌ Out of memory/disk errors  

---

## 📂 File Reference

| File | Purpose | Status |
|------|---------|--------|
| `PHASE_5_COMPLETE_LAUNCHER.sh` | Main Phase 5 orchestrator | ✅ FIXED |
| `scripts/bootstrap-validator.sh` | Single validator startup | ✅ NEW |
| `scripts/start-validator-network.sh` | Multi-validator coordinator | ✅ NEW |
| `PHASE_5_REMEDIATION_GUIDE.md` | Detailed documentation | ✅ NEW |
| `PHASE_5_LAUNCHER_EXECUTION_SUMMARY.md` | Original issue report | Reference |

---

## 🎯 Next Actions

### Immediate (Now)
1. Run: `bash PHASE_5_COMPLETE_LAUNCHER.sh`
2. Monitor: `tail -f /tmp/x3-testnet-logs/*.log`
3. Verify: Check all processes running

### If Issues
1. Check relevant log file
2. Look up issue in troubleshooting section
3. Run appropriate diagnostic command
4. Apply fix or escalate

### When Working
1. Document any new issues
2. Update PHASE_5_REMEDIATION_GUIDE.md
3. Prepare for production hardening

---

## 🔗 Related Commands

```bash
# View all Phase 5 logs at once
ls -lh /tmp/x3-testnet-logs/ && echo "---" && wc -l /tmp/x3-testnet-logs/*

# Kill all Phase 5 processes
pkill -f "p4_p5_production_release|x3-indexer|x3-chain-node.*validator"

# Clean Phase 5 workspace
rm -rf /tmp/x3-validator-{1,2,3}
rm -f /tmp/x3-testnet-logs/*
rmdir /tmp/x3-testnet-logs

# Rebuild everything from scratch
cd /home/lojak/Desktop/X3_ATOMIC_STAR && cargo clean && cargo build --release

# Check workspace status
cd /home/lojak/Desktop/X3_ATOMIC_STAR && cargo check
```

---

**Ready to Launch Phase 5! 🚀**
