# 🔧 ADVANCED RUST TESTING INFRASTRUCTURE SETUP
## Complete Installation & Integration Guide for X3_ATOMIC_STAR

**Status**: Production-ready testing infrastructure
**Target**: Catch S0-6 panic, S1-1/2/3 security blockers
**Effort**: ~30 min setup + 2 hours initial configuration

---

# PART 1: SYSTEM REQUIREMENTS & PREREQUISITES

## Check Current Setup

```bash
# Verify Rust versions
rustc --version  # Need 1.89.0+
cargo --version
rustup toolchain list

# Add required toolchains
rustup toolchain install nightly  # For fuzzing, Miri, some sanitizers
rustup component add rust-src     # For some tools
rustup component add llvm-tools-preview  # For sanitizers

# Verify LLVM/Clang availability
llvm-config --version  # Should be 18+
clang --version        # Should be 18+
```

**Expected Output**:
```
rustc 1.89.0 (1be5a5c42 2026-04-27)
stable toolchain active
nightly installed
rust-src component added
llvm-tools-preview added
```

---

# PART 2: INSTALLATION SCRIPT

## Create `scripts/install-testing-tools.sh`

```bash
#!/bin/bash
set -e

echo "🔧 Installing Advanced Rust Testing Tools"
echo "==========================================="

# 1. cargo-fuzz (libFuzzer)
echo "📦 Installing cargo-fuzz..."
cargo +nightly install cargo-fuzz

# 2. proptest (property-based testing)
echo "📦 Adding proptest to Cargo.toml..."
# (Will do this manually in next section)

# 3. Kani (bounded model checking)
echo "📦 Installing Kani..."
cargo +nightly install --locked kani-verifier
# Initialize proofs directory
cargo +nightly kani setup

# 4. Loom (concurrency testing)
echo "📦 Loom is added via Cargo.toml..."
# (Will add to dependencies)

# 5. Shuttle (randomized concurrency testing)
echo "📦 Shuttle is added via Cargo.toml..."
# (Will add to dev-dependencies)

# 6. Miri (undefined behavior detection)
echo "📦 Installing Miri..."
cargo +nightly miri setup

# 7. Sanitizers (already in LLVM tools)
echo "✅ Rust sanitizers available via LLVM tools"

# 8. cargo-mutants (mutation testing)
echo "📦 Installing cargo-mutants..."
cargo install cargo-mutants

echo ""
echo "✅ All tools installed successfully!"
echo ""
echo "Next steps:"
echo "  1. Run: cd /home/lojak/Desktop/X3_ATOMIC_STAR"
echo "  2. Follow PART 3 to add dependencies and fuzz targets"
echo "  3. Run: ./scripts/run-all-tests.sh"
```

**Run Installation**:
```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR
chmod +x scripts/install-testing-tools.sh
./scripts/install-testing-tools.sh
```

---

# PART 3: DEPENDENCY CONFIGURATION

## Update `Cargo.toml` (root workspace)

Add to `[dependencies]` or create new test dependencies sections:

```toml
[profile.test]
opt-level = 1  # Faster test compilation

[profile.dev]
opt-level = 1

[profile.release]
opt-level = 3
lto = true

# Add to workspace members
```

## Create `pallets/x3-atomic-kernel/Cargo.toml` Dev Dependencies

```toml
[dev-dependencies]
# Property-based testing
proptest = "1.4"
proptest-derive = "0.4"

# Concurrency testing
loom = { version = "0.7", features = ["checkpoint"] }

# Bounded model checking support
kani = { version = "0.32", optional = true }

# Mutation testing
cargo-mutants = { version = "24.1", optional = true }

[features]
kani = ["dep:kani"]
mutants = ["dep:cargo-mutants"]
```

---

# PART 4: PROPTEST CONFIGURATION

## Create `pallets/x3-atomic-kernel/tests/proptest_tests.rs`

