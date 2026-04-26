# 🗂️ X3 ProofForge — COMPLETE INDEX & NAVIGATION

**Everything you need to know about X3's executable proof system in one place.**

---

## 📖 READ THESE FIRST

Start here based on your role:

### If you're a **Developer**
1. Read: [X3_PROOFFORGE_QUICK_START.md](./X3_PROOFFORGE_QUICK_START.md) — Day-to-day commands
2. Then: [x3proofengine.md](./x3proofengine.md) — Sections 1-7 (understand the system)
3. Reference: Section 10 (CLI commands) for complete command list

### If you're a **Maintainer/Release Manager**
1. Read: [X3_PROOFFORGE_QUICK_START.md](./X3_PROOFFORGE_QUICK_START.md) — Workflows
2. Then: [x3proofengine.md](./x3proofengine.md) — v1-v4 (Prove, Debt, Severity, Mainnet Gate)
3. Key section: **v7 Mainnet Gate** for launch procedures

### If you're **Operations/DevOps**
1. Read: [X3_PROOFFORGE_QUICK_START.md](./X3_PROOFFORGE_QUICK_START.md) — Workflows 2-3
2. Then: [x3proofengine.md](./x3proofengine.md) — v6 (Operator Safety) & v7 (Mainnet Gate)
3. Key section: **Safe Command Wrappers** and **Go-Live Checklist**

### If you're a **Security Researcher**
1. Read: [x3proofengine.md](./x3proofengine.md) — v5 (Hack-Resistance)
2. Then: [x3proofengine.md](./x3proofengine.md) — v3 (Edge Cases) & v4 (Degradation)
3. Reference: Attack surfaces, hack tests by subsystem, adversarial patterns

### If you're a **Validator/Node Operator**
1. Read: [X3_PROOFFORGE_QUICK_START.md](./X3_PROOFFORGE_QUICK_START.md) — Overview
2. Then: [x3proofengine.md](./x3proofengine.md) — v6 (Operator Safety)
3. Key section: **Wrong Network Protection** and **Big Red Button Mode**

---

## 📚 DOCUMENT STRUCTURE

### Main Documents

| Document | Lines | Purpose |
|----------|-------|---------|
| **x3proofengine.md** | 14,569 | Complete ProofForge specification (v0-v7) |
| **X3_PROOFFORGE_COMPLETE_MANIFEST.md** | 400 | What was built, statistics, go-live workflow |
| **X3_PROOFFORGE_QUICK_START.md** | 350 | How to USE ProofForge, common workflows |
| **X3_PROOFFORGE_INDEX.md** | 200 | This file — navigation guide |

---

## 🔍 QUICK TOPIC FINDER

Find what you need in x3proofengine.md:

### ProofForge Core Architecture
- **Proof concept** → Section 1 (v1)
- **Proof levels P0-P7** → Section 2 (v1)
- **Proof scoring formula** → Section 3 (v1)
- **Severity gates S0-S4** → v2 sections 8-9

### Testing Frameworks
- **Unit + integration testing** → v1 sections 4-5
- **Adversarial testing** → v1 section 5 + v5 section 5
- **Mutation testing** → v2 section 10
- **Stateful fuzzing** → v3 section 7
- **Model-based testing** → v3 section 8
- **Chaos testing** → v3 section 10
- **Fuzz orchestration** → v3 section 9

### Proof Modules (25+)
- **Asset kernel** → v1 section 4 + v5 section 6
- **Bridge safety** → v1 section 4 + v5 section 6
- **Cross-VM atomicity** → v1 section 4 + v5 section 6
- **Consensus** → v1 section 4 + v5 section 6
- **Governance** → v5 section 6 + "Legal Proof" section
- **DEX/Launchpad** → "Launchpad Proof" section
- **Oracle** → "Oracle Proof" section (25+)
- **Treasury** → "Treasury Proof" section (25+)
- **X3VM + X3Lang** → "X3 Stack" section (18+)
- **Flashloans** → v18 section (18+)

