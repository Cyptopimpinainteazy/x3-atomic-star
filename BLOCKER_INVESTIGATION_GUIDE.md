# S0-6 & S1-1,2,3 BLOCKER INVESTIGATION GUIDE
## Using Advanced Testing Tools to Find Remaining Vulnerabilities

**Status**: 8 tools ready; 4 blockers to find and fix
**Timeline**: Investigation (1-2 days) → Fixes → Re-audit
**Success Criteria**: 100% ProofForge pass after fixes

---

# BLOCKER: S0-6 runtime_panic_critical_path

## What It Means
Runtime panics in critical execution paths (rollback, finality, atomic operations) that:
- **Crash validators** (BFT consensus broken)
- **Cause chain halts** (finality stalled)
- **Allow DoS attacks** (craft input → panic → halt)

## How Tools Will Find It

### 🟡 PRIMARY: cargo-fuzz (libFuzzer)
**Why**: Fuzzing generates crash-triggering inputs

```bash
# Fuzz for 5 minutes with crash detection
cd pallets/x3-atomic-kernel
cargo +nightly fuzz run fuzz_rollback -- -max_len=4096 -artifact_prefix=crashes/

# Examine crash inputs
ls -la crashes/crash-*

# Reproduce crash with specific input
cat crashes/crash-xxx | cargo +nightly fuzz run fuzz_rollback -- -artifact_prefix=crashes/
```

**Expected Finding**: Crash log showing which input triggers panic

### 🟡 SECONDARY: AddressSanitizer
**Why**: Detects memory corruption that causes panics

```bash
RUSTFLAGS="-Zsanitizer=address" cargo +nightly test --lib -- --nocapture 2>&1 | grep "panic\|SUMMARY"
```

### 🟡 TERTIARY: ThreadSanitizer
**Why**: Race conditions that cause panics under concurrency

```bash
RUSTFLAGS="-Zsanitizer=thread" cargo +nightly test --lib -- --test-threads=1
```

## Investigation Checklist

```bash
# 1. Check error handling paths
grep -n "panic\|unwrap\|expect" pallets/x3-atomic-kernel/src/rollback.rs

# 2. Run fuzzer and wait for crash
cargo +nightly fuzz run fuzz_rollback 2>&1 | tee fuzz.log

# 3. Inspect crash
file crashes/crash-*
hexdump -C crashes/crash-* | head

# 4. Find crash-triggering code
# Look for: division by zero, out-of-bounds array access, unwrap on None
grep -B5 -A5 "panic\!" pallets/x3-atomic-kernel/src/*.rs
```

## Common Root Causes

| Symptom | Root Cause | Fix |
|---------|-----------|-----|
| `thread panicked: called Option::unwrap() on a None value` | Unwrap on optional value | Use `?` or `.ok_or(Error)` |
| `thread panicked: attempt to divide by zero` | Unchecked divisor | Check divisor != 0 |
| `thread panicked: index out of bounds` | Array bounds violation | Use `.get(idx)` instead |
| `thread panicked: called Result::unwrap() on an Err` | Error not handled | Use error handling |

---

# BLOCKER: S1-1 failed_rollback

## What It Means
Rollback operation fails to completely revert state:
- **Partial rollbacks leave inconsistent state** (some changes reverted, others not)
- **Atomic invariants broken** (supply != sum of balances)
- **Double-spend possible** (tokens in 2 places after rollback)

## How Tools Will Find It

### 🟡 PRIMARY: cargo-mutants (Mutation Testing)
**Why**: Detects incomplete rollback logic by intentionally breaking code

```bash
cd pallets/x3-atomic-kernel

# List all mutations
cargo mutants --list 2>&1 | grep -i rollback

# Run mutations (takes ~10 minutes)
cargo mutants --verbose 2>&1 | tee mutations.log

# If mutation survives (test passes despite mutation), rollback logic is weak
```

**Expected Finding**: Mutations like "removed rollback call" that still pass

### 🟡 SECONDARY: Loom (Concurrency)
**Why**: Race conditions cause partial rollbacks

```bash
# Run Loom concurrency tests
LOOM=1 cargo +nightly test test_rollback_not_racy -- --test-threads=1

# If test fails, race condition exists
```

### 🟡 TERTIARY: proptest (Property-Based)
**Why**: Generates edge cases for rollback

```bash
cargo test --test proptest_tests prop_state_change_values_bounded -- --nocapture
```

## Investigation Checklist