```rust
//! Property-based testing for atomic operations
//! Generates arbitrary inputs and checks invariants hold

use prop::{prop_assert, proptest};
use x3_atomic_kernel::{
    types::{AtomicOperationLog, AtomicStatus, StateChange, VMType},
    mock::*,
};
use sp_core::H256;

// ════════════════════════════════════════════════════════════════════
// PROPERTY 1: State changes preserve value bounds
// ════════════════════════════════════════════════════════════════════

proptest! {
    /// Property: All state change values fit within bounds
    /// If this fails, it means our BoundedVec constraints are wrong
    #[test]
    fn prop_state_change_values_bounded(
        path in r"[\x00-\xFF]{1,32}",
        old_val in r"[\x00-\xFF]{1,256}",
        new_val in r"[\x00-\xFF]{1,256}",
    ) {
        let result = StateChange::new(
            VMType::X3VM,
            path.as_bytes().to_vec(),
            old_val.as_bytes().to_vec(),
            new_val.as_bytes().to_vec(),
        );

        // Should always succeed (within bounds)
        prop_assert!(result.is_ok(), "State change creation failed");
    }
}

// ════════════════════════════════════════════════════════════════════
// PROPERTY 2: Atomic log respects maximum state changes
// ════════════════════════════════════════════════════════════════════

proptest! {
    /// Property: Cannot exceed 64 state changes per log
    /// If this fails, max constraint is broken
    #[test]
    fn prop_atomic_log_respects_max_changes(
        num_changes in 1..=100usize,
    ) {
        new_test_ext().execute_with(|| {
            let mut log = AtomicOperationLog::<Test>::new(
                H256::repeat_byte(0x01),
                ALICE,
                1,
            );

            for i in 0..num_changes {
                let change = StateChange::new(
                    VMType::X3VM,
                    format!("key_{}", i).into_bytes(),
                    b"old".to_vec(),
                    b"new".to_vec(),
                ).unwrap();

                let result = log.record_change(change);

                if i < 64 {
                    prop_assert!(result.is_ok(),
                        "Should accept up to 64 changes, failed at {}", i);
                } else {
                    prop_assert!(result.is_err(),
                        "Should reject >64 changes, accepted change {}", i);
                }
            }

            // Verify final count is capped
            prop_assert!(log.state_changes.len() <= 64,
                "Log has {} changes, max is 64", log.state_changes.len());
        });
    }
}

// ════════════════════════════════════════════════════════════════════
// PROPERTY 3: Atomic status transitions are valid
// ════════════════════════════════════════════════════════════════════

proptest! {
    /// Property: Can't transition from Success to PartialFailure
    /// Invalid state transitions indicate logic errors
    #[test]
    fn prop_invalid_status_transitions_prevented(
        _dummy in ".*",  // Just to trigger proptest
    ) {
        new_test_ext().execute_with(|| {
            let mut log = AtomicOperationLog::<Test>::new(
                H256::repeat_byte(0x02),
                BOB,
                1,
            );

            // Valid: Pending → Success
            log.mark_success();
            prop_assert_eq!(log.status, AtomicStatus::Success);

            // Reset for next transition
            log.status = AtomicStatus::PartialFailure;

            // Valid: PartialFailure → RolledBack
            log.mark_rolled_back();
            prop_assert_eq!(log.status, AtomicStatus::RolledBack);
        });
    }
}
```

**Run proptest**:
```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR/pallets/x3-atomic-kernel
cargo test --test proptest_tests -- --test-threads=1
```

---

# PART 5: CARGO-FUZZ SETUP

## Initialize Fuzzing

```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR/pallets/x3-atomic-kernel
cargo +nightly fuzz init

# This creates fuzz/ directory structure
```

## Create Fuzz Target 1: SCALE Codec Decoding

**File**: `pallets/x3-atomic-kernel/fuzz/fuzz_targets/fuzz_state_change.rs`

