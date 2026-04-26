# X3 Mainnet Proof Machine: Comprehensive File Index

**Generated:** $(date)  
**Completeness:** ✅ All components deployed and verified  
**Status:** Ready for Phase 1 execution  

---

## 📑 Document Navigation Map

### 🎬 START HERE (Read First - 5 min)

```
00-START-HERE-MAINNET-READINESS.md
├─ What was built (summary)
├─ How to use (3-4 hour overview)
├─ Quick copy-paste commands
└─ Next actions
```

### 📖 READ NEXT (Choose One)

**Fast Track (TL;DR - 5 min):**
```
MAINNET_QUICK_START.md
├─ The 5 packs & 5 prompts
├─ Quick start checklist
├─ Timeline (3-4 hours)
└─ Troubleshooting
```

**Complete Guide (Full Details - 20 min):**
```
MAINNET_PROOF_MACHINE_WORKFLOW.md
├─ Phase 1: Generate audit packs (step-by-step)
├─ Phase 2: Run proof commands (step-by-step)
├─ Phase 3: Feed packs to AI (step-by-step)
├─ Phase 4: Score & decide (step-by-step)
└─ Complete checklist
```

**Deployment Summary (Architecture - 10 min):**
```
MAINNET_PROOF_MACHINE_DEPLOYED.md
├─ What was built (components)
├─ File structure & locations
├─ Key innovation (proof-level capping)
├─ Scoring framework (10 categories)
└─ Success criteria
```

---

## 🔧 Execution Scripts (run in this order)

### Phase 1: Generate Audit Packs (5-10 min)

```bash
launch-gates/build-audit-packs.sh
├─ Reads: repomix.config.json
├─ Generates: 5 Repomix markdown packs
├─ Output: launch-gates/repomix/pack-*.md
├─ Also: SHA256 hashes for reproducibility
└─ Success: "✅ All packs generated"
```

**Command:**
```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR
chmod +x launch-gates/build-audit-packs.sh
./launch-gates/build-audit-packs.sh
```

**Expected Output:**
```
launch-gates/repomix/
├── pack-01-full-repo-[TIMESTAMP].md
├── pack-01-full-repo-[TIMESTAMP].md.sha256
├── pack-02-runtime-consensus-[TIMESTAMP].md
├── pack-02-runtime-consensus-[TIMESTAMP].md.sha256
├── pack-03-bridge-atomic-[TIMESTAMP].md
├── pack-03-bridge-atomic-[TIMESTAMP].md.sha256
├── pack-04-test-coverage-[TIMESTAMP].md
├── pack-04-test-coverage-[TIMESTAMP].md.sha256
├── pack-05-git-drift-[TIMESTAMP].md
└── pack-05-git-drift-[TIMESTAMP].md.sha256
```

---

### Phase 2: Run Hard Evidence Collection (15-20 min)

```bash
launch-gates/run-all-proofs.sh
├─ Runs: 9 proof categories (20+ commands)
├─ Collects: Compilation, tests, quality, hazards, git, build
├─ Output: launch-gates/evidence/proof-*.log
├─ Also: SHA256 hashes for each proof
└─ Success: "✅ All proofs collected"
```

**Command:**
```bash
chmod +x launch-gates/run-all-proofs.sh
./launch-gates/run-all-proofs.sh
```

**Expected Output:**
```
launch-gates/evidence/
├── proof-01-check-workspace-[TIMESTAMP].log
├── proof-02-test-workspace-[TIMESTAMP].log
├── proof-03-test-bridge-[TIMESTAMP].log
├── proof-04-clippy-[TIMESTAMP].log
├── proof-05-fmt-check-[TIMESTAMP].log
├── proof-06-hazard-scan-[TIMESTAMP].log
├── proof-07-construct-runtime-[TIMESTAMP].log
├── proof-08-settlement-tests-[TIMESTAMP].log
├── proof-09-git-commit-[TIMESTAMP].log
├── ALL_PROOFS-[TIMESTAMP].log
└── ALL_PROOFS-[TIMESTAMP].sha256
```

---

### Phase 4: Generate Final Report (30 min - after all audits)

```bash
launch-gates/mainnet-go-no-go-template.sh
├─ Reads: All 5 audit JSON files
├─ Calculates: Overall score with category weights
├─ Applies: Hard fail gates (P0 blocker = FAIL)
├─ Outputs: Markdown report with GO/NO-GO
└─ Success: "✅ Decision documented"
```