### Critical Security Topics
- **Supply conservation** → v1 section 4
- **Replay prevention** → v5 section 6
- **Double mint protection** → v5 section 6
- **Governance safety** → v5 section 6
- **Finality verification** → v5 section 6
- **AI patch firewall** → v5 section 10
- **Dependency/supply chain audit** → v5 section 11
- **Secrets & key management** → v5 section 12
- **Runtime upgrade safety** → v5 section 13
- **Blast radius controls** → v5 section 14

### Operational Safety (v6)
- **Safe defaults** → v6 section 3
- **Preflight checks** → v6 section 7
- **Wrong network protection** → v6 section 5
- **Big red button mode** → v6 section 6
- **Freshness proof** → v6 section 8
- **Operator levels I0-I10** → v6 section 2

### Launch & Deployment (v7)
- **Unified CLI architecture** → v7 section 1
- **Proof registry format** → v7 section 2
- **Unified dashboard** → v7 section 3
- **CI/CD gates** → v7 section 4
- **Testnet deployment** → v7 section 5
- **Mainnet decision tree** → v7 section 6
- **Mainnet gate receipt** → v7 section 7
- **Go-live checklist** → v7 section 8
- **CLI reference** → v7 section 10

### Edge Case & Degradation
- **Edge case levels E0-E10** → v3 section 2
- **Boundary value testing** → v3 section 11
- **State machine testing** → v3 section 3
- **Race conditions** → v3 section 12
- **Reorg simulation** → v3 section 3
- **Degradation levels D0-D10** → v4 section 2
- **Limp modes** → v4 section 4
- **Quarantine system** → v4 section 9
- **Crash-point testing** → v4 section 8

---

## 🎯 BY PROOF LEVEL

### Proof Level P (Compile → Launch)
| Level | Meaning | Where to Read |
|-------|---------|---------------|
| P0 | Claimed | v1 section 2 |
| P2 | Compiles | v1 section 2 |
| P3 | Unit tested | v1 section 2 |
| P4 | Integration tested | v1 section 2 |
| P5 | Invariant proven | v2 section 10 + v3 section 1 |
| P6 | Adversarial tested | v5 section 1 |
| P7 | Launch proven | v7 section 1 |

### Proof Level E (Edge Cases)
| Level | Meaning | Where to Read |
|-------|---------|---------------|
| E0 | Claimed | v3 section 2 |
| E3 | Max+1 tested | v3 section 11 |
| E8 | Chaos proven | v3 section 10 |
| E10 | Operationally proven | v3 section 9 |

### Proof Level H (Hack Resistance)
| Level | Meaning | Where to Read |
|-------|---------|---------------|
| H0 | Claimed secure | v5 section 2 |
| H3 | Permission tested | v5 section 6 |
| H5 | Attack simulated | v5 section 5 |
| H8 | Red team partial | v5 section 17 |
| H10 | External audit passed | v5 section 17 |

### Proof Level I (Operator Safety)
| Level | Meaning | Where to Read |
|-------|---------|---------------|
| I0 | Manual chaos | v6 section 2 |
| I4 | Preflight checks | v6 section 7 |
| I6 | Role-aware controls | v6 section 3 |
| I9 | Proof-gated | v7 section 4 |
| I10 | One-button safe | v6 section 4 |

---

## 🔐 BLOCKERS & SEVERITY

### Blocker Levels
| Level | Impact | Block | Read |
|-------|--------|-------|------|
| S0 | Catastrophic (can break chain) | All merges | v2 section 9 |
| S1 | Critical (production blocker) | All releases | v2 section 9 |
| S2 | High (mainnet blocker) | Mainnet only | v2 section 9 |
| S3 | Medium (warning) | Track | v2 section 9 |
| S4 | Low (note) | Track | v2 section 9 |

### Known Blocker Types
- Double mint paths → v5 section 6
- Panic paths → v5 section 6 + v6 section 5
- Unproven claims → v2 section 2
- Missing negative tests → v2 section 3
- Supply drift → v1 section 4 + v3 section 5
- Replay vulnerabilities → v5 section 6
- Unauthorized operations → v5 section 6

---

## 📊 DASHBOARDS & REPORTS

