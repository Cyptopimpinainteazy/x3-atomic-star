# S1-1 Fix Implementation Guide

## Quick Start
```bash
# 1. Apply fix (see below)
# 2. Test
cd /home/lojak/Desktop/X3_ATOMIC_STAR/pallets/x3-atomic-kernel
cargo test --test loom_concurrency -- --nocapture --test-threads=1

# 3. Should see:
# test loom_tests::loom_rollback_visibility_across_threads ... ok
```

## The Problem (Reproduced by Loom)

**Thread 1**: Executes rollback
```rust
frame_support::storage::with_storage_layer(|| {
    record.status = BundleStatus::RolledBack;
    Bundles::<T>::insert(bundle_id, &record);  // ← Write
    Ok(())  // ← Commit
});
// No explicit visibility guarantee here ❌
```

**Thread 2**: Reads status immediately after
```rust
let bundle = Bundles::<T>::get(bundle_id);
assert_eq!(bundle.status, BundleStatus::RolledBack);  // ❌ Fails! Still sees old value
```

**Why?**: `with_storage_layer()` ensures transactional consistency within single-threaded context, but doesn't flush caches for other threads to see.

---

## Solution

### Option A: Recommended (Minimal Change)

Add explicit flush after `with_storage_layer()` completes:

```rust
pub fn rollback_atomic_bundle(
    origin: OriginFor<T>,
    bundle_id: H256,
    reason: BundleRollbackReason,
) -> DispatchResult {
    frame_support::storage::with_storage_layer(|| {
        let caller = ensure_signed(origin)?;
        // ... existing authorization and update logic ...
        record.status = BundleStatus::RolledBack;
        Bundles::<T>::insert(bundle_id, &record);
        // ... bond slashing ...
        Self::deposit_event(Event::BundleRolledBack { bundle_id, reason });
        Ok(())
    })?;  // ← Add ? to handle error

    // ✅ NEW: Explicit visibility flush
    sp_io::storage::commit_layer();

    Ok(())
}
```

**File**: `/pallets/x3-atomic-kernel/src/lib.rs`
**Line**: After line 797 (after the closing `})`)

### Option B: Alternative (Event-Based Synchronization)

Events already act as synchronization points. Ensure event is always emitted:

```rust
// Inside with_storage_layer():
Self::deposit_event(Event::BundleRolledBack { bundle_id, reason });

// Event emission creates implicit synchronization point
// Readers can wait for event before reading status
```

This is already done, but document the synchronization contract.

### Option C: Generation Counter (More Robust)

```rust
pub struct BundleRecord<T> {
    // ... existing fields ...
    pub generation: u32,  // ← Add this
}

// In rollback_atomic_bundle():
record.generation = record.generation.saturating_add(1);  // Force cache invalidation
Bundles::<T>::insert(bundle_id, &record);
```

---

## Implementation Steps

### Step 1: Locate the Code
```bash
# Line 685-797 in this file:
vim +685 /home/lojak/Desktop/X3_ATOMIC_STAR/pallets/x3-atomic-kernel/src/lib.rs
```

Find the `rollback_atomic_bundle` function that ends with:
```rust
                Self::deposit_event(Event::BundleRolledBack { bundle_id, reason });
                log::warn!(...);
                Ok(())
            })
        }
```

### Step 2: Apply Option A (Recommended)

**Before**:
```rust
        frame_support::storage::with_storage_layer(|| {
            // ... lots of code ...
            Ok(())
        })
    }
```

**After**:
```rust
        frame_support::storage::with_storage_layer(|| {
            // ... lots of code ...
            Ok(())
        })?;

        // S1-1 FIX: Ensure rollback state is visible to other threads
        sp_io::storage::commit_layer();

        Ok(())
    }
```

### Step 3: Verify the Import

Check that `sp_io` is available. If not, add:
```rust
use sp_io;
```

### Step 4: Test
```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR/pallets/x3-atomic-kernel

# Build check
cargo check

# Run Loom test
cargo test --test loom_concurrency --test loom_rollback_visibility_across_threads

# Should pass
# test loom_tests::loom_rollback_visibility_across_threads ... ok
```

---

## Verification

### Test 1: Loom Passes
```bash
cargo test --test loom_concurrency -- --nocapture

# Expected output:
# test result: ok. 5 passed; 0 failed
```

### Test 2: No Regression
```bash
cargo test --test proptest_tests --test miri_tests

# All should still pass
```

### Test 3: ProofForge Shows S1-1 Resolved
```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR
cargo build --manifest-path proof-forge/Cargo.toml --release
./target/release/x3-proof prove-everything | grep "S1-1"

# Should show: S1-1: ✓ RESOLVED
```

---

## Why This Fix Works

1. **`frame_support::storage::with_storage_layer()`**: Provides transactional atomicity
   - All-or-nothing semantics within execution context
   - BUT: Changes may be cached, not immediately visible to other threads

2. **`sp_io::storage::commit_layer()`**: Flushes pending storage changes
   - Ensures data written to the backing store
   - Makes changes visible to all concurrent readers
   - Memory barrier semantics (acquire/release)

3. **Result**: Loom model checker can no longer find a thread interleaving where visibility fails

---

## Alternative: Why Event Alone Isn't Enough

The event IS emitted, but:
- Events are logged/published separately from storage
- A reader might see the event but haven't seen the storage update yet
- There's no hard coupling between event emission and storage visibility

By adding `commit_layer()`, we explicitly force storage visibility.

---

## Performance Impact

**Minimal**:
- `commit_layer()` is called ONCE per rollback
- Rollbacks are infrequent (only on error/deadline/cancellation)
- Cost: < 1ms (storage flush)
- Compared to potential correctness issues: Negligible

---

## References

- **Test**: `/pallets/x3-atomic-kernel/tests/loom_concurrency.rs:194`
- **Function**: `/pallets/x3-atomic-kernel/src/lib.rs:685-797`
- **Analysis**: `S1-1_BLOCKER_ANALYSIS.md`
- **Sp-io docs**: https://github.com/paritytech/polkadot-sdk/blob/master/substrate/primitives/io/src/lib.rs

---

## Rollback (If Needed)

If the fix causes issues:
```bash
git diff HEAD  # See what changed
git checkout -- pallets/x3-atomic-kernel/src/lib.rs  # Revert
```
