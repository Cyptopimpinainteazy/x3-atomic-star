# Option C: Phase 1 Planning (Substrate Unpin) - Research Roadmap

**Date**: April 28, 2026  
**Target Quarter**: Q2 2026 (May-July)  
**Effort**: ~4-6 weeks research + planning  
**Complexity**: HIGH (affects 91 pins, 154+ dependent crates)

## Executive Summary

Phase 1 aims to unpin Substrate from commit 948fbd2, enabling upgrades to trie-db and uint. However, the Substrate pin exists because of **custom wasm support** requirements. This research phase will determine if later Substrate versions provide equivalent support.

**Feasibility Rating**: 2/5 (HIGH RISK)  
**Current Runway**: 12-24 months (NOT blocking current work)  
**Decision Point**: After research, decide if Phase 1 is viable or defer indefinitely

---

## Why We're Pinned to Substrate 948fbd2

```
Current Setup:
├─ Substrate rev 948fbd2 (custom commit, not on crates.io)
├─ sp-core v21 (custom, doesn't exist on crates.io as 21.x)
├─ wasm support: Custom WASM execution for X3 VM integration
│  └─ Runtime can execute WASM blobs for cross-chain logic
└─ 91 pins across workspace

Why can't we upgrade to crates.io?
├─ sp-core v23.0.0 (latest) has different wasm API
├─ Later Substrate versions don't include X3 wasm integration
└─ Unpin requires finding new base version + porting X3 features
```

---

## Phase 1 Research Scope

### Research Task 1: Substrate Version Audit (Week 1-2)

**Objective**: Find Substrate versions with wasm support >= current level

**Activities**:
1. **Audit Current Wasm Support**
   - [ ] Document all wasm APIs in Substrate 948fbd2
   - [ ] Identify wasm execution layer (which modules)
   - [ ] Measure wasm performance/compatibility requirements
   - [ ] List all X3-specific wasm extensions

2. **Identify Candidate Versions**
   - [ ] Search Substrate releases: v14, v15, v16 (2024-2026)
   - [ ] Check each version for wasm support status
   - [ ] Document what changed in wasm layer per version
   - [ ] Rank by compatibility with current wasm requirements

3. **Detailed Comparison Matrix**
   ```
   Version | Wasm API | Custom Extensions | Risk | Effort
   --------|----------|-------------------|------|--------
   948fbd2 | Current  | X3-patched        | 0    | Baseline
   v14     | ?        | Not available     | ?    | ?
   v15     | ?        | Not available     | ?    | ?
   v16     | ?        | Not available     | ?    | ?
   ```

**Deliverable**: `SUBSTRATE_VERSION_AUDIT.md` with feature matrix

---

### Research Task 2: Dependency Upgrade Matrix (Week 2-3)

**Objective**: For each candidate Substrate version, build upgrade path

**Activities**:
1. **For Each Target Version**:
   - [ ] Build dependency graph (Substrate → sp-core → trie-db → uint)
   - [ ] Identify version constraints
   - [ ] List all 91 pinned crates and their upgrade paths
   - [ ] Detect conflicts/incompatibilities

2. **Create Upgrade Matrix**
   ```
   Target: Substrate v15
   ├─ sp-core: 23.x (available)
   ├─ trie-db: Upgrade from 0.27.1 → 0.29.0+
   │  └─ Requires: hashbrown 0.14+, codec 1.2+
   ├─ uint: Upgrade from 0.4.1 → 0.5.0
   │  └─ No blockers identified
   └─ Dependent crates (sample):
      ├─ pallet-contracts: 0.27.0 → 0.28.0
      ├─ pallet-grandpa: 18.0.0 → 19.0.0
      └─ ... (90+ more crates)
   ```

3. **Conflict Analysis**
   - [ ] Run `cargo tree` for each target
   - [ ] Identify multiple-version conflicts
   - [ ] Categorize by severity (breaking, compatible, optional)
   - [ ] Estimate coordination complexity

**Deliverable**: `SUBSTRATE_UPGRADE_MATRIX.md` with upgrade paths per version

---

### Research Task 3: Critical Path Analysis (Week 3)

**Objective**: Determine minimum crates that must update together

**Activities**:
1. **Dependency Sorting**
   - [ ] Identify critical dependencies (cross-chain router, ledger, registry)
   - [ ] Build dependency DAG
   - [ ] Find strongly connected components
   - [ ] Determine update order

2. **Minimum Update Set**
   ```
   Phase 1a (Must update together):
   └─ Substrate + sp-core + trie-db
      
   Phase 1b (Dependent crates):
   ├─ pallet-* (30+ pallets)
   ├─ crates/x3-* (core X3 types)
   └─ other Substrate-dependent (100+ crates)
   
   Phase 1c (Independent):
   └─ solana-sdk, redis, etc. (upgradeable in parallel)
   ```

3. **Coordination Strategy**
   - [ ] Can we do a 3-phase rollout?
   - [ ] Which crates can be updated in parallel?
   - [ ] Are there circular dependencies?
   - [ ] How do we handle transitive dependencies?

**Deliverable**: `CRITICAL_PATH_ANALYSIS.md` with update sequencing

---

### Research Task 4: Risk Assessment (Week 3-4)

**Objective**: Quantify risks for each upgrade path

**Activities**:
1. **Breaking Change Analysis**
   ```
   For each target Substrate version:
   ├─ API breaking changes (list affected callsites)
   ├─ Trait incompatibilities (need rewrites?)
   ├─ Storage layout changes (migration needed?)
   └─ Performance implications (benchmarks differ?)
   ```