### Main Dashboard
- **File:** `/proof/unified/status.json`
- **Shows:** Proof scores by area, blockers, status
- **Updated:** Every CI run
- **Check:** Section v7.3 (Unified Dashboard)

### Mainnet Gate Receipt
- **File:** `/proof/reports/mainnet_gate.json`
- **Shows:** Final verdict (PASS/BLOCKED/CANDIDATE)
- **Format:** Section v7.7 (Mainnet Gate Receipt Format)

### Blocker Report
- **File:** `/proof/reports/blocker_report.md`
- **Shows:** All S0/S1/S2 issues with fixes
- **Updated:** Every proof run

### Proof Debt Tracking
- **File:** `/proof/reports/proof_debt.md`
- **Shows:** Missing tests, weak tests, mock debt
- **Reference:** v2 section 6 (Proof Debt)

---

## 🎛️ COMPLETE CLI COMMAND MAP

### Verification Commands
```
x3-proof verify [--area] [--strict]              # v7.10
x3-proof hack [--all] [--strict]                 # v7.10
x3-proof edgecase [--all] [--chaos]              # v7.10
x3-proof limp [--all] [--chaos]                  # v7.10
x3-proof idiot [--all] [--preflight]             # v7.10
```

### Specialized Proofs
```
x3-proof formal [--all]                          # v7.10
x3-proof oracle [--strict]                       # v7.10
x3-proof composition [--strict]                  # v7.10
x3-proof treasury [--strict]                     # v7.10
x3-proof custody [--strict]                      # v7.10
```

### Deployment
```
x3-proof testnet [--deploy]                      # v7.5
x3-proof mainnet [--strict] [--fail-hard]        # v7.6
x3-proof civilization [--strict]                 # v7.10
x3-proof go-live [--mainnet] [--ceremony-date]   # v7.8
```

### Monitoring
```
x3-proof daily-verify                            # v7.10
x3-proof weekly-security                         # v7.10
x3-proof monthly-chaos                           # v7.10
x3-proof quarterly-civilization                  # v7.10
```

See Section v7.10 for full reference.

---

## 🚀 LAUNCH CHECKLIST

### Before Testnet
- [ ] Read v1-v2 of x3proofengine.md
- [ ] Understand proof levels P0-P7
- [ ] Run `x3-proof verify --strict`
- [ ] Review `/proof/reports/blocker_report.md`
- [ ] Fix all S0/S1 blockers

### Before Testnet Deploy
- [ ] All S0 blockers fixed
- [ ] `x3-proof testnet --deploy` passes
- [ ] 24-hour stability run complete
- [ ] Attack simulations pass
- [ ] Recovery drills pass

### Before Mainnet Candidate
- [ ] Read v1-v7 of x3proofengine.md
- [ ] Understand all proof modules
- [ ] `x3-proof mainnet --strict --fail-hard` shows CANDIDATE
- [ ] All S1 blockers fixed
- [ ] Formal proofs pass
- [ ] Ceremony team ready

### Before Mainnet Launch
- [ ] Go-live checklist completed (v7.8)
- [ ] `x3-proof go-live --mainnet` authorized
- [ ] Genesis verified
- [ ] Validators enrolled
- [ ] Community notified
- [ ] Support team ready
- [ ] Launch! 🎉

See v7.8 (Go-Live Checklist) for complete details.

---

## 🆘 TROUBLESHOOTING & FAQ

### Common Questions
| Q | A | Read |
|---|---|------|
| What's a proof level? | P0-P7 rating system | v1 section 2 |
| How do I fix S0 blocker? | Check blocker report, follow fix command | v7.6 |
| Can I merge with warnings? | S3/S4 yes, S0/S1/S2 no | v2 section 9 |
| How do I add a feature? | Write test first, then code | Quick Start workflow 1 |
| What if prod goes down? | Run `x3-proof emergency` | v6 section 6 |
| Is mainnet ready? | Check `/proof/unified/status.json` | v7 section 3 |
| How do I launch? | `x3-proof go-live --mainnet` | v7 section 8 |