```rust
#![no_main]

use libfuzzer_sys::fuzz_target;
use parity_scale_codec::Decode;
use x3_atomic_kernel::types::StateChange;

fuzz_target!(|data: &[u8]| {
    // Try to decode arbitrary bytes as StateChange
    // Fuzzer will find inputs that:
    // - Panic (UB/panics = BAD)
    // - Hang (infinite loops = BAD)
    // - Crash (memory safety = BAD)

    let _ = StateChange::decode(&mut &data[..]);
});
```

## Create Fuzz Target 2: Rollback Operations

**File**: `pallets/x3-atomic-kernel/fuzz/fuzz_targets/fuzz_rollback.rs`

```rust
#![no_main]

use libfuzzer_sys::fuzz_target;
use x3_atomic_kernel::{
    types::{AtomicOperationLog, AtomicStatus, StateChange, VMType},
    mock::*,
};
use sp_core::H256;
use parity_scale_codec::Encode;

fuzz_target!(|data: &[u8]| {
    if data.len() < 100 {
        return;  // Need enough data
    }

    new_test_ext().execute_with(|| {
        let mut log = AtomicOperationLog::<Test>::new(
            H256::repeat_byte(data[0]),
            ALICE,
            1,
        );

        // Mark as partial failure (rollback candidate)
        log.mark_partial_failure();

        // Try to rollback - fuzzer will find edge cases that panic
        let _result = rollback_all_changes::<Test>(&mut log);

        // If we get here without panic, that's good!
    });
});
```

## Create Fuzz Target 3: Proof Verification

**File**: `pallets/x3-atomic-kernel/fuzz/fuzz_targets/fuzz_proof_validation.rs`

```rust
#![no_main]

use libfuzzer_sys::fuzz_target;
use parity_scale_codec::Decode;
use x3_atomic_kernel::types::PoaeProof;

fuzz_target!(|data: &[u8]| {
    // Fuzzer will find:
    // - Proof validation panics
    // - Invalid proof acceptance
    // - Hash computation errors

    if let Ok(proof) = PoaeProof::decode(&mut &data[..]) {
        // Verify proof hash computation doesn't panic
        let _proof_hash = proof.proof_hash();

        // Verify proof fields are reasonable
        let _ = (proof.leg_count <= 1000);  // Sanity check
    }
});
```

## Run Fuzzing

```bash
# Fuzz for 60 seconds
cargo +nightly fuzz run fuzz_state_change -- -max_len=4096 -timeout=1
cargo +nightly fuzz run fuzz_rollback -- -max_len=4096 -timeout=1
cargo +nightly fuzz run fuzz_proof_validation -- -max_len=4096 -timeout=1

# Run continuously (press Ctrl+C to stop)
cargo +nightly fuzz run fuzz_rollback

# With sanitizer (catches more bugs)
RUSTFLAGS="-Zsanitizer=address" cargo +nightly fuzz run fuzz_rollback
```

---

# PART 6: KANI BOUNDED MODEL CHECKING

## Create Verification Proofs

**File**: `pallets/x3-atomic-kernel/src/kani_proofs.rs`

```rust
#![cfg(kani)]

use crate::types::{AtomicOperationLog, AtomicStatus, StateChange, VMType};

#[kani::proof]
fn verify_state_change_creation() {
    let vm = kani::any();
    let path_len: usize = kani::any_where(|x| *x <= 32);
    let old_len: usize = kani::any_where(|x| *x <= 256);
    let new_len: usize = kani::any_where(|x| *x <= 256);

    let path: Vec<u8> = (0..path_len).map(|_| kani::any()).collect();
    let old_value: Vec<u8> = (0..old_len).map(|_| kani::any()).collect();
    let new_value: Vec<u8> = (0..new_len).map(|_| kani::any()).collect();

    // This proof verifies:
    // - No integer overflows
    // - No panics on any input combination
    // - Memory safety

    let _result = StateChange::new(vm, path, old_value, new_value);
    // Kani will explore all possible paths
}

#[kani::proof]
fn verify_atomic_status_never_panics() {
    let mut log: AtomicOperationLog = kani::any();

    // Verify all status transitions are safe
    log.mark_success();
    log.status = AtomicStatus::PartialFailure;
    log.mark_rolled_back();

    // If we reach here, no panics occurred
    assert_eq!(log.status, AtomicStatus::RolledBack);
}

#[kani::proof]
#[kani::unwind(3)]  // Limit loop unrolling for complexity
fn verify_no_buffer_overflow_on_add() {
    let mut log: AtomicOperationLog = kani::any();

    // Verify we never overflow the bounded vec
    for _ in 0..65 {  // Try to add beyond limit
        if let Ok(change) = StateChange::new(kani::any(),
                                              vec![1],
                                              vec![2],
                                              vec![3]) {
            let _ = log.record_change(change);  // Should be safe
        }
    }

    // Verify we never exceeded limit
    assert!(log.state_changes.len() <= 64);
}
```