```bash
# 1. Check rollback logic is complete
grep -n "mark_rolled_back\|rollback_all_changes" pallets/x3-atomic-kernel/src/rollback.rs

# 2. Verify all state changes are reverted
grep -A10 "fn rollback_all_changes" pallets/x3-atomic-kernel/src/rollback.rs

# 3. Check for LIFO order (last-in-first-out for safe rollback)
grep -B5 -A5 "for.*reverse\|reversed()" pallets/x3-atomic-kernel/src/rollback.rs

# 4. Run mutation tests and check for survivors
cargo mutants --verbose | grep -i "SURVIVED\|UNVIABLE"

# 5. Verify no panics during rollback that skip later steps
grep -B3 "panic\|return" pallets/x3-atomic-kernel/src/rollback.rs
```

## Common Root Causes

| Symptom | Root Cause | Fix |
|---------|-----------|-----|
| Mutation "removed rollback call" survives | Missing revert for some state | Add revert for all state changes |
| Loom test shows race | Rollback not atomic | Use with_storage_layer() |
| Partial rollback in logs | Early return on error | Complete rollback before error handling |
| Supply invariant fails after rollback | Balance not fully restored | Verify LIFO reversal order |

---

# BLOCKER: S1-2 governance_bypass

## What It Means
Access control insufficient; attackers can:
- **Call privileged extrinsics** without permission
- **Bypass governance requirements** (call admin functions as user)
- **Modify critical settings** (chain parameters, validators)

## How Tools Will Find It

### 🟡 PRIMARY: proptest (Property-Based Testing)
**Why**: Generates boundary-case permission checks

```bash
# Property: Unprivileged user cannot call admin functions
cd pallets/x3-atomic-kernel
cargo test --test proptest_tests prop_governance_boundaries -- --nocapture

# If property fails for any input, governance bypass exists
```

**Expected Finding**: Permission check failed for unauthorized user

### 🟡 SECONDARY: cargo-mutants
**Why**: Detects missing permission checks

```bash
# Mutations like "removed access control check"
cargo mutants --verbose 2>&1 | grep -A3 "access.*check\|permission\|origin"

# If mutation survives, access control is missing
```

### 🟡 TERTIARY: Kani (Bounded Model Checking)
**Why**: Proves all permission paths are correct

```bash
# Create and run Kani proof for permission checks
cargo +nightly kani --harness verify_governance_enforcement
```

## Investigation Checklist

```bash
# 1. Find all privileged extrinsics
grep -n "RollbackOrigin\|ExecutorOrigin\|ensure_" pallets/x3-atomic-kernel/src/lib.rs

# 2. Check each extrinsic has permission guard
for func in rollback_failed_bundle finalize_bundle_with_fallback; do
    grep -B2 "$func" pallets/x3-atomic-kernel/src/lib.rs | grep -i "ensure\|require\|origin"
done

# 3. Look for suspicious permission patterns
grep -E "if|match" pallets/x3-atomic-kernel/src/lib.rs | grep -i "origin\|access"

# 4. Run proptest for bypass attempts
cargo test --test proptest_tests -- --nocapture | grep -i "unauthorized\|bypass"

# 5. Check mutation survival
cargo mutants --verbose 2>&1 | grep -B2 "SURVIVED" | head -20
```

## Common Root Causes

| Symptom | Root Cause | Fix |
|---------|-----------|-----|
| Mutation removes `ensure_origin!()` and still passes | Missing permission check | Add ensure_origin guard |
| Proptest generates unauthorized caller that succeeds | Wrong origin type | Use correct Origin enum |
| Admin function callable by any account | No access control | Require specific role |
| Permission check in wrong order | Early return before check | Move check before state change |

---

# BLOCKER: S1-3 unauthorized_mint

## What It Means
Privilege escalation in token minting:
- **Arbitrary minting** without authorization
- **Unlimited supply increase** (inflate token)
- **Balance modification** without transfer

## How Tools Will Find It

### 🟡 PRIMARY: AddressSanitizer (ASAN)
**Why**: Detects buffer overflows in supply calculations

```bash
RUSTFLAGS="-Zsanitizer=address" cargo +nightly test --lib --target x86_64-unknown-linux-gnu 2>&1 \
    | grep -A5 "SUMMARY\|ERROR"
```

**Expected Finding**: Memory safety error in balance/supply code

### 🟡 SECONDARY: proptest (Supply Invariants)
**Why**: Generates mint amounts and verifies supply = sum(balances)

```bash
# Property: Supply invariant after every operation
cargo test --test proptest_tests prop_supply_invariant_maintained -- --nocapture

# If property fails, unauthorized mint exists
```

### 🟡 TERTIARY: Kani (Bounded Model Checking)
**Why**: Proves minting only increases supply by mint_amount

```bash
cargo +nightly kani --harness verify_supply_only_increases_by_amount
```

## Investigation Checklist