**Command:**
```bash
chmod +x launch-gates/mainnet-go-no-go-template.sh
./launch-gates/mainnet-go-no-go-template.sh > launch-gates/reports/MAINNET-DECISION-$(date +%Y%m%d-%H%M%S).md
```

---

## 🎯 5 Audit Prompts (Use in Phase 3)

### Prompt 1: Wiring Audit

```
File: launch-gates/prompts/01-wiring-audit.md
Use with: Pack 1 (full repository)
Question: "Is everything wired into the runtime?"
Output: JSON with wiring map + unwired modules
Time: 15-20 min
```

**How to use:**
1. Load prompt: `cat launch-gates/prompts/01-wiring-audit.md`
2. Load pack: `cat launch-gates/repomix/pack-01-full-repo-*.md`
3. Request: AI to output JSON with wiring verification
4. Save response: `launch-gates/reports/audit-01-wiring-map.json`

---

### Prompt 2: Mainnet Launch Gate (Scoring)

```
File: launch-gates/prompts/02-mainnet-gate.md
Use with: Pack 2 (runtime/consensus)
Question: "Is this mainnet-ready? Score by category."
Output: JSON with scores (1-100) + P0 blockers
Time: 20-30 min
```

**How to use:**
1. Load prompt: `cat launch-gates/prompts/02-mainnet-gate.md`
2. Load pack: `cat launch-gates/repomix/pack-02-runtime-consensus-*.md`
3. Request: AI to score 10 categories + list P0 blockers
4. Save response: `launch-gates/reports/audit-02-mainnet-scoring.json`

**Categories to score:**
- Runtime/Pallets (12% weight)
- Consensus/Finality (12%)
- Asset Kernel (15%)
- Atomic Cross-VM (18%)
- Bridge (15%)
- DEX (8%)
- Governance (6%)
- Validator Ops (6%)
- Observability (4%)
- Documentation (4%)

---

### Prompt 3: Bridge Red Team Audit

```
File: launch-gates/prompts/03-bridge-redteam.md
Use with: Pack 3 (bridge/atomic)
Question: "How can attackers break this bridge?"
Output: JSON with 10 attack scenarios + exploitability
Time: 30-45 min
```

**How to use:**
1. Load prompt: `cat launch-gates/prompts/03-bridge-redteam.md`
2. Load pack: `cat launch-gates/repomix/pack-03-bridge-atomic-*.md`
3. Request: AI to evaluate 10 attack scenarios
4. Save response: `launch-gates/reports/audit-03-bridge-attacks.json`

**Attack scenarios covered:**
- Replay attacks
- Partial settlement
- Timeout abuse
- Nonce exhaustion
- Finality conflicts
- Governance attacks
- Supply leakage
- Signature forgery
- Timing attacks
- Cross-VM desync

---

### Prompt 4: Invariant Hunter

```
File: launch-gates/prompts/04-invariant-hunter.md
Use with: Pack 4 (test coverage)
Question: "What P0 invariants exist? Are they tested?"
Output: JSON with invariants + test coverage matrix
Time: 20-30 min
```

**How to use:**
1. Load prompt: `cat launch-gates/prompts/04-invariant-hunter.md`
2. Load pack: `cat launch-gates/repomix/pack-04-test-coverage-*.md`
3. Request: AI to extract all P0 invariants + verify test coverage
4. Save response: `launch-gates/reports/audit-04-invariants.json`

**Invariant types:**
- Financial (canonical supply, mint/burn balance)
- Atomicity (all-or-nothing execution)
- Bridge (exactly-once, nonce increases)
- Consensus (no equivocation, finality)
- Governance (authority checks, timelock)

---

### Prompt 5: Test Gap Audit

```
File: launch-gates/prompts/05-test-gap-audit.md
Use with: Pack 5 (git drift)
Question: "What critical behaviors are NOT tested?"
Output: JSON with missing tests + fuzz/invariant gaps
Time: 20-30 min
```

**How to use:**
1. Load prompt: `cat launch-gates/prompts/05-test-gap-audit.md`
2. Load pack: `cat launch-gates/repomix/pack-05-git-drift-*.md`
3. Request: AI to identify production-critical untested behaviors
4. Save response: `launch-gates/reports/audit-05-test-gaps.json`

**Critical behaviors checked:**
- Replay attack resistance
- Partial execution failure
- Rollback guarantee
- Finality & reorg handling
- Bridge timeout execution
- Storage overflow handling
- Boundary amount handling
- Invalid input rejection
- Governance gate enforcement
- Validator equivocation detection
- Mempool front-running
- Contract migration safety
- Fresh-machine launch
- Multi-node testnet capability

