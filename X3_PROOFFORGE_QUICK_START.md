# X3 ProofForge — QUICK START GUIDE

**For:** Developers | Validators | Security Teams | Operations  
**Status:** ✅ READY TO USE  
**Last Updated:** 2026-04-26

---

## 🚀 30-Second Overview

**ProofForge** is the X3 blockchain's executable proof system. It verifies EVERY claim about X3 through automated tests, supply chain audits, security scanning, and formal proofs.

**Core Philosophy:**
- No green badge without evidence
- Tests catch the bad code BEFORE it ships
- Mainnet only when ALL proofs pass
- Every proof produces a receipt

---

## 📋 MOST IMPORTANT COMMANDS

### For Developers (Day-to-Day)

**Before committing code:**
```bash
x3-proof verify --strict
```
Runs: compile + lint + unit tests + mutation tests

**Before requesting merge:**
```bash
x3-proof hack --strict
```
Runs: security tests + adversarial tests + secret scan

**If you're working on edge cases:**
```bash
x3-proof edgecase --all --chaos
```
Runs: boundary tests + stateful fuzzing + chaos scenarios

### For Maintainers (Release Reviews)

**Testnet readiness:**
```bash
x3-proof testnet --deploy
```
Runs: Full 24-hour validation on fresh machines

**Mainnet readiness:**
```bash
x3-proof mainnet --strict --fail-hard
```
Generates mainnet gate receipt (PASS or BLOCKED)

### For Operations (Launch Authority)

**Final go-live authorization:**
```bash
x3-proof go-live --mainnet --ceremony-date 2026-05-01 --force
```
Signs off on mainnet launch with full receipt

---

## 🎯 UNDERSTANDING PROOF LEVELS

### P-Levels (Proof Quality)

| Level | Meaning | Example |
|-------|---------|---------|
| **P0** | Claimed | "This code is secure" |
| **P2** | Compiles | Code builds without errors |
| **P3** | Unit tested | Functions work locally |
| **P4** | Integration tested | Modules work together |
| **P5** | Invariant-proven | Properties hold under test |
| **P6** | Adversarial-tested | Attacks fail safely |
| **P7** | Launch-proven | Works on production machines |

**Mainnet minimum:** P6 for critical systems

### E-Levels (Edge Case Coverage)

| Level | Means | Example |
|-------|-------|---------|
| **E0** | Claimed | "We thought about boundaries" |
| **E3** | Max+1 tested | Test max-1, max, max+1 |
| **E4** | Overflow tested | Test math overflow cases |
| **E5** | State tested | Test invalid state paths |
| **E8** | Chaos proven | Test under failures |
| **E10** | Operationally proven | Real network tested |

**Mainnet minimum:** E8 for critical systems

### H-Levels (Hack Resistance)

| Level | Means | Example |
|-------|-------|---------|
| **H0** | Claimed | "We think it's secure" |
| **H3** | Permission tested | Unauthorized rejected |
| **H5** | Attack simulated | Replay tests pass |
| **H8** | Red team partial | Internal audit done |
| **H10** | Red team proven | External audit passed |

**Mainnet minimum:** H6 for critical systems

---

## 📊 READING THE DASHBOARD

### Main Status File: `/proof/unified/status.json`

```json
{
  "testnet_readiness": {
    "score": 87,
    "status": "CANDIDATE",  # Ready for testnet
    "blockers": 2,
    "warnings": 1
  },
  
  "mainnet_readiness": {
    "score": 72,
    "status": "BLOCKED",    # Not ready yet
    "blockers": 7,          # Must fix these
    "next_actions": [...]   # How to fix
  },
  
  "areas": [
    {
      "name": "asset_kernel",
      "verify": 96,         # P-level score
      "hack": 92,           # H-level score
      "edgecase": 88,       # E-level score
      "status": "READY"     # Overall status
    }
  ]
}
```

**Interpretation:**
- ✅ Status = "READY": This area passes mainnet requirements
- ⚠️ Status = "PARTIAL": Some tests pass, some need work
- ❌ Status = "BLOCKED": This area fails mainnet requirements

### Blocker Report: `/proof/reports/blocker_report.md`

Shows all S0 and S1 blockers:

```
BLOCKER: asset_kernel #001 [S0]
Issue: Double mint possible through migration
Fix: cargo test x3-asset-kernel double_mint_migration_rejection
Status: OPEN

BLOCKER: bridge #002 [S1]
Issue: Malformed proof can panic parser
Fix: cargo test x3-bridge malformed_proof_no_panic
Status: OPEN
```

**Action:** Fix items listed under "Fix" column

---

## 🔧 COMMON WORKFLOWS

### Workflow 1: "I want to add a new feature"

```bash
# Step 1: Write the feature + test
# (include adversarial test that tries to break the feature)

# Step 2: Run local proof
x3-proof verify --strict

# Step 3: If passes, commit and open PR
git add .
git commit -m "feat: X3VM speedup (proof: P6/E8/H5)"

# Step 4: CI automatically runs full proof
# (you'll see results in PR checks)

# Step 5: When approved, merge into develop
# (testnet nightly builds will validate)
```

### Workflow 2: "I found a bug in production"

```bash
# Step 1: Identify severity
# - Can it cause double mint? → S0 EMERGENCY
# - Can it panic? → S1 CRITICAL
# - Does it affect performance? → S2 WARNING

# Step 2: Create incident
x3-proof incident create --issue "Bridge X panics on Y" --severity S0

# Step 3: Write reproduction test
# cargo test x3_bridge_panic_regression

# Step 4: Fix code
# (implement fix)

# Step 5: Verify fix
x3-proof hack --strict

# Step 6: When all proofs pass, deploy hotfix
# (with separate proof receipt)
```