2. **Wasm Support Validation**
   - [ ] Can wasm execution be ported to new version?
   - [ ] What API changes needed?
   - [ ] Performance impact?
   - [ ] New security considerations?

3. **Rollback Complexity**
   - [ ] How hard to revert if problems discovered?
   - [ ] Do we need to maintain both versions for N commits?
   - [ ] Testing strategy for both old + new?

**Deliverable**: `RISK_ASSESSMENT.md` with severity ratings

---

### Research Task 5: Proof of Concept (Week 4)

**Objective**: Validate upgrade theory on small subset

**Activities**:
1. **Select PoC Subset**
   - Pick 5-10 non-critical crates (no cross-dependencies)
   - Example: x3-common, x3-ast, x3-compiler
   
2. **Upgrade Flow**
   ```bash
   git checkout -b poc/substrate-upgrade-v15
   
   # Step 1: Update Substrate pin
   sed -i 's/948fbd2/NEW_COMMIT/g' Cargo.toml
   
   # Step 2: Update sp-core
   cargo update -p sp-core --aggressive
   
   # Step 3: Fix PoC crates one-by-one
   cargo build -p x3-common
   # Fix compilation errors
   # Commit each fix
   
   # Step 4: Run PoC tests
   cargo test -p x3-common
   ```

3. **Document Findings**
   - [ ] What compile errors occurred?
   - [ ] How were they fixed?
   - [ ] Performance impact?
   - [ ] Any unexpected issues?

**Deliverable**: `PoC_BRANCH` + `POC_RESULTS.md`

---

## Detailed Timeline

| Week | Task | Owner | Deliverable |
|------|------|-------|------------|
| W1-2 | Substrate version audit | Research | `SUBSTRATE_VERSION_AUDIT.md` |
| W2-3 | Upgrade matrix build | Research | `SUBSTRATE_UPGRADE_MATRIX.md` |
| W3 | Critical path analysis | Architecture | `CRITICAL_PATH_ANALYSIS.md` |
| W3-4 | Risk assessment | QA/Security | `RISK_ASSESSMENT.md` |
| W4 | PoC testing | Developer | PoC branch + `POC_RESULTS.md` |
| W4-5 | Decision & planning | Leadership | `PHASE_1_DECISION.md` |

**Total**: ~4 weeks

---

## Success Criteria for Phase 1 Research

After completing these tasks, we'll have:

- [x] Complete audit of Substrate versions (v14 → v16)
- [x] Upgrade path for all 91 pins
- [x] Risk analysis with severity ratings
- [x] PoC branch with real integration issues documented
- [x] Decision: Is Phase 1 worth pursuing?

**Decision Gate**:
- If PoC reveals major issues → **Defer Phase 1 indefinitely** (keep runway)
- If PoC is smooth → **Schedule Phase 1 for Q3 2026** (full upgrade)

---

## Expected Outcomes

### Optimistic Scenario (Probability: 30%)
```
Substrate v15 upgrade possible
├─ Wasm support maintained or improved
├─ < 5 breaking changes in critical crates
├─ No new security concerns
└─ Estimated effort: 4 weeks (after Phase 1 research)
```

### Realistic Scenario (Probability: 50%)
```
Substrate v15 possible but with effort
├─ Wasm support requires light porting
├─ 10-20 breaking changes across crates
├─ Some API rewrites needed
├─ Estimated effort: 8-12 weeks (full phase)
```

### Pessimistic Scenario (Probability: 20%)
```
Phase 1 not viable with current constraints
├─ Wasm support degraded or removed in newer versions
├─ Major architectural changes required
├─ Too risky for mainnet codebase
└─ Recommendation: Keep current pin indefinitely
```

---

## Key Questions to Answer

1. **Can we port X3 wasm to new Substrate?**
   - What's the effort?
   - Performance impact?
   - Security review needed?

2. **How many breaking changes in pallets?**
   - Storage layout migrations?
   - Trait bounds changes?
   - Dispatch origin changes?

3. **What's the critical path?**
   - Can we update incrementally?
   - Or must all 91 crates update together?
   - How many crates can parallelize?

4. **Is it worth the effort?**
   - What do we gain? (trie-db features, uint improvements)
   - What do we risk? (stability, security validation)
   - Better to stay pinned?

---

## Next Steps After Research

### If Feasible (PoC Successful):
1. Schedule Phase 1 for Q3 2026
2. Create detailed implementation plan
3. Allocate 6-8 weeks for full upgrade
4. Plan migration period (old + new version running in parallel)

### If Not Feasible:
1. Document why Phase 1 is blocked
2. Evaluate workarounds
3. Keep Substrate pin indefinitely
4. Monitor future Substrate versions for opportunities

---

## Appendix: Tracking Template

```markdown
### Weekly Research Update

**Week**: 2 / 4 (dates)
**Focus**: Substrate version audit + upgrade matrix

**Progress**:
- [ ] Substrate 948fbd2 wasm API documented
- [ ] Versions v14-v16 reviewed
- [ ] Candidate versions: v15 (promising), v16 (risky)
- [ ] Started upgrade matrix for v15

**Blockers**:
- Need: Custom wasm extension docs (in 948fbd2 only?)

**Next Week**:
- Complete upgrade matrix
- Start risk assessment

**Confidence**: 60% (research on track, wasm support unclear yet)
```

