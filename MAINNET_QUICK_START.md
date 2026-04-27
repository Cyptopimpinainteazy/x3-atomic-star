# ⚠️ X3 Testnet Proof Machine: Quick Start (NOT MAINNET READY)

🚨 **IMPORTANT**: Despite the filename, this blockchain is **NOT READY FOR MAINNET DEPLOYMENT**.  
📖 See [⚠️_CRITICAL_PROOFFORGE_DISCREPANCY.md](./⚠️_CRITICAL_PROOFFORGE_DISCREPANCY.md) and [S0_BLOCKERS_REMEDIATION_PLAN.md](./S0_BLOCKERS_REMEDIATION_PLAN.md) for 9 critical blockers  
⏱️ Estimated timeline to mainnet readiness: **12-24 weeks**

This document describes testnet deployment and proof generation processes.

---

## 🎬 Start Here (5 Minutes)

```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR

# 1. Verify setup
ls -l launch-gates/build-audit-packs.sh     # ✅ Exists
ls -l launch-gates/prompts/0*.md            # ✅ 5 prompts exist
ls -l launch-gates/run-all-proofs.sh        # ✅ Exists

# 2. Generate packs (5-10 min)
chmod +x launch-gates/build-audit-packs.sh
./launch-gates/build-audit-packs.sh

# 3. Verify packs created
ls -lh launch-gates/repomix/pack-*.md       # ✅ 5 files

# 4. Collect evidence (15-20 min)
chmod +x launch-gates/run-all-proofs.sh
./launch-gates/run-all-proofs.sh

# 5. Verify evidence
ls -lh launch-gates/evidence/proof-*.log    # ✅ 12+ files
```

---

## 📋 The 5 Packs & 5 Prompts

| # | Pack | Prompt | Question | Output |
|---|------|--------|----------|--------|
| 1 | Full Repo | Wiring Audit | Is everything connected? | Wiring map |
| 2 | Runtime/Consensus | Mainnet Gate | Is it production-ready? | Category scores |
| 3 | Bridge/Atomic | Red Team | How can attackers break it? | Attack vectors |
| 4 | Test Coverage | Invariant Hunter | What invariants exist? | Invariants list |
| 5 | Git Drift | Test Gap Audit | What's not tested? | Missing tests |

---

## 🤖 Feed Each Pack to AI (Once Packs Generated)

```bash
# For each pack (1-5):
cd launch-gates

# Example: Audit pack 1
cat prompts/01-wiring-audit.md                              # Copy this
cat repomix/pack-01-full-repo-*.md                          # Paste this
# [AI auditor responds with JSON output]
# Save response to: reports/audit-01-wiring-map.json

# Repeat for packs 2-5 with corresponding prompts
```

---

## 📊 Score & Decide (After All 5 Audits)

```bash
# Review all audit results
ls -l launch-gates/reports/audit-*.json

# Extract scores from audit-02 (mainnet-scoring.json)
# and calculate overall score using category weights:

# Overall = 
#   (runtime_pct × 12%) +
#   (consensus_pct × 12%) +
#   (asset_kernel_pct × 15%) +
#   (atomic_pct × 18%) +
#   (bridge_pct × 15%) +
#   (dex_pct × 8%) +
#   (governance_pct × 6%) +
#   (validator_ops_pct × 6%) +
#   (observability_pct × 4%) +
#   (docs_pct × 4%)

# DECISION:
# - If score ≥ 90% AND zero P0 blockers → GO
# - Otherwise → NO-GO (list fixes required)
```

---

## ✅ Success Checklist

- [ ] All 5 packs generated (launch-gates/repomix/)
- [ ] All evidence collected (launch-gates/evidence/)
- [ ] All 5 audits completed (launch-gates/reports/audit-0*.json)
- [ ] Overall score calculated
- [ ] All P0 blockers identified
- [ ] Final GO/NO-GO decision documented

---

## 🚨 Hard Fail Gates (Any One = FAIL)

- [ ] Any P0 blocker exists
- [ ] Bridge has no replay test
- [ ] Atomic swap has no rollback test
- [ ] Canonical supply not tested
- [ ] No multi-node testnet proof

---

## 📂 File Locations

```
launch-gates/
├── repomix.config.json              ← Strategic includes/excludes
├── build-audit-packs.sh             ← Generate 5 packs
├── run-all-proofs.sh                ← Collect all evidence
├── mainnet-go-no-go-template.sh     ← Score & decide
├── prompts/
│   ├── 01-wiring-audit.md           ← Prompt 1
│   ├── 02-mainnet-gate.md           ← Prompt 2
│   ├── 03-bridge-redteam.md         ← Prompt 3
│   ├── 04-invariant-hunter.md       ← Prompt 4
│   └── 05-test-gap-audit.md         ← Prompt 5
├── repomix/                         ← 5 audit packs (generated)
├── evidence/                        ← Proof outputs (generated)
└── reports/                         ← Audit results (generated)
```

---

## 📖 Full Workflow

See: `MAINNET_PROOF_MACHINE_WORKFLOW.md` for complete step-by-step guide.

---

## 🎯 Core Principle

**"No proof = no points"**

A feature cannot score higher than its strongest test:
- Code exists only → max 10%
- Code wired → max 25%
- Compiles → max 45%
- Unit tested → max 55%
- Integration tested → max 70%
- Fuzz/invariant tested → max 85%
- Testnet proven → max 95%
- Externally audited → 100%

Beautiful code without tests scores 55%. Period.

---

## ⏱️ Timeline

| Phase | Task | Time |
|-------|------|------|
| 1 | Generate 5 packs | 5-10 min |
| 2 | Collect proofs | 15-20 min |
| 3 | Audit all 5 packs | 2-3 hours |
| 4 | Score & decide | 30 min |
| **Total** | **End-to-end** | **3-4 hours** |

---

## 🚀 After Decision

**GO:** Create release, finalize chain spec, Genesis ceremony  
**NO-GO:** Fix blockers, re-run Phase 1-4

---

## 💡 Tips

1. **Packs too large?** They're optimized per category. Use them whole.
2. **AI getting confused?** Load prompt first, then pack (in that order).
3. **Results look incomplete?** Ensure AI response was saved as JSON (use formatting).
4. **Need reproducibility?** Save commit hash + all hashes in a release folder.

---

## 📞 Troubleshooting

| Problem | Solution |
|---------|----------|
| Packs not generating | Run: `repomix --version` to verify installed |
| Proofs failing | Check: `cargo check --workspace` compiles cleanly |
| AI audit unclear | Ensure you pasted both prompt AND entire pack |
| Score calculation wrong | Double-check category weights sum to 100% |
| P0 blocker disagreement | Re-run audit; weight security findings heavily |

---

**Start now:** `./launch-gates/build-audit-packs.sh`

**Questions?** See full workflow guide: `MAINNET_PROOF_MACHINE_WORKFLOW.md`

