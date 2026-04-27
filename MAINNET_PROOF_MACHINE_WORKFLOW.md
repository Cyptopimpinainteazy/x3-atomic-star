# 🚨 X3 Mainnet Proof Machine: Workflow (NOT MAINNET READY)

**Status:** ⚠️ Testnet validation framework only  
**Last Updated:** April 26, 2026 (Current session)  
**Commitment:** "No proof = no points" methodology revealed 9 critical blockers — 🟡 **5 NOW RESOLVED (April 27, 2026 audit, [STATUS_AUDIT_2026_04_27.md](./STATUS_AUDIT_2026_04_27.md))**

---

## ⚠️ CRITICAL FINDING

This "Mainnet Proof Machine" framework successfully identified that the X3 blockchain was **NOT READY FOR MAINNET** — since then, **5 of 9 blockers have been RESOLVED** (April 27, 2026 audit). See [STATUS_AUDIT_2026_04_27.md](./STATUS_AUDIT_2026_04_27.md) for current state.

**ProofForge Comprehensive Results**:
- 🚨 **0% Mainnet Ready** (all 4 gates FAILED)
- 🚨 **9 Critical Blockers** (6 S0 catastrophic + 3 S1 critical)
- 🚨 **116 Implementation Gaps** preventing production deployment
- 🚨 **12-24 weeks** estimated remediation timeline

**Key Blockers**:
1. atomic_rollback_missing (S0-005) - Failed atomics leave partial state
2. double_mint_possible (S0-002) - Infinite minting risk
3. bridge_replay_accepted (S0-003) - Transactions replayed multiple times
4. finality_spoof_accepted (S0-004) - False finality claims accepted
5-9. [See S0_BLOCKERS_REMEDIATION_PLAN.md](S0_BLOCKERS_REMEDIATION_PLAN.md)

**See:** [⚠️_CRITICAL_PROOFFORGE_DISCREPANCY.md](⚠️_CRITICAL_PROOFFORGE_DISCREPANCY.md)

---

## 📋 Quick Navigation