---

## 📋 Configuration Files

### Repomix Config (Strategic)

```bash
Location: /home/lojak/Desktop/X3_ATOMIC_STAR/repomix.config.json
Purpose: Define what goes into audit packs
Content:
  Include: launch-gates/**, **/*.rs, **/*.toml, **/*.json, *.md
  Exclude: target/, node_modules/, .git/, *.log
```

### Proofs Registry

```bash
Location: launch-gates/proofs.yaml
Purpose: Define proof tiers and feature requirements
Content:
  - Proof tier levels (10% → 100%)
  - Feature definitions (runtime_core, flash_finality, etc.)
  - Proof requirements per feature
```

### Invariants Registry

```bash
Location: launch-gates/invariants.yaml
Purpose: Define P0 invariants for verification
Content:
  - Financial invariants (supply conservation)
  - Atomicity invariants (all-or-nothing)
  - Bridge invariants (replay safety)
  - Consensus invariants (no equivocation)
  - Governance invariants (authority)
```

---

## 📂 Generated Output Directories

### Audit Packs (Phase 1 Output)

```bash
launch-gates/repomix/
├── pack-01-full-repo-[TIMESTAMP].md          (full repo wiring)
├── pack-02-runtime-consensus-[TIMESTAMP].md  (runtime internals)
├── pack-03-bridge-atomic-[TIMESTAMP].md      (bridge/atomic security)
├── pack-04-test-coverage-[TIMESTAMP].md      (test suites & gaps)
├── pack-05-git-drift-[TIMESTAMP].md          (git history & drift)
└── *.sha256                                   (reproducibility hashes)
```

### Evidence Logs (Phase 2 Output)

```bash
launch-gates/evidence/
├── proof-01-check-workspace-*.log   (cargo check)
├── proof-02-test-workspace-*.log    (cargo test)
├── proof-03-test-bridge-*.log       (bridge tests)
├── proof-04-clippy-*.log            (code warnings)
├── proof-05-fmt-check-*.log         (format check)
├── proof-06-hazard-scan-*.log       (TODO/panic/unwrap)
├── proof-07-construct-runtime-*.log (wiring check)
├── proof-08-settlement-tests-*.log  (settlement tests)
├── proof-09-git-commit-*.log        (git state)
└── ALL_PROOFS-*.sha256              (aggregate hash)
```

### Audit Reports (Phase 3 Output)

```bash
launch-gates/reports/
├── audit-01-wiring-map.json         (wiring map + unwired)
├── audit-02-mainnet-scoring.json    (category scores + P0s)
├── audit-03-bridge-attacks.json     (attack scenarios)
├── audit-04-invariants.json         (P0 invariants + coverage)
├── audit-05-test-gaps.json          (missing tests)
└── MAINNET-DECISION-*.md            (final GO/NO-GO report)
```

---

## 🔑 Key Formulas & Decision Logic

### Overall Score Calculation

```
Overall_Score = 
  (Category_Runtime_Score × 0.12) +
  (Category_Consensus_Score × 0.12) +
  (Category_AssetKernel_Score × 0.15) +
  (Category_Atomic_Score × 0.18) +
  (Category_Bridge_Score × 0.15) +
  (Category_DEX_Score × 0.08) +
  (Category_Governance_Score × 0.06) +
  (Category_ValidatorOps_Score × 0.06) +
  (Category_Observability_Score × 0.04) +
  (Category_Docs_Score × 0.04)
```

### Proof Level Cap

```
IF test_level == "None":
  max_score = 10
ELSE IF test_level == "Wired":
  max_score = 25
ELSE IF test_level == "Compiles":
  max_score = 45
ELSE IF test_level == "UnitTested":
  max_score = 55
ELSE IF test_level == "IntegrationTested":
  max_score = 70
ELSE IF test_level == "FuzzTested":
  max_score = 85
ELSE IF test_level == "TestnetProven":
  max_score = 95
ELSE IF test_level == "ExternallyAudited":
  max_score = 100
END IF

category_score = MIN(category_score, max_score)
```

### Final Decision

```
IF overall_score >= 90 AND p0_blocker_count == 0:
  DECISION = "GO" ✅
  
ELSE IF overall_score >= 75 AND p0_blocker_count <= 2:
  DECISION = "CONDITIONAL_GO" ⚠️
  
ELSE:
  DECISION = "NO_GO" ❌
END IF
```

---

## 🚨 Hard Fail Conditions (Any = NO-GO)