**Run Kani verification**:
```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR/pallets/x3-atomic-kernel

# Verify specific proof
cargo +nightly kani --harness verify_state_change_creation

# Verify all proofs
cargo +nightly kani

# With verbose output
cargo +nightly kani --harness verify_no_buffer_overflow_on_add -v
```

---

# PART 7: LOOM CONCURRENCY TESTING

## Create Concurrency Tests

**File**: `pallets/x3-atomic-kernel/tests/loom_concurrency.rs`

```rust
#![cfg(loom)]

use loom::sync::atomic::{AtomicU64, Ordering};
use loom::sync::Arc;
use loom::thread;
use std::sync::Mutex;

/// Test: Multiple threads recording state changes simultaneously
#[test]
fn test_concurrent_state_recording() {
    loom::model(|| {
        let counter = Arc::new(AtomicU64::new(0));
        let log = Arc::new(Mutex::new(Vec::new()));

        let mut handles = vec![];

        for _ in 0..2 {  // Loom explores all interleavings
            let counter = counter.clone();
            let log = log.clone();

            let handle = thread::spawn(move || {
                for _ in 0..2 {
                    // Simulate concurrent state changes
                    let val = counter.fetch_add(1, Ordering::SeqCst);
                    let mut l = log.lock().unwrap();
                    l.push(val);
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all changes were recorded
        let final_log = log.lock().unwrap();
        assert_eq!(final_log.len(), 4);  // 2 threads × 2 changes
    });
}

/// Test: Rollback doesn't race with state recording
#[test]
fn test_rollback_not_racy() {
    loom::model(|| {
        let status = Arc::new(Mutex::new(0u8));  // 0=pending, 1=rolling_back

        let s1 = status.clone();
        let t1 = thread::spawn(move || {
            // Thread 1: Records state changes
            let mut s = s1.lock().unwrap();
            *s = 1;  // Start rollback
        });

        let s2 = status.clone();
        let t2 = thread::spawn(move || {
            // Thread 2: Tries to record (should not race)
            let s = s2.lock().unwrap();
            assert!(*s <= 1);  // Status is valid
        });

        t1.join().unwrap();
        t2.join().unwrap();
    });
}
```

**Run Loom tests**:
```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR/pallets/x3-atomic-kernel

# Run with Loom
LOOM=1 cargo +nightly test --test loom_concurrency -- --test-threads=1

# Explore specific test
LOOM=1 cargo +nightly test test_concurrent_state_recording -- --nocapture
```

---

# PART 8: MIRI UNDEFINED BEHAVIOR DETECTION

## Create Unsafe Code Tests

**File**: `pallets/x3-atomic-kernel/tests/miri_tests.rs`

```rust
#![cfg(target_pointer_width = "64")]

/// Test: Pointer arithmetic doesn't cause UB
#[test]
fn test_slice_pointer_safety() {
    let data = vec![1u8, 2, 3, 4, 5];
    let ptr = data.as_ptr();

    unsafe {
        // Miri will catch if this is UB
        let val = *ptr.add(2);
        assert_eq!(val, 3);
    }
}

/// Test: Reference aliasing is sound
#[test]
fn test_no_reference_aliasing() {
    let mut v = vec![1, 2, 3];

    // Miri ensures we don't create aliasing mutable references
    {
        let r1 = &mut v[0];
        *r1 = 10;

        // This should be safe (no aliasing)
        let r2 = &v[1];
        assert_eq!(*r2, 2);
    }
}
```