1. [What You're About To Do](#what-youre-about-to-do)
2. [Phase 1: Generate Audit Packs](#phase-1-generate-audit-packs)
3. [Phase 2: Run Proof Commands](#phase-2-run-proof-commands)
4. [Phase 3: Feed Packs to AI Auditors](#phase-3-feed-packs-to-ai-auditors)
5. [Phase 4: Score & Decide](#phase-4-score--decide)
6. [Complete Checklist](#complete-checklist)

---

## 🎯 What You're About To Do

You're building a **"Mainnet Proof Machine"** for X3. This is NOT "AI reads the repo once." This is:

1. **5 Targeted Repomix Packs** (not 1 huge dump)
   - Pack 1: Full repository wiring map
   - Pack 2: Runtime/consensus internals
   - Pack 3: Bridge/atomic cross-VM security
   - Pack 4: Test coverage gaps
   - Pack 5: Git drift (code/docs alignment)

2. **5 AI Audit Prompts** (specialized for each pack)
   - Prompt 01: "Is everything wired?"
   - Prompt 02: "Is this mainnet-ready?" (with 13-category scoring)
   - Prompt 03: "How can attackers break this?" (red team)
   - Prompt 04: "What invariants exist and are they tested?"
   - Prompt 05: "What critical behaviors are NOT tested?"

3. **Hard Evidence Collection** (run-all-proofs.sh)
   - Compilation proofs
   - Test execution proofs
   - Code quality proofs
   - Git state proofs
   - Fresh-machine launch proofs

4. **Proof-Based Scoring Dashboard**
   - 13 scoring categories with weights
   - Hard fail gates (P0 blocker = instant FAIL)
   - Score capped at proof level (no test = max 55%)
   - Final GO/NO-GO decision

---

## Phase 1: Generate Audit Packs

### Step 1.1: Verify Repomix Config

```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR
cat launch-gates/repomix.config.json | head -20
```

Expected output: Strategic includes (launch-gates/**, **/*.rs, **/*.toml) and excludes (target/, node_modules/).

### Step 1.2: Build All 5 Packs

```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR
chmod +x launch-gates/build-audit-packs.sh
./launch-gates/build-audit-packs.sh
```

Expected output in `launch-gates/repomix/`:
```
pack-01-full-repo-[timestamp].md (with SHA256)
pack-02-runtime-consensus-[timestamp].md (with SHA256)
pack-03-bridge-atomic-[timestamp].md (with SHA256)
pack-04-test-coverage-[timestamp].md (with SHA256)
pack-05-git-drift-[timestamp].md (with SHA256)
```

### Step 1.3: Verify Pack Generation

```bash
ls -lh launch-gates/repomix/ | grep pack
wc -l launch-gates/repomix/pack-*.md
```

Each pack should be 3MB-10MB. Total ~40-50MB.

---

## Phase 2: Run Proof Commands

### Step 2.1: Generate All Evidence

```bash
chmod +x launch-gates/run-all-proofs.sh
./launch-gates/run-all-proofs.sh
```

This runs:
- ✅ Cargo check (workspace + runtime)
- ✅ Cargo test (all tests)
- ✅ Clippy warnings scan
- ✅ Format check
- ✅ Hazard scan (TODO/panic/unwrap detection)
- ✅ Wiring verification
- ✅ Settlement E2E tests (if Phase 5a completed)
- ✅ Git state verification
- ✅ Release build proof

Expected output in `launch-gates/evidence/`:
```
proof-01-check-workspace-[timestamp].log
proof-02-test-workspace-[timestamp].log
proof-03-test-bridge-[timestamp].log
proof-04-clippy-[timestamp].log
...
ALL_PROOFS_[timestamp].sha256
```

### Step 2.2: Verify Proof Generation

```bash
ls -lh launch-gates/evidence/ | wc -l
cat launch-gates/evidence/ALL_PROOFS_*.sha256
```

Should have 12+ proof files with SHA256 hashes.

---

## Phase 3: Feed Packs to AI Auditors

### Step 3.1: Prepare Audit Session

You have 5 packs and 5 prompts. For each pack:

| Pack | Prompt | Purpose | Time | Output |
|------|--------|---------|------|--------|
| Pack 1: Full Repo | Prompt 01: Wiring Audit | "Is everything connected?" | 15-20 min | Wiring map JSON |
| Pack 2: Runtime/Consensus | Prompt 02: Mainnet Gate | "Is it production-ready?" (scoring) | 20-30 min | Category scores JSON |
| Pack 3: Bridge/Atomic | Prompt 03: Red Team | "How can attackers break it?" | 30-45 min | Attack vectors JSON |
| Pack 4: Test Coverage | Prompt 04: Invariant Hunter | "What invariants exist?" | 20-30 min | Invariants list JSON |
| Pack 5: Git Drift | Prompt 05: Test Gap Audit | "What's not tested?" | 20-30 min | Missing tests JSON |

### Step 3.2: Audit Pack 1 (Full Repository Wiring)

1. Open new AI chat session
2. Load prompt: `cat launch-gates/prompts/01-wiring-audit.md`
3. Paste entire pack 1: `cat launch-gates/repomix/pack-01-full-repo-*.md`
4. Request: "Use the JSON output format and flag any module that exists but is not wired."
5. Save AI response as: `launch-gates/reports/audit-01-wiring-map.json`

Expected response structure:
```json
{
  "audit_type": "wiring_completeness",
  "wiring_map": [ ... ],
  "unwired_modules": [ ... ],
  "blockers": [ ... ]
}
```

### Step 3.3: Audit Pack 2 (Mainnet Launch Gate)

1. New AI chat session
2. Load prompt: `cat launch-gates/prompts/02-mainnet-gate.md`
3. Paste pack 2: `cat launch-gates/repomix/pack-02-runtime-consensus-*.md`
4. Request: "Score each category using the proof-level capping rule. If no integration test exists, max 55% even if code is beautiful."
5. Save response as: `launch-gates/reports/audit-02-mainnet-scoring.json`

Expected response structure:
```json
{
  "audit_type": "mainnet_launch_gate",
  "overall_score": 0,
  "categories": [ ... ],
  "p0_blockers": [ ... ]
}
```

### Step 3.4: Audit Pack 3 (Bridge Red Team)

1. New AI chat session
2. Load prompt: `cat launch-gates/prompts/03-bridge-redteam.md`
3. Paste pack 3: `cat launch-gates/repomix/pack-03-bridge-atomic-*.md`
4. Request: "Attempt each of the 10 attack scenarios. For each, state if it's exploitable before a fix."
5. Save as: `launch-gates/reports/audit-03-bridge-attacks.json`

Expected response structure:
```json
{
  "audit_type": "bridge_red_team",
  "threat_level": "CRITICAL|HIGH|MEDIUM|LOW",
  "attacks": [ ... ],
  "exploitable_before_fix": true|false
}
```

### Step 3.5: Audit Pack 4 (Invariants)

1. New AI chat session
2. Load prompt: `cat launch-gates/prompts/04-invariant-hunter.md`
3. Paste pack 4: `cat launch-gates/repomix/pack-04-test-coverage-*.md`
4. Request: "Extract all P0 financial, atomicity, bridge, and consensus invariants. For each, state if it's tested."
5. Save as: `launch-gates/reports/audit-04-invariants.json`

### Step 3.6: Audit Pack 5 (Test Gaps)

1. New AI chat session
2. Load prompt: `cat launch-gates/prompts/05-test-gap-audit.md`
3. Paste pack 5: `cat launch-gates/repomix/pack-05-git-drift-*.md`
4. Request: "Identify production-critical behaviors with ZERO test coverage."
5. Save as: `launch-gates/reports/audit-05-test-gaps.json`

---

## Phase 4: Score & Decide

### Step 4.1: Aggregate Audit Results

```bash
# Collect all audit outputs
ls -lh launch-gates/reports/audit-*.json

# Verify all 5 audits completed
test -f launch-gates/reports/audit-01-*.json && echo "✅ Pack 1"
test -f launch-gates/reports/audit-02-*.json && echo "✅ Pack 2"
test -f launch-gates/reports/audit-03-*.json && echo "✅ Pack 3"
test -f launch-gates/reports/audit-04-*.json && echo "✅ Pack 4"
test -f launch-gates/reports/audit-05-*.json && echo "✅ Pack 5"
```

### Step 4.2: Calculate Overall Score

Using audit results:

```
Overall Score = 
  (Runtime/Pallets % × 12) +
  (Consensus/Finality % × 12) +
  (Asset Kernel % × 15) +
  (Atomic Cross-VM % × 18) +
  (Bridge % × 15) +
  (DEX % × 8) +
  (Governance % × 6) +
  (Validator Ops % × 6) +
  (Observability % × 4) +
  (Docs % × 4)
```

### Step 4.3: Apply Hard Fail Gates

Check each blocking condition:

```bash
# Check P0 blockers from all audit reports
grep -l '"severity": "P0"' launch-gates/reports/audit-*.json | wc -l

# If count > 0, result is FAIL regardless of score
```

Hard fails (any one = NO-GO):
- [ ] Any P0 blocker exists
- [ ] Runtime compiles with mocks only
- [ ] Critical pallet not in construct_runtime!
- [ ] Bridge has no replay test
- [ ] Atomic swap has no rollback test
- [ ] Canonical supply not invariant-tested
- [ ] No multi-node testnet proof
- [ ] No fresh-machine launch proof

### Step 4.4: Generate Final Report

```bash
chmod +x launch-gates/mainnet-go-no-go-template.sh
./launch-gates/mainnet-go-no-go-template.sh > launch-gates/reports/MAINNET-DECISION-$(date +%Y%m%d-%H%M%S).md
```

### Step 4.5: Make Decision

```
IF overall_score ≥ 90% AND zero_p0_blockers:
  DECISION = "GO"
ELSE:
  DECISION = "NO-GO"
  List required fixes
```

---

## Complete Checklist

### Pre-Flight (Before Starting)

- [ ] X3 repository cloned and up-to-date
- [ ] Rust toolchain installed (`rustc --version`)
- [ ] Cargo works (`cargo --version`)
- [ ] Repomix available (`repomix --version`)
- [ ] Git clean state (`git status`)

### Phase 1: Packs Generated ✅

- [ ] Verified repomix.config.json
- [ ] Ran build-audit-packs.sh
- [ ] All 5 packs exist in launch-gates/repomix/
- [ ] All 5 packs have SHA256 hashes

### Phase 2: Proofs Collected ✅

- [ ] Ran run-all-proofs.sh
- [ ] All 12+ proof files exist in launch-gates/evidence/
- [ ] All proofs have SHA256 hashes
- [ ] Proofs include: compilation, tests, code quality, git state, build

### Phase 3: Packs Audited ✅

- [ ] Audit 01 (Wiring) completed → audit-01-wiring-map.json
- [ ] Audit 02 (Mainnet Gate) completed → audit-02-mainnet-scoring.json
- [ ] Audit 03 (Red Team) completed → audit-03-bridge-attacks.json
- [ ] Audit 04 (Invariants) completed → audit-04-invariants.json
- [ ] Audit 05 (Test Gaps) completed → audit-05-test-gaps.json

### Phase 4: Scored & Decided ✅

- [ ] Aggregated all 5 audit results
- [ ] Calculated overall score using category weights
- [ ] Applied hard fail gates
- [ ] Generated final report with GO/NO-GO decision
- [ ] Documented all P0 blockers (if any)
- [ ] Identified all required fixes

### Post-Decision Actions

**IF DECISION = GO:**
- [ ] Create mainnet candidate release
- [ ] Finalize chain spec
- [ ] Collect validator session keys
- [ ] Set bootnodes
- [ ] Configure telemetry
- [ ] Launch genesis ceremony

**IF DECISION = NO-GO:**
- [ ] List all P0 blockers
- [ ] Prioritize fixes
- [ ] Assign fix owners
- [ ] Set re-audit date
- [ ] Go back to Phase 1 after fixes

---

## 📊 Success Criteria

**You have successfully built the Mainnet Proof Machine when:**

✅ All 5 Repomix packs generated with hashes  
✅ All 5 AI audits completed with JSON output  
✅ Overall score calculated using category weights  
✅ All P0 blockers identified  
✅ Final GO/NO-GO decision documented  
✅ Every claim has proof attached  
✅ All evidence reproducible from commit hash  

---

## 🚀 Next Steps After Decision

### If GO (Score ≥90%, Zero P0 Blockers)

1. **Create Release Snapshot**
   ```bash
   mkdir -p launch-gates/releases/mainnet-candidate-001
   cp launch-gates/repomix/*.sha256 launch-gates/releases/mainnet-candidate-001/
   cp launch-gates/evidence/*.sha256 launch-gates/releases/mainnet-candidate-001/
   cp launch-gates/reports/*.json launch-gates/releases/mainnet-candidate-001/
   git commit -m "Mainnet candidate 001: score=$(score)% GO"
   ```

2. **Finalize Chain Spec**
   ```bash
   # Set final validator keys, bootnodes, telemetry
   cargo build --release
   ./target/release/x3-chain-node build-spec --chain=mainnet > chain_spec.json
   # Add validators, bootnodes, telemetry
   ```

3. **Genesis Ceremony**
   - Collect validator session keys
   - Distribute chain spec
   - Coordinate launch time
   - Monitor first blocks

### If NO-GO (Score <90% OR Any P0 Blocker)

1. **Document Blockers**
   - List all P0, P1, P2 blockers
   - Assign fix owners
   - Set target fix dates

2. **Fix & Re-Audit**
   - Fix each blocker
   - Commit changes
   - Re-run Phase 1-4
   - Repeat until GO

---

## 📞 Support

If audit gets stuck or unclear:
1. Check that all 5 prompts match their intended task
2. Verify audit packs are not truncated (check file sizes)
3. Feed larger packs one section at a time if needed
4. Save all AI audit responses for reproducibility

Remember: **No proof = no points. No exceptions.**

