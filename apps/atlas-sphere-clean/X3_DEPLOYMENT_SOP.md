# X3 DEPLOYMENT STANDARD OPERATING PROCEDURES (SOP)

**Version:** 1.0.0  
**Status:** ACTIVE  
**Authority:** DevOps + Engineering Lead  
**Last Updated:** 2026-03-12  

---

## TABLE OF CONTENTS

1. [Pre-Commit Checklist](#pre-commit-checklist)
2. [Debugging CI Failures](#debugging-ci-failures)
3. [Updating the Completion Checklist](#updating-the-completion-checklist)
4. [Adding New Subsystems](#adding-new-subsystems)
5. [Coverage Issues](#coverage-issues)
6. [Release Procedures](#release-procedures)
7. [Emergency Procedures](#emergency-procedures)

---

## PRE-COMMIT CHECKLIST

**Before pushing ANY code:**

```bash
#!/usr/bin/env bash
set -e

echo "🔍 X3 COMMIT VERIFICATION"

# Step 1: Run audit
echo "[1/4] Running structural audit..."
bash scripts/x3_audit.sh || { echo "❌ Audit failed"; exit 1; }

# Step 2: Run minimal build gate
echo "[2/5] Running cargo check..."
cargo check --workspace || { echo "❌ cargo check failed"; exit 1; }

# Step 3: Enforce formatting
echo "[3/5] Running cargo fmt check..."
cargo fmt --all -- --check || { echo "❌ cargo fmt check failed"; exit 1; }

# Step 4: Build TypeScript packages
echo "[4/5] Building packages..."
npm run build:all-packages --if-present || { echo "❌ package build failed"; exit 1; }

# Step 5: Update checklist
echo "[5/5] Verify X3_COMPLETION.md reflects your changes"
echo "   - Mark completed items as ✅"
echo "   - Leave incomplete items as ⬜"
read -p "   Checklist updated? (y/n) " -n 1 -r
echo
[[ $REPLY =~ ^[Yy]$ ]] || { echo "❌ Please update checklist"; exit 1; }

echo "✅ Ready to commit"
git add X3_COMPLETION.md
git commit -m "[audit] Update completion checklist"
```

**Result:** All checks pass → push safely  
**Typical time:** 5-10 minutes (with cache)

**Phase 4+ gates (deferred):** release build, full tests, clippy, launch-validator, WASM checks, coverage enforcement.  
**Toolchain reference:** `rust-toolchain.toml` documents the pinned Rust version for Phase 3.

### Phase 4 Gate Additions (when Phase 4 is active)

```bash
# Build + lint + tests (CI is strict)
cargo build --release --locked --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --release --locked

# Runtime WASM (no_std)
cd runtime && cargo build --release --target wasm32-unknown-unknown --no-default-features

# Launch validator
cargo run -p x3-launch-validator -- --check pre-launch
cargo run -p x3-launch-validator -- --check failure-conditions
```

**CI strictness:** warnings are errors (`-D warnings`). Flaky tests fail immediately (no retries).  
**Local strictness:** use `--strict` to make warnings fail locally.
**Launch-validator defaults:** `target/release/x3-chain-node`, `testnet/genesis.json`, and `prometheus.yml` must exist or the gate fails.

---

## DEBUGGING CI FAILURES

**Phase 3 note:** CI runs `bash scripts/x3_audit.sh --ci` only. Build/test scenarios below apply in Phase 4+.

### Scenario 1: Build Fails (`cargo build --release`)

**Symptom:**
```
❌ Build Integrity job FAILED
Error: cannot resolve type `X`
```

**Steps:**

```bash
# 1. Check which crate failed
gh run view <run-id> --log | grep -A 10 "error\["

# 2. Build locally with same flags
cargo build --release --locked

# 3. Fix compilation errors
# (Edit the offending crate)

# 4. Verify fix
cargo build --release --locked

# 5. Test before pushing
cargo test --all --locked

# 6. Commit and re-run CI
git push origin <branch>
```

**Common causes:**
- Dependency mismatch (fix: `cargo update --locked`)
- Breaking API change (fix: update call sites)
- Unsafe code without `unsafe` block (fix: wrap unsafe operations)

---

### Scenario 2: Tests Fail (`cargo test --all`)

**Symptom:**
```
❌ Test Suite job FAILED
test result: FAILED. X passed; Y failed
```

**Steps:**

```bash
# 1. View failed test names
gh run view <run-id> --log | grep "test.*FAILED"

# 2. Run locally to debug
cargo test --all -- --nocapture | grep -A 20 "FAILED test"

# 3. Inspect test code
# File: crates/<crate>/src/tests.rs or tests/

# 4. Fix the test or feature
# (Debug failures, don't skip tests)

# 5. Run again locally
cargo test --all --locked

# 6. Push for CI
```

**Common causes:**
- Test assumes deterministic seed (fix: use seeded RNG)
- Race condition in async test (fix: add synchronization)
- Test data fixture missing (fix: create test data)
- Flaky test (fix: isolate state, use `tokio::test`)

---

### Scenario 3: Unwrap/Expect Found

**Symptom:**
```
❌ Structural Audit FAILED
Found 5 unwrap() in production code
```

**Steps:**

```bash
# 1. Find all instances
rg 'unwrap\(\)' --glob '!**/tests/**' --type rust

# 2. For each one, decide:
#    Option A: Use Result<T, E> and propagate error
#    Option B: If truly panic-safe, add // SAFETY: comment
#    Option C: Use .expect("reason") with justification

# Example: Change this
let value = option_val.unwrap();

# To this (if fallible):
let value = option_val.ok_or(MyError::NotFound)?;

# Or this (if panic is OK):
let value = option_val.expect("config must be set at init");

# 3. Re-run audit
bash scripts/x3_audit.sh

# 4. Commit
```

**Policy:**
- Production code: `Result<T, E>` required
- Test code: `.unwrap()` is OK (tests can panic)
- Comments required: Every `expect()` needs justification

---

### Scenario 4: Checklist Not Green

**Symptom:**
```
❌ Completion Checklist FAILED
X3 COMPLETION CHECKLIST NOT FULLY GREEN (15 items remaining)
```

**This is expected early in sprints.** Steps:

```bash
# 1. View unchecked items
grep "⬜" X3_COMPLETION.md

# 2. For this PR, update items you completed
# Change: ⬜ → ✅
vim X3_COMPLETION.md

# 3. Verify your update
grep -c "✅" X3_COMPLETION.md   # should increase
grep -c "⬜" X3_COMPLETION.md   # should decrease

# 4. Commit
git add X3_COMPLETION.md
git commit -m "[checklist] Update: feature X complete (5 items ✅)"

# 5. Re-run CI
git push origin <branch>
```

**Note:** It's normal for the checklist to have `⬜` remaining—the gate just ensures awareness.

---

### Scenario 5: Coverage Below Threshold

**Symptom:**
```
⚠️  Coverage Gates
x3-agent: 76.2% < 80% FAILURE
```

**Steps:**

```bash
# 1. Generate coverage report
bash scripts/x3_coverage_gate.sh --report

# 2. Open HTML report
open coverage/index.html   # macOS
# or: xdg-open coverage/index.html   # Linux

# 3. Identify untested code
# (Look for red lines in report)

# 4. Write tests for missing coverage
vim crates/x3-agent/src/lib.rs     # feature code
vim crates/x3-agent/src/tests.rs   # test code

# Example test template:
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_feature() {
        let result = my_function();
        assert_eq!(result, expected);
    }
}

# 5. Re-run coverage
bash scripts/x3_coverage_gate.sh

# 6. Commit tests
git add crates/x3-agent/src/tests.rs
git commit -m "[test] Add coverage for agent budget logic (+5%)"
```

**Coverage targets:**
- Runtime: 95% (consensus-critical)
- Pallets: 90% (state-transition critical)
- VM: 90% (execution-critical)
- Agent: 80% (AI safety)
- SDK: 80% (developer UX)

---

## UPDATING THE COMPLETION CHECKLIST

### Adding a Completed Item

```bash
# 1. Open checklist
vim X3_COMPLETION.md

# 2. Find the section
# e.g.: "3.2 Consensus"

# 3. Change ⬜ to ✅
# Before: | Aura producing blocks correctly | ⬜ | /runtime/src/lib.rs |
# After:  | Aura producing blocks correctly | ✅ | /runtime/src/lib.rs |

# 4. Commit with context
git add X3_COMPLETION.md
git commit -m "[audit] Aura consensus verified ✅

Tests: https://github.com/x3-chain/main/runs/12345
Evidence: pallet_aura tests pass deterministically"

# 5. This will trigger CI re-check
```

### Marking as Incomplete (Regression)

**⚠️ Use carefully—signals something broke.**

```bash
# 1. If a feature regresses:
vim X3_COMPLETION.md

# 2. Change ✅ back to ⬜
# Before: | Aura producing blocks correctly | ✅ | /runtime/src/lib.rs |
# After:  | Aura producing blocks correctly | ⬜ | /runtime/src/lib.rs |

# 3. Commit with incident details
git commit -m "[incident] Aura consensus regression detected ⬜

Root cause: timestamp overflow in block time validation
Restored from: abc123def456
See issue: https://github.com/x3-chain/issues/89"

# 4. This re-opens the GitHub issue
```

---

## ADDING NEW SUBSYSTEMS

### When Adding a New Pallet / Crate

**Steps:**

1. **Create crate**
   ```bash
   cargo new --lib crates/x3-newfeature
   ```

2. **Add to checklist**
   ```bash
   vim X3_COMPLETION.md
   # Add under relevant section:
   # | New feature X | ⬜ | /crates/x3-newfeature/src/lib.rs |
   ```

3. **Create test module**
   ```bash
   cat > crates/x3-newfeature/src/tests.rs << 'EOF'
   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_basic_functionality() {
           // TODO
       }
   }
   EOF
   ```

4. **Add to `Cargo.toml` (workspace)**
   ```toml
   [workspace]
   members = [
       "crates/x3-newfeature",
   ]
   ```

5. **Configure coverage threshold**
   ```bash
   vim Cargo.toml
   # Under [workspace.metadata.coverage]:
   x3-newfeature = 80  # Add line
   ```

6. **Add to CI audit (if needed)**
   ```bash
   vim .github/workflows/x3-audit.yml
   # May need to add job to validate new subsystem
   ```

7. **Document in architecture**
   ```bash
   vim docs/ARCHITECTURE.md
   # Add section explaining new subsystem
   ```

8. **Test locally**
   ```bash
   cargo build --release --locked
   cargo test --all --locked
   bash scripts/x3_audit.sh
   ```

9. **Open GitHub issue for completion**
   ```bash
   python3 scripts/x3_generate_issues.py --dry-run
   # Verify issue would be created, then:
   python3 scripts/x3_generate_issues.py
   ```

---

## COVERAGE ISSUES

### If Coverage Drops

```bash
# 1. Identify drop
bash scripts/x3_coverage_gate.sh

# Example output:
# x3-agent: 82.1% → 79.3% (REGRESSION)

# 2. Find untested code (from coverage report)
open coverage/x3-agent/index.html

# 3. Add tests
vim crates/x3-agent/src/tests.rs

#[test]
fn test_agent_budget_constraint() {
    let agent = Agent::new(Budget { max: 100 });
    assert!(agent.spend(50).is_ok());
    assert!(agent.spend(60).is_err());  // exceeds budget
}

# 4. Verify
bash scripts/x3_coverage_gate.sh

# 5. Commit
git commit -m "[test] Restore agent coverage to 82%"
```

### If You Need to Raise a Threshold

**This should be rare.** Only if code is truly untestable:

```bash
# 1. Justify in code comment
vim Cargo.toml

# [workspace.metadata.coverage]
# x3-agent = 75  # Was 80; lowered because:
#                # - OS-specific code hard to test
#                # - See https://github.com/x3-chain/issues/XX

# 2. Create issue documenting exception
gh issue create \
  --title "[technical debt] Restore x3-agent coverage to 80%" \
  --label "technical-debt" \
  --body "Currently at 75% due to untestable syscall code. Blocked on: https://..."

# 3. Commit
git commit -m "[coverage] Temporarily lower x3-agent threshold to 75%

See: https://github.com/x3-chain/issues/XX
Target: Restore to 80% by sprint 5"
```

---

## RELEASE PROCEDURES

### Pre-Release Checklist (1 week before)

```bash
#!/usr/bin/env bash

echo "🚀 X3 PRE-RELEASE VERIFICATION"

# 1. Verify checklist is green
unchecked=$(grep -c "⬜" X3_COMPLETION.md)
if [ "$unchecked" -gt 0 ]; then
  echo "❌ FAIL: $unchecked items still unchecked in X3_COMPLETION.md"
  grep "⬜" X3_COMPLETION.md
  exit 1
fi
echo "✅ Checklist fully green"

# 2. Verify CI passing on main
gh run list --workflow x3-audit.yml --branch main --limit 1 | grep -i success
echo "✅ Latest CI run passed"

# 3. Verify coverage thresholds
bash scripts/x3_coverage_gate.sh
echo "✅ Coverage gates passed"

# 4. Generate release notes
git log --oneline --since="2 weeks ago" | tee RELEASE_NOTES.tmp

# 5. Tag version
VERSION="v1.0.0"  # Update version
git tag -a "$VERSION" -m "Release: $VERSION"
git push origin "$VERSION"

echo "✅ Release tagged: $VERSION"
```

### Release Day

```bash
#!/usr/bin/env bash

VERSION="v1.0.0"

echo "📦 X3 RELEASE: $VERSION"

# 1. Build release artifacts
cargo build --release --locked

# 2. Sign artifacts
bash scripts/x3_release_sign.sh

# 3. Publish
# (Your CD pipeline handles this)

# 4. Announce
gh release create "$VERSION" --title "Release $VERSION" --notes-file RELEASE_NOTES.tmp

echo "🎉 Released: $VERSION"
```

---

## EMERGENCY PROCEDURES

### Emergency Halt

**If critical bug discovered post-release:**

```bash
#!/usr/bin/env bash

echo "🚨 EMERGENCY: Halting system"

# 1. Kill all nodes
pkill x3-node || true

# 2. Document incident
cat > INCIDENT.log << EOF
Time: $(date)
Severity: CRITICAL
Issue: [DESCRIBE]

Halted by: [YOUR NAME]
Root cause: [TBD]
Remediation: [TBD]
EOF

# 3. Notify team
# (Email, Slack, etc.)

# 4. Prepare rollback
git checkout <previous-stable-tag>

# 5. Re-test
bash scripts/x3_audit.sh
cargo test --all --locked

# 6. If rollback needed
git revert <bad-commit>
git push origin main
```

### Rollback Procedure

```bash
#!/usr/bin/env bash

PREVIOUS_VERSION="v0.9.9"

echo "⏮️  ROLLBACK: $PREVIOUS_VERSION"

# 1. Fetch previous version
git fetch origin refs/tags/$PREVIOUS_VERSION:refs/tags/$PREVIOUS_VERSION

# 2. Checkout
git checkout $PREVIOUS_VERSION

# 3. Verify old build works
bash scripts/x3_audit.sh
cargo test --all --locked

# 4. If good, create rollback commit
git checkout main
git revert --no-edit HEAD  # Revert the bad commit

# 5. Push
git push origin main

# 6. Document
cat > ROLLBACK_LOG.txt << EOF
Rolled back from: [bad version]
Rolled back to: $PREVIOUS_VERSION
Reason: [describe incident]
Date: $(date)

Next steps:
1. Fix root cause on develop branch
2. Run full test suite
3. Merge back to main
EOF

echo "✅ Rollback complete"
```

---

## ROUTINE MAINTENANCE

### Weekly (Every Monday)

```bash
# Update dependencies (non-breaking)
cargo update

# Run full audit
bash scripts/x3_audit.sh

# Check for abandoned crates
cargo deny check advisories

# Review open issues
gh issue list --label "x3,blocking" --state open
```

### Monthly (First day of month)

```bash
# Comprehensive coverage report
bash scripts/x3_coverage_gate.sh --report

# Review completion dashboard
cat X3_AUDIT_DASHBOARD.md

# Update progress tracking
python3 scripts/x3_generate_issues.py
```

### Quarterly (Every 3 months)

```bash
# Full security audit
cargo audit
rustlings clean  # ensure no deprecated patterns

# Review and update checklist
vim X3_COMPLETION.md

# Assess technical debt
gh issue list --label "technical-debt" --state open

# Plan next quarter
# (See X3_COMPLETION.md progress section)
```

---

## QUICK REFERENCE

### Essential Commands

```bash
# Audit
bash scripts/x3_audit.sh

# Test
cargo test --all --locked

# Build
cargo build --release --locked

# Coverage
bash scripts/x3_coverage_gate.sh

# Generate issues
python3 scripts/x3_generate_issues.py

# View CI status
gh run list --workflow x3-audit.yml --limit 5

# View checklist
grep -c "✅" X3_COMPLETION.md      # completed
grep -c "⬜" X3_COMPLETION.md      # remaining
```

---

## ESCALATION

| Issue | Contact | Response Time |
|-------|---------|---|
| Build failing | DevOps | 1 hour |
| Coverage drop | QA Lead | 2 hours |
| Test regression | Engineering | 1 hour |
| Security issue | Security Lead | 30 min |
| Release blocker | Project Lead | immediate |

---

**Document Version:** 1.0.0  
**Last Updated:** 2026-03-12  
**Next Review:** 2026-06-12  