```bash
# 1. Find all mint-related code
grep -rn "mint\|TotalIssuance" pallets/x3-atomic-kernel/src/

# 2. Check mint authorization
grep -B5 "mint" pallets/x3-atomic-kernel/src/lib.rs | grep -i "ensure\|origin\|permission"

# 3. Verify supply calculation
grep -rn "supply\|issuance" pallets/x3-atomic-kernel/src/

# 4. Run ASAN to detect memory errors
RUSTFLAGS="-Zsanitizer=address" cargo +nightly test --lib -- --nocapture 2>&1 | head -100

# 5. Run proptest for supply invariant
cargo test prop_supply_invariant -- --nocapture

# 6. Check for integer overflow in supply
grep -n "checked_add\|saturating_add\|wrapping_add" pallets/x3-atomic-kernel/src/
```

## Common Root Causes

| Symptom | Root Cause | Fix |
|---------|-----------|-----|
| ASAN detects buffer overflow in supply | Unchecked array write | Use safe Vec/BoundedVec |
| Supply invariant fails for arbitrary amounts | Unchecked supply increase | Use checked_add + error |
| Proptest finds minter that's not authorized | Missing permission | Require specific origin |
| Integer overflow in total supply | No overflow check | Use saturating_add/checked_add |

---

# TESTING WORKFLOW SUMMARY

## Step 1: Run Fuzz for S0-6 (Panics)
```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR
cargo +nightly fuzz run fuzz_rollback
# Wait 5-10 min for crash
# If crash found: examine crash file, find code causing panic, fix it
```

## Step 2: Run Mutations for S1-1 (Incomplete Rollback)
```bash
cargo mutants --verbose
# Check for mutations that SURVIVE (test should fail)
# If mutation survives: add missing revert logic
```

## Step 3: Run Proptest for S1-2 & S1-3 (Security)
```bash
cargo test --test proptest_tests -- --nocapture
# Check for permission/supply failures
# If failures: add missing guards/checks
```

## Step 4: Run Sanitizers for S1-3 (Overflow)
```bash
RUSTFLAGS="-Zsanitizer=address" cargo +nightly test --lib --target x86_64-unknown-linux-gnu
# Check for memory errors
# If errors: use safe types (checked_add, BoundedVec)
```

## Step 5: Verify All Fixes
```bash
./scripts/run-all-tests.sh
# All tests should pass
```

## Step 6: Re-Run ProofForge
```bash
cargo build --manifest-path proof-forge/Cargo.toml --release
./target/release/x3-proof prove-everything
# Should show 9/9 blockers RESOLVED
```

---

# QUICK START

```bash
# Terminal 1: Install tools (one-time)
cd /home/lojak/Desktop/X3_ATOMIC_STAR
chmod +x scripts/install-testing-tools.sh
./scripts/install-testing-tools.sh

# Terminal 2: Run main test suite
chmod +x scripts/run-all-tests.sh
./scripts/run-all-tests.sh

# Terminal 3: Focused investigation
cd pallets/x3-atomic-kernel

# Fuzz for S0-6 (run for 30 minutes)
cargo +nightly fuzz run fuzz_rollback 2>&1 | tee fuzz_findings.txt

# Monitor for crashes
tail -f fuzz_findings.txt | grep -i "panic\|crash\|ERROR"
```

---

# TOOL COMPARISON TABLE

| Tool | Best For | Time | Confidence |
|------|----------|------|-----------|
| **cargo-fuzz** | Finding panics | 5-60 min | Very High |
| **proptest** | Logic errors | 1-5 min | High |
| **Kani** | Overflow proofs | 5-30 min | Very High |
| **Loom** | Race conditions | 2-10 min | Very High |
| **Miri** | UB in unsafe | 1-5 min | Very High |
| **ASAN** | Memory safety | 1-5 min | High |
| **TSAN** | Data races | 2-10 min | High |
| **cargo-mutants** | Test quality | 10-60 min | High |

---

# EXPECTED OUTCOMES

After running all tools, you should have:

### ✅ S0-6 Finding
- Crash input that triggers panic
- Stack trace showing code path
- Root cause (division by zero, unwrap, etc.)

### ✅ S1-1 Finding
- Mutation that survives (reveals incomplete rollback)
- State inconsistency after rollback
- Missing revert operation

### ✅ S1-2 Finding
- Permission bypass input (proptest)
- Unauthorized caller that succeeded
- Missing access control guard

### ✅ S1-3 Finding
- ASAN buffer overflow or proptest supply failure
- Unauthorized mint amount
- Missing validation

---

# NEXT: Create Fixes

Once blockers are found, create fixes in:
1. `pallets/x3-atomic-kernel/src/rollback.rs` (S0-6, S1-1)
2. `pallets/x3-atomic-kernel/src/lib.rs` (S1-2, S1-3)
3. Re-run tests to verify fix
4. Run ProofForge audit

Target: **100% pass rate before mainnet**