### Emergency Procedures
- **Supply leak detected** → v1 section 4 + v4 section 9 (Quarantine)
- **Bridge panicked** → v6 section 5 (Wrong Network) + v6 section 6 (Big Red)
- **Malicious upgrade attempted** → v5 section 13 (Runtime Upgrade)
- **Key compromise** → v5 section 12 (Secrets & Keys)
- **Oracle manipulation** → "Oracle Proof" section

---

## 📞 SUPPORT MATRIX

| Issue | Read | Command |
|-------|------|---------|
| Proof failing | Quick Start troubleshooting | `x3-proof verify --verbose` |
| Blocker unclear | Blocker report | `cat /proof/reports/blocker_report.md` |
| Can't merge | Check severity | `grep -A5 "S0\|S1" /proof/reports/blocker_report.md` |
| Need help | Complete Index | You're reading it! |
| Emergency | v6 section 6 | `x3-proof emergency --help` |

---

## 📈 STATISTICS AT A GLANCE

| Metric | Value | Reference |
|--------|-------|-----------|
| Total specification lines | 14,569 | x3proofengine.md |
| Proof versions | 7 (v0-v7) | v1-v7 sections |
| Proof modules | 25+ | "More X3 Proofs" section |
| CLI commands | 40+ | v7 section 10 |
| Blocker types | 5 (S0-S4) | v2 section 9 |
| Proof levels | 18 | All v sections |
| Supported areas | 25+ | v7 section 2 registry |
| Attack personas | 6 | v5 section 4 |

---

## 🎯 NEXT STEPS

### If you're starting out
1. Read Quick Start guide
2. Read v1-v2 of x3proofengine.md
3. Run `x3-proof verify --strict`
4. Start writing proofs!

### If you're reviewing a PR
1. Check CI proof results
2. Review blocker report
3. Approve if S0/S1 pass
4. Merge if all tests green

### If you're preparing mainnet
1. Read entire x3proofengine.md (v1-v7)
2. Run `x3-proof mainnet --strict`
3. Fix any blockers
4. Run `x3-proof go-live --mainnet`

### If you're launching today
1. Verify go-live checklist (v7.8)
2. Run final proof: `x3-proof go-live --mainnet --ceremony-date TODAY`
3. Announce launch
4. Monitor continuously

---

## 🔗 CROSS-REFERENCES

### Proof Dependencies
```
v0 (Base) → v1 (Levels) → v2 (Debt)
                       ↘
                         v3 (EdgeCase)
                       ↙
v4 (Degradation) ← linked through invariants
                       ↓
v5 (Security) ← attacks on degraded systems
                       ↓
v6 (Safety) ← operator safety during failures
                       ↓
v7 (Mainnet) ← everything proven
```

### Area Dependencies
```
Asset Kernel ← feeds supply to Bridge
Bridge ← feeds finality to Consensus
Consensus ← gates Runtime Upgrades
Runtime ← runs Governance
Governance ← manages Treasury
Treasury ← funds Operations
```

---

## 📖 READING ORDER BY DEPTH

### Executive Level (15 min)
1. This index
2. X3_PROOFFORGE_QUICK_START.md overview
3. `/proof/unified/status.json` dashboard

### Developer Level (1-2 hours)
1. X3_PROOFFORGE_QUICK_START.md
2. x3proofengine.md v1-v2
3. x3proofengine.md v5 (security focus)

### Architect Level (4-6 hours)
1. Complete x3proofengine.md v1-v7
2. X3_PROOFFORGE_COMPLETE_MANIFEST.md
3. All proof specifications

### Launch Authority (Full depth)
1. All documents
2. Understand every proof level
3. Memorize go-live checklist
4. Know every blocker type

---

## ✅ FINAL CHECKLIST

- [ ] You can explain P, E, H, I levels
- [ ] You know the difference between S0-S4
- [ ] You can run `x3-proof verify --strict`
- [ ] You understand the mainnet gate
- [ ] You know the go-live procedure
- [ ] You've read the quick start guide

**If yes to all:** You're ready to use ProofForge! 🚀

---

*Last updated: 2026-04-26*
*X3 ProofForge v7 — Complete & Production Ready*

**YOLO DON'T STOP UNTIL YOUR GPU BURNS UP!** 🔥

**X3 PROOF OR GTFO** ✅
