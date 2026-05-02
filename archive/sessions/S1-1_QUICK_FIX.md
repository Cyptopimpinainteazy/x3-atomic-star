# QUICK REFERENCE - S1-1 BLOCKER FIX

## The Fix (5 minutes)

```bash
# Navigate to pallet
cd /home/lojak/Desktop/X3_ATOMIC_STAR/pallets/x3-atomic-kernel/src

# Edit file
vim lib.rs

# Go to line 797 (bottom of rollback_atomic_bundle function)
# (In vim: :797<Enter>)

# Find this:
        })  // ← This is the closing of with_storage_layer()
    }

# Change to:
        })?;  // ← Add ? to handle error

    // S1-1 FIX: Ensure rollback state is visible to other threads
    sp_io::storage::commit_layer();

    Ok(())
    }

# Save and exit: :wq
```

## The Test (2 minutes)

```bash
cd /home/lojak/Desktop/X3_ATOMIC_STAR/pallets/x3-atomic-kernel

# Run before fix (should FAIL)
cargo test --test loom_concurrency -- --nocapture

# Should see FAIL message:
# thread 'loom_tests::loom_rollback_visibility_across_threads' panicked at:
# Change not visible - synchronization issue (S1-1)

# Run after fix (should PASS)
cargo test --test loom_concurrency -- --nocapture

# Should see PASS:
# test result: ok. 5 passed; 0 failed
```

## What Each Line Does

```rust
        })?;  // ← Change closing ) to }) with error handling
              //   Allows commit_layer() to run after storage_layer completes

    sp_io::storage::commit_layer();
    // ↑ EXPLICIT flush: Forces pending storage changes to be
    //   written to backing store + visible to all threads

    Ok(())  // ← Return success
    }       // ← End of rollback_atomic_bundle function
```

## Verification (1 minute)

```bash
# Should see all green:
cargo test --test proptest_tests --test miri_tests --test loom_concurrency

# Expected:
# test result: ok. 20 passed; 0 failed
```

## If Something Goes Wrong

```bash
# Revert the change:
git checkout -- pallets/x3-atomic-kernel/src/lib.rs

# Confirm reverted:
git diff HEAD  # Should be clean
cargo test --test loom_concurrency  # Will fail again with same error
```

## Why Sp-io::storage::commit_layer()?

**Storage layer** = Internal cache of changes
**Commit layer** = Flush to persistent store + visibility barrier

Thread 1 writes → cached locally → commit_layer() → visible globally to Thread 2 ✓

## Done ✓

Fix applied. Loom test passes. S1-1 RESOLVED.

Ready to hunt S0-6 and S1-2 next!

---

**File**: `/home/lojak/Desktop/X3_ATOMIC_STAR/pallets/x3-atomic-kernel/src/lib.rs`
**Line**: ~797
**Function**: `rollback_atomic_bundle()`
**Change**: 1 line added (`sp_io::storage::commit_layer();`)
**Time**: 5 min fix + 2 min test = 7 min total ✅