```
1. Runtime compiles with mocks only
2. Critical pallet not in construct_runtime!
3. Bridge has no replay protection test
4. Atomic swap has no rollback test
5. Canonical supply not invariant-tested
6. No multi-node testnet proof
7. No fresh-machine launch proof
8. No validator onboarding pack
9. Benchmark weights incomplete
10. Chain spec incomplete
```

---

## ⏱️ Time Breakdown

| Phase | Task | Time | Cumulative |
|-------|------|------|------------|
| Setup | Verify everything in place | 5 min | 5 min |
| Phase 1 | Generate audit packs | 5-10 min | 10-15 min |
| Phase 2 | Collect hard evidence | 15-20 min | 25-35 min |
| *Break* | Coffee/review prompts | 10 min | 35-45 min |
| Phase 3a | Audit pack 1 (wiring) | 15-20 min | 50-65 min |
| Phase 3b | Audit pack 2 (mainnet scoring) | 20-30 min | 70-95 min |
| Phase 3c | Audit pack 3 (red team) | 30-45 min | 100-140 min |
| Phase 3d | Audit pack 4 (invariants) | 20-30 min | 120-170 min |
| Phase 3e | Audit pack 5 (test gaps) | 20-30 min | 140-200 min |
| Phase 4 | Score & decide | 30 min | 170-230 min |
| **TOTAL** | **End-to-end** | **2:50 - 3:50** | **~3-4 hours** |

---

## ✅ Pre-Flight Checklist

Before starting Phase 1:

```bash
# ✅ Repository exists and clean
test -d /home/lojak/Desktop/X3_ATOMIC_STAR && echo "✅ Repo"
cd /home/lojak/Desktop/X3_ATOMIC_STAR && git status | grep -q "nothing to commit" && echo "✅ Git clean"

# ✅ All scripts exist
test -f launch-gates/build-audit-packs.sh && echo "✅ Pack builder"
test -f launch-gates/run-all-proofs.sh && echo "✅ Proof runner"
test -f launch-gates/mainnet-go-no-go-template.sh && echo "✅ Scorer"

# ✅ All prompts exist
ls launch-gates/prompts/0{1-5}-*.md | wc -l | grep -q 5 && echo "✅ All 5 prompts"

# ✅ Toolchain works
cargo --version && echo "✅ Cargo"
repomix --version && echo "✅ Repomix"

# ✅ Directories ready
mkdir -p launch-gates/{repomix,evidence,reports} && echo "✅ Directories"
```

---

## 🎓 Core Principles

### 1. No Proof = No Points
Every claim requires evidence. No exceptions.

### 2. Score Capped at Proof Level
Beautiful code without tests cannot score higher than 55%.

### 3. Hard Fail Gates
ANY P0 blocker = instant NO-GO, regardless of score.

### 4. Reproducibility
Same commit hash + same commands = same results.

### 5. Auditability
All evidence collected, hashed, and saved for review.

---

## 🚀 Immediate Next Steps

```bash
# 1. Verify this document
cat /home/lojak/Desktop/X3_ATOMIC_STAR/00-START-HERE-MAINNET-READINESS.md

# 2. Read quick start
cat /home/lojak/Desktop/X3_ATOMIC_STAR/MAINNET_QUICK_START.md

# 3. Navigate to repo
cd /home/lojak/Desktop/X3_ATOMIC_STAR

# 4. Run Phase 1
./launch-gates/build-audit-packs.sh

# 5. Run Phase 2
./launch-gates/run-all-proofs.sh

# 6. See next instructions
cat MAINNET_QUICK_START.md | tail -20
```

---

## 📞 Quick Reference

| Need | File |
|------|------|
| Quick overview (5 min) | MAINNET_QUICK_START.md |
| Complete workflow | MAINNET_PROOF_MACHINE_WORKFLOW.md |
| Architecture & scoring | MAINNET_PROOF_MACHINE_DEPLOYED.md |
| This index | MAINNET_PROOF_MACHINE_FILE_INDEX.md |
| Wiring audit (prompt 1) | launch-gates/prompts/01-wiring-audit.md |
| Scoring (prompt 2) | launch-gates/prompts/02-mainnet-gate.md |
| Red team (prompt 3) | launch-gates/prompts/03-bridge-redteam.md |
| Invariants (prompt 4) | launch-gates/prompts/04-invariant-hunter.md |
| Test gaps (prompt 5) | launch-gates/prompts/05-test-gap-audit.md |

---

**Ready to start?**

```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR
./launch-gates/build-audit-packs.sh
```

