# S1-1 Blocker: Failed Rollback - Synchronization Issue

**Status**: ✅ FOUND by Loom Concurrency Test

## Discovery
- **Test**: `loom_concurrency.rs::loom_tests::loom_rollback_visibility_across_threads`
- **Panic Message**: "Change not visible - synchronization issue (S1-1)"
- **Severity**: CRITICAL - Atomicity violation in concurrent rollback

## Root Cause Analysis

### What the Test Found
The Loom test explores all possible thread interleavings when:
1. **Thread 1**: Makes a state change (status = RolledBack)
2. **Thread 2**: Attempts to read that change from storage

**Result**: Thread 2 does not see the change, indicating a visibility/synchronization issue.

### Code Location
File: `/pallets/x3-atomic-kernel/src/lib.rs`
Function: `rollback_atomic_bundle()` (lines 685-797)
Key Line: 752 - `Bundles::<T>::insert(bundle_id, &record);`

### Why It's Happening

The rollback logic wraps state updates in `frame_support::storage::with_storage_layer()`:

```rust
frame_support::storage::with_storage_layer(|| {
    // ... authorization checks ...
    record.status = BundleStatus::RolledBack;
    Bundles::<T>::insert(bundle_id, &record);  // ← Storage write

    // ... bond slashing ...

    Ok(())  // ← Transaction commits
})
```

**Issue**: While `with_storage_layer()` ensures atomicity within a single thread's execution context, it does NOT provide cross-thread memory visibility guarantees when:

1. Multiple threads access the same bundle storage concurrently
2. One thread writes (rollback) while another thread reads (monitors status)
3. The storage layer changes may not be visible across thread boundaries without explicit synchronization

### Synchronization Gap

In Substrate/Frame, storage operations use caching layers that work well within a single execution context but may create visibility issues across concurrent accesses:

1. **Local cache**: Thread A writes to storage cache
2. **No explicit memory barrier**: Cache hasn't been flushed with visibility guarantees
3. **Thread B reads**: Sees old cached value instead of Thread A's write
4. **Result**: Failed atomicity detection

## Impact

### What Can Go Wrong
- Concurrent rollback requests may observe stale bundle status
- Partial bundle state rollbacks (some fields updated, others not)
- Two threads execute the same bundle (atomicity violation)
- Settlement engine processes already-rolled-back bundles

### Current Workaround
The function uses `with_storage_layer()` for transaction atomicity at the pallet level, but this doesn't guarantee cross-thread visibility. The runtime level relies on block-based execution sequencing (single-threaded within a block), so this issue primarily manifests in:
- Concurrent external queries to bundle status
- Off-chain indexers reading simultaneously with rollbacks
- Test frameworks like Loom that explore interleavings

## Fix Strategy

### Solution: Ensure Read-After-Write Visibility
Add explicit synchronization to guarantee changes are visible to subsequent reads:

**Option A** (Recommended - Minimal Change):
After `with_storage_layer()` completes, explicitly flush/commit any pending state changes with a visibility barrier:
```rust
frame_support::storage::with_storage_layer(|| { ... Ok(()) });
// Explicit flush to ensure visibility across threads
sp_io::storage::commit_layer();
```

**Option B** (More Robust - Mutation Ordering):
Add a generation counter that increments on rollback, forcing cache invalidation:
```rust
record.status = BundleStatus::RolledBack;
record.generation += 1;  // Force cache invalidation
Bundles::<T>::insert(bundle_id, &record);
```

**Option C** (Best Practice - Explicit Synchronization):
Use a dedicated synchronization event that readers must wait for:
```rust
Self::deposit_event(Event::BundleRolledBack { ... });
// Event acts as synchronization point
```

## Implementation Plan

1. **Locate** the exact visibility issue in Frame's storage layer
2. **Test** the fix with Loom to verify all thread interleavings pass
3. **Verify** no performance regression from synchronization overhead
4. **Audit** other storage operations for similar visibility issues

## Related Issues
- **S0-6**: Runtime panics (may occur if visibility not guaranteed)
- **G2**: Storage consistency (related cross-VM verification)

## References
- Test: `/pallets/x3-atomic-kernel/tests/loom_concurrency.rs:180-210`
- Implementation: `/pallets/x3-atomic-kernel/src/lib.rs:685-797`
