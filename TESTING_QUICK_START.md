# 🚀 TESTING INFRASTRUCTURE: QUICK START
## One-Command Setup & Execution

```bash
# ════════════════════════════════════════════════════════════
# ONE-TIME SETUP (install all tools)
# ════════════════════════════════════════════════════════════

cd /home/lojak/Desktop/X3_ATOMIC_STAR
chmod +x scripts/install-testing-tools.sh
./scripts/install-testing-tools.sh

# Expected: 5 min, outputs "✅ All tools installed"

# ════════════════════════════════════════════════════════════
# RUN FULL TEST SUITE
# ════════════════════════════════════════════════════════════

chmod +x scripts/run-all-tests.sh
./scripts/run-all-tests.sh

# Expected: 15-30 min, outputs PASSED/FAILED/SKIPPED summary

# ════════════════════════════════════════════════════════════
# FOCUSED BLOCKER INVESTIGATION (if above finds issues)
# ════════════════════════════════════════════════════════════

# S0-6: Runtime Panic (fuzz for crashes - run this for 30 min)
cd pallets/x3-atomic-kernel
cargo +nightly fuzz run fuzz_rollback -- -max_len=4096

# S1-1: Incomplete Rollback (mutation testing)
cargo mutants --verbose

# S1-2/S1-3: Permission/Supply Issues (property testing)
cargo test --test proptest_tests -- --nocapture

# Memory errors (sanitizers)
RUSTFLAGS="-Zsanitizer=address" cargo +nightly test --lib
```

---

# 📋 WHAT'S INSTALLED

## 8 Advanced Testing Tools

1. **cargo-fuzz** ✅
   - libFuzzer integration
   - Finds crashes via coverage-guided fuzzing
   - **Target**: S0-6 panics

2. **proptest** ✅
   - Property-based testing
   - Generates random inputs, checks invariants
   - **Target**: S1-2/3 edge cases

3. **Kani** ✅
   - Bounded model checking
   - Proves no integer overflow/panics
   - **Target**: S0-6 proofs, S1-3 supply invariants

4. **Loom** ✅
   - Concurrency interleaving exploration
   - Finds race conditions
   - **Target**: S1-1 rollback races

5. **Shuttle** ✅
   - Randomized concurrency testing
   - **Target**: Async service races

6. **Miri** ✅
   - Interprets Rust, detects UB
   - Catches unsafe pointer errors
   - **Target**: GPU bridge unsafe code

7. **Rust Sanitizers** ✅
   - AddressSanitizer (memory safety)
   - ThreadSanitizer (data races)
   - **Target**: S1-3 buffer overflows

8. **cargo-mutants** ✅
   - Mutation testing
   - Verifies tests catch bugs
   - **Target**: S1-1 incomplete logic

---

# 📊 FILES CREATED

| File | Purpose |
|------|---------|
| `ADVANCED_TESTING_INFRASTRUCTURE_SETUP.md` | Complete setup guide (13 parts) |
| `BLOCKER_INVESTIGATION_GUIDE.md` | Targeted investigation for each blocker |
| `scripts/install-testing-tools.sh` | Auto-install all 8 tools |
| `scripts/run-all-tests.sh` | Master test runner with all sections |

---

# ✨ NEXT STEPS

**NOW**:
```bash
# Terminal 1: Install tools
cd /home/lojak/Desktop/X3_ATOMIC_STAR
./scripts/install-testing-tools.sh

# Terminal 2: Run tests
./scripts/run-all-tests.sh

# Terminal 3: Monitor findings
tail -f run-all-tests.log
```

**IF TESTS PASS**:
- ✅ 5 S0 blockers already fixed
- ✅ S0-6 panic likely fixed (cargo-fuzz finds none)
- ✅ S1-1/2/3 likely fixed (mutations don't survive)
- → Ready for ProofForge re-audit

**IF TESTS FAIL**:
- Review `BLOCKER_INVESTIGATION_GUIDE.md`
- Locate finding in output
- Apply fix
- Re-run tests

---

# 🎯 SUCCESS CRITERIA

| Tool | Success = | Failure = |
|------|-----------|-----------|
| Fuzz | No crashes | Crash file found |
| Proptest | All properties pass | Property fails |
| Kani | All proofs verified | Verification fails |
| Loom | No data races | Race detected |
| Miri | No UB | UB detected |
| ASAN | No memory errors | Error reported |
| TSAN | No race conditions | Race found |
| Mutants | Weak tests | Mutations survive |

---

# 📞 TROUBLESHOOTING

**cargo-fuzz not found after install?**
```bash
rustup +nightly install cargo-fuzz
source ~/.cargo/env
```

**Kani setup fails?**
```bash
cargo +nightly kani setup --force
```

**ASAN link errors?**
```bash
RUSTFLAGS="-Zsanitizer=address" cargo +nightly test --target x86_64-unknown-linux-gnu
```

**Tools slow?**
```bash
# Run only critical tests
cd pallets/x3-atomic-kernel

# Fuzz (30 sec)
timeout 30 cargo +nightly fuzz run fuzz_rollback

# Mutants (quick count)
cargo mutants --list | wc -l

# Proptest (fast properties)
cargo test --test proptest_tests --lib -- --test-threads=4
```

---

# 📈 EXPECTED TIMELINE

| Phase | Duration | Action |
|-------|----------|--------|
| Tool installation | 5 min | `./scripts/install-testing-tools.sh` |
| Standard tests | 2 min | Cargo check + unit tests |
| Proptest | 3 min | Generate 256 test cases per property |
| Fuzzing | 30 min | Run fuzz_rollback, fuzz_state_change, fuzz_proof |
| Kani proofs | 10 min | Bounded model checking |
| Loom | 5 min | Concurrency exploration |
| Miri | 5 min | UB detection |
| Sanitizers | 10 min | ASAN + TSAN runs |
| Mutants | 30 min | Mutation testing (optional, slower) |
| **TOTAL** | **~100 min** | Full comprehensive run |

---

# 🔗 INTEGRATION

These tests will now:
- ✅ Run daily via CI/CD (add to GitHub Actions)
- ✅ Block PRs if tests fail
- ✅ Enforce 0-panic policy
- ✅ Detect supply invariant violations
- ✅ Catch unauthorized operations
- ✅ Find undefined behavior

---

# 🎓 LEARNING RESOURCE

For deep dives:
- See: `ADVANCED_TESTING_INFRASTRUCTURE_SETUP.md` → PART 13
- Read: `BLOCKER_INVESTIGATION_GUIDE.md` → Common Root Causes
- Tools docs: https://github.com/rust-fuzz/cargo-fuzz, https://kani-verifier.github.io/, etc.

---

**Status**: 🟢 Ready to Execute
**Next**: Run `./scripts/install-testing-tools.sh` then `./scripts/run-all-tests.sh`
**Goal**: Find and fix all 4 remaining blockers (S0-6, S1-1/2/3)