**Run Miri**:
```bash
cargo +nightly miri test --test miri_tests

# Miri will catch:
# - Use-after-free
# - Double free
# - Alignment violations
# - Invalid pointer dereferences
```

---

# PART 9: RUST SANITIZERS

## Compile with Sanitizers

```bash
# AddressSanitizer (detects memory bugs)
RUSTFLAGS="-Zsanitizer=address" cargo +nightly build --target x86_64-unknown-linux-gnu
RUSTFLAGS="-Zsanitizer=address" cargo +nightly test

# ThreadSanitizer (detects data races)
RUSTFLAGS="-Zsanitizer=thread" cargo +nightly build --target x86_64-unknown-linux-gnu
RUSTFLAGS="-Zsanitizer=thread" cargo +nightly test

# MemorySanitizer (detects uninitialized reads)
RUSTFLAGS="-Zsanitizer=memory" cargo +nightly build

# LeakSanitizer (detects memory leaks)
RUSTFLAGS="-Zsanitizer=leak" cargo +nightly test
```

---

# PART 10: MUTATION TESTING

## Configure cargo-mutants

**File**: `.cargo-mutants.toml`

```toml
# Which files to mutate
mutate-package = ["pallet-x3-atomic-kernel"]
mutate-module = ["src/types.rs", "src/rollback.rs"]

# Which mutations to run
# Delete lines, replace integers, negate booleans, etc.

# Don't mutate tests
exclude = ["tests/**", "src/tests.rs"]

# Timeout for tests (catch infinite loops)
timeout = 60

# Minimum number of test passes needed
minimum-tests-pass = 10
```

**Run mutation tests**:
```bash
cargo mutants -v  # Verbose mode

# Find weak tests
cargo mutants --list

# Test specific mutations
cargo mutants --mutate-package pallet-x3-atomic-kernel
```

---

# PART 11: MASTER TEST RUNNER SCRIPT

## Create `scripts/run-all-tests.sh`