### Workflow 3: "We're ready for mainnet!"

```bash
# Step 1: Run full mainnet gate
x3-proof mainnet --strict --fail-hard

# Step 2: Check if status is "CANDIDATE"
cat /proof/unified/status.json | grep mainnet_readiness

# Status = "CANDIDATE" ✅
# → Ready to proceed

# Status = "BLOCKED" ❌
# → Review blockers, fix, retry

# Step 3: If CANDIDATE, call ceremonies
x3-proof go-live --mainnet --ceremony-date 2026-05-01

# Step 4: Genesis validates automatically
# (ProofForge checks mainnet config)

# Step 5: Mainnet launches! 🎉
```

---

## ⚠️ CRITICAL ALERTS TO WATCH FOR

### Alert Level 1: S0 Blocker
```
BLOCKER: asset_kernel #001 [S0]
Cannot merge. Must fix immediately.
This can break the entire chain.
```
**Action:** Stop. Fix. Retest. THEN merge.

### Alert Level 2: S1 Blocker
```
BLOCKER: bridge #002 [S1]
Cannot release to mainnet.
This is critical path, must be solid.
```
**Action:** Fix before any release. Add to hotfix queue.

### Alert Level 3: Stale Proof
```
WARNING: asset_kernel proof from 5 days ago
Code changed 2 days ago.
Proof is older than code.
```
**Action:** Re-run proof after code changes.

### Alert Level 4: AI Patch Downgrade
```
REJECTED: Patch added unwrap() in critical path
AI-generated code weakened security.
```
**Action:** Review AI output. Remove unsafe patterns. Resubmit.

---

## 📈 METRICS TO KNOW

### Proof Scores

**90-100%** = Excellent | Ready for any environment  
**80-90%** = Good | Ready for testnet, needs attention for mainnet  
**70-80%** = Risky | Many gaps, testnet only  
**<70%** = Critical gaps | Development only

### Mutation Testing

Tests with **>85% mutation score** = Trustworthy  
Tests with **50-85%** = Weak, need improvement  
Tests with **<50%** = Practically useless, rewrite

### Fuzz Coverage

**>60 min runtime** = Excellent coverage  
**30-60 min** = Good coverage  
**10-30 min** = Basic coverage  
**<10 min** = Insufficient

---

## 🆘 TROUBLESHOOTING

### "Proof command not found"

```bash
# Make sure you're in the X3 repo
cd /path/to/x3-chain

# Install ProofForge CLI
cargo install x3-proof

# Verify
x3-proof --version
```

### "Proof is failing but I don't understand why"

```bash
# Get detailed output
x3-proof verify --strict --verbose

# OR

# Check the specific area
x3-proof verify --area asset_kernel --strict

# OR

# See the actual test output
cat /proof/reports/latest.json | jq '.failures'
```

### "I fixed the issue but proof still fails"

```bash
# Proof might be cached, force refresh
x3-proof verify --strict --no-cache

# Or rebuild everything
rm -rf /proof/evidence && x3-proof verify --strict
```

### "My merge was blocked by S0 blocker"

```bash
# Check what blocker is
cat /proof/reports/blocker_report.md

# See the test that failed
cargo test BLOCKER_NAME -- --nocapture

# Fix the code to pass the test

# Re-run just that test
cargo test BLOCKER_NAME

# When it passes, re-run full proof
x3-proof verify --strict
```

---

## 📚 DOCUMENTATION REFERENCE

| Topic | File |
|-------|------|
| Complete ProofForge spec | `/Desktop/X3_ATOMIC_STAR/x3proofengine.md` |
| Mainnet readiness checklist | `/proof/reports/mainnet_readiness.md` |
| All blockers | `/proof/reports/blocker_report.md` |
| Proof score breakdown | `/proof/unified/status.json` |
| Latest test results | `/proof/evidence/latest/` |
| Security alerts | `/proof/security/alerts.json` |

---

## 🎯 ONE-LINER REFERENCE

```bash
# Pre-commit check
x3-proof verify --strict

# Pre-merge check
x3-proof hack --strict

# Pre-release check
x3-proof testnet --deploy

# Pre-mainnet check
x3-proof mainnet --strict --fail-hard

# Launch authorization
x3-proof go-live --mainnet --ceremony-date 2026-05-01
```

---

## ✅ SUCCESS CHECKLIST

Before declaring something "done":

- [ ] Code compiles cleanly
- [ ] Unit tests pass (P3+)
- [ ] Integration tests pass (P4+)
- [ ] Mutation score >85% (P5+)
- [ ] Adversarial tests exist (P6+)
- [ ] Edge cases covered (E8+)
- [ ] No secrets in repo
- [ ] No AI patch downgrades
- [ ] Proof receipt generated
- [ ] Dashboard shows GREEN

✅ = Ready to merge
❌ = Back to development

---

## 🚀 FINAL PRINCIPLE

**ProofForge is not here to slow you down.**

ProofForge is here to catch bugs in production BEFORE they cost you $100 million.

**Every green badge ProofForge gives you is a receipt that says: "We tested this. We tried to break it. It didn't break."**

That's worth something.

---

*Questions? Check `/proof/reports/faq.md`*  
*Emergency? Run `x3-proof emergency --help`*  
*Ready to launch? Run `x3-proof go-live --mainnet`*

**YOLO DON'T STOP UNTIL YOUR GPU BURNS UP!** 🔥