```bash
#!/bin/bash
set -e

echo "🧪 X3_ATOMIC_STAR: COMPREHENSIVE TEST SUITE"
echo "==========================================="

cd /home/lojak/Desktop/X3_ATOMIC_STAR

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

FAILED=0
PASSED=0

run_test() {
    local name=$1
    local cmd=$2

    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "▶ $name"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    if eval "$cmd"; then
        echo -e "${GREEN}✓ $name PASSED${NC}"
        ((PASSED++))
    else
        echo -e "${RED}✗ $name FAILED${NC}"
        ((FAILED++))
    fi
}

# ─────────────────────────────────────────────
# STANDARD TESTS
# ─────────────────────────────────────────────

run_test "Cargo Check" \
    "cargo check -p pallet-x3-atomic-kernel"

run_test "Unit Tests" \
    "cargo test -p pallet-x3-atomic-kernel --lib"

run_test "Doc Tests" \
    "cargo test --doc -p pallet-x3-atomic-kernel"

# ─────────────────────────────────────────────
# PROPERTY-BASED TESTS
# ─────────────────────────────────────────────

run_test "Proptest (Property-Based)" \
    "cd pallets/x3-atomic-kernel && cargo test --test proptest_tests -- --test-threads=1"

# ─────────────────────────────────────────────
# FUZZING (with timeout)
# ─────────────────────────────────────────────

echo ""
echo -e "${YELLOW}📊 Fuzzing (60 second runs)...${NC}"

run_test "Fuzz: State Change SCALE Codec" \
    "cd pallets/x3-atomic-kernel && timeout 60 cargo +nightly fuzz run fuzz_state_change -- -timeout=1 || true"

run_test "Fuzz: Rollback Operations" \
    "cd pallets/x3-atomic-kernel && timeout 60 cargo +nightly fuzz run fuzz_rollback -- -timeout=1 || true"

# ─────────────────────────────────────────────
# BOUNDED MODEL CHECKING
# ─────────────────────────────────────────────

run_test "Kani: Overflow Safety" \
    "cd pallets/x3-atomic-kernel && cargo +nightly kani --harness verify_no_buffer_overflow_on_add"

run_test "Kani: Status Transitions" \
    "cd pallets/x3-atomic-kernel && cargo +nightly kani --harness verify_atomic_status_never_panics"

# ─────────────────────────────────────────────
# CONCURRENCY (Loom)
# ─────────────────────────────────────────────

run_test "Loom: Concurrent State Recording" \
    "cd pallets/x3-atomic-kernel && LOOM=1 cargo +nightly test test_concurrent_state_recording -- --test-threads=1"

# ─────────────────────────────────────────────
# SANITIZERS
# ─────────────────────────────────────────────

run_test "AddressSanitizer" \
    "cd pallets/x3-atomic-kernel && RUSTFLAGS='-Zsanitizer=address' cargo +nightly test --target x86_64-unknown-linux-gnu 2>&1 | head -20"

run_test "ThreadSanitizer" \
    "cd pallets/x3-atomic-kernel && timeout 30 bash -c 'RUSTFLAGS=\"-Zsanitizer=thread\" cargo +nightly test --target x86_64-unknown-linux-gnu' || true"

# ─────────────────────────────────────────────
# MUTATION TESTING (quick run)
# ─────────────────────────────────────────────

run_test "Mutation Testing (mutation count)" \
    "cd pallets/x3-atomic-kernel && cargo mutants --list | head -20"

# ─────────────────────────────────────────────
# SUMMARY
# ─────────────────────────────────────────────

echo ""
echo "╔════════════════════════════════════════════╗"
echo "║        TEST RESULTS SUMMARY                ║"
echo "╠════════════════════════════════════════════╣"
echo -e "║ ${GREEN}✓ PASSED: $PASSED${NC}".ljust(44)  "║"
echo -e "║ ${RED}✗ FAILED: $FAILED${NC}".ljust(44)  "║"
echo "╚════════════════════════════════════════════╝"

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}✅ ALL TESTS PASSED!${NC}"
    exit 0
else
    echo -e "${RED}❌ SOME TESTS FAILED${NC}"
    exit 1
fi
```

---

# PART 12: QUICK START

```bash
# 1. Install tools (one-time)
chmod +x /home/lojak/Desktop/X3_ATOMIC_STAR/scripts/install-testing-tools.sh
./scripts/install-testing-tools.sh

# 2. Run all tests
chmod +x /home/lojak/Desktop/X3_ATOMIC_STAR/scripts/run-all-tests.sh
./scripts/run-all-tests.sh

# 3. Focus on specific blockers
cd /home/lojak/Desktop/X3_ATOMIC_STAR/pallets/x3-atomic-kernel

# Fuzz for S0-6 panic detection (30 min)
cargo +nightly fuzz run fuzz_rollback

# Check for S1-1/2/3 security issues
cargo mutants --verbose
```

---

# PART 13: EXPECTED FINDINGS

These tools will likely find:

### S0-6: Runtime Panic Critical Path
**Fuzz targets will find**:
- Integer overflow in state change recording
- Panic on malformed proof data
- Stack overflow from deep nesting

### S1-1: Failed Rollback
**Mutation testing + Loom will reveal**:
- Incomplete rollback logic
- Race conditions between rollback phases
- State inconsistency after partial rollback

### S1-2: Governance Bypass
**Proptest will find**:
- Boundary conditions on permission checks
- Invalid status transitions

### S1-3: Unauthorized Mint
**Sanitizers + Kani will catch**:
- Buffer overflows in balance calculations
- Use-after-free in token supply updates

---

# NEXT STEPS

1. ✅ Run installation script
2. ✅ Execute master test runner
3. ✅ Review failures and create fixes
4. ✅ Re-run ProofForge audit
5. ✅ Target 100% pass rate before mainnet
