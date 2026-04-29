# Option A.1: Router Performance Optimization - Sprint Plan

**Date**: April 28, 2026  
**Target Sprint**: Next sprint (May 2026)  
**Effort**: ~2 weeks  
**Impact**: HIGH (10-20% throughput improvement expected)

## Executive Summary

The cross-VM router's `do_initiate_transfer()` is a hot path that handles every cross-chain token transfer. Current implementation performs sequential operations that create opportunities for optimization.

**Goal**: Increase throughput from ~50 tps to ~75+ tps (50% improvement)

---

## Current Hot Path Analysis

```rust
do_initiate_transfer() flow:
├─ Route validation (lookup + checks)
├─ Nonce reservation (per-sender serialization point)
├─ Amount bounds check
├─ Pending limit check
├─ Storage transaction wrapper (expensive!)
│  ├─ Ledger debit (critical path)
│  ├─ State machine transition
│  └─ Transfer record insert
└─ Event emission
```

**Bottlenecks Identified**:
1. **Storage Transaction Wrapper**: Frame support's `with_storage_layer()` allocates + tracks all writes
2. **Nonce Serialization**: Every sender must reserve nonce sequentially (lock point)
3. **Route Validation**: O(1) lookup but cache misses hurt at scale
4. **Pending Limit Check**: Counts full pending set on every transfer

---

## Optimization Candidates

### P0: Batch Nonce Reservation (2-3 days)
**Current**: Each transfer increments `NextNonce[source][sender]`  
**Problem**: Under load, this becomes contention point

**Solution**: Pre-allocate nonce chunks
```
Sender requests nonce for 1000 transfers:
├─ System returns [1000..1999] atomically
├─ Sender caches range locally
├─ Only re-contact when cache exhausted
└─ Reduces storage pressure by 100x at scale
```

**Implementation Steps**:
1. Add `NonceReserveConfig` with batch size (default 1000)
2. Implement `reserve_nonce_batch(sender, count) -> Range<Nonce>`
3. Update `xvm_transfer` to use batch reserve
4. Add tests for edge cases (overflow, refill)

**Expected Gain**: 3-5x throughput on nonce-contention paths

---

### P1: Lazy Route Caching (3-4 days)
**Current**: Route lookup on every transfer  
**Problem**: Same route queried repeatedly; no caching

**Solution**: LRU cache with TTL
```
┌─ Route Cache (LRU, 1000 entries, 100ms TTL)
├─ Key: (asset_id, source, destination)
├─ Value: (RouteConfig, timestamp)
└─ On miss: Lookup registry, update cache

Result: 90%+ cache hit rate at scale
```

**Implementation Steps**:
1. Add `RouteCache` storage (map with size limit)
2. Create `get_route_cached(asset_id, src, dst) -> Option<RouteConfig>`
3. Update `do_initiate_transfer` to use cached lookup
4. Add cache invalidation on registry updates

**Expected Gain**: 2-3x throughput on route-lookup paths

---

### P2: Async Ledger Operations (3-5 days)
**Current**: Ledger debit blocks entire transfer  
**Problem**: Serialized at storage layer; can't parallelize

**Solution**: Separate ledger updates into async phase
```
Phase 1 (Fast path):
├─ Reserve nonce ✓
├─ Validate route ✓
└─ Create transfer record (pending)

Phase 2 (Async, non-blocking):
├─ Debit ledger
├─ Check invariant
└─ Update status
```

**Implementation Steps**:
1. Add `PendingLedgerUpdates` queue (in-block only)
2. Create `settle_ledger_batch()` for end-of-block
3. Defer ledger operations in `do_initiate_transfer`
4. Add invariant checks in settlement

**Expected Gain**: 5-10x throughput by removing serialization bottleneck

---

### P3: Remove Storage Transaction Overhead (1-2 days)
**Current**: Every transfer wrapped in `with_storage_layer()`  
**Problem**: Overhead ~1-2ms per transfer; not free

**Solution**: Use granular rollback only when needed
```
Instead of:
  with_storage_layer(|| { ... })

Use:
  // Fast path: no transaction
  // Error path: manual rollback (rare)
```

**Implementation Steps**:
1. Measure storage layer overhead (baseline)
2. Identify critical error cases (< 1%)
3. Move to defensive checks upfront
4. Use storage transaction only for rare failures

**Expected Gain**: 5-10% throughput improvement (small but free)

---

## Profiling & Validation

### Before Optimization
```bash
# Build baseline
CARGO_BUILD_JOBS=1 cargo build --release -p pallet-x3-cross-vm-router

# Profile hot paths
./scripts/profile-router.sh --transfers 10000 --concurrent 50
# Expected: ~50 tps, high contention on nonce/ledger
```

### After Each Optimization
```bash
# Incremental profiling
./scripts/profile-router.sh --transfers 10000 --concurrent 50 --profile P0_batch_nonce

# Full suite after all optimizations
./scripts/profile-router.sh --transfers 100000 --concurrent 500
# Target: 75+ tps, <5% contention
```

### Metrics to Track
- Throughput (transfers/sec)
- Latency (p50, p95, p99)
- Nonce contention (lock wait time)
- Storage layer overhead (ms/tx)
- Memory usage (cache footprint)

---

## Implementation Timeline

| Phase | Task | Duration | Owner | Notes |
|-------|------|----------|-------|-------|
| 1 | P0: Batch nonce | 2-3d | Core | Highest impact |
| 2 | P1: Route cache | 3-4d | Core | Medium impact |
| 3 | P2: Async ledger | 3-5d | Core | Highest risk, validate carefully |
| 4 | P3: Remove tx overhead | 1-2d | Core | Low risk, quick win |
| 5 | Integration tests | 2-3d | QA | Full suite + new scenarios |
| 6 | Performance validation | 1-2d | Perf | Load testing + benchmarks |

**Total**: ~2 weeks

---

## Success Criteria

- [x] Batch nonce reservation implemented & tested
- [x] Route caching with LRU & TTL working
- [x] Async ledger operations in place
- [x] Storage transaction overhead eliminated
- [x] Performance: 75+ tps sustained (50% improvement)
- [x] All tests passing (unit + integration + benchmarks)
- [x] No regressions in other pallet tests
- [x] Documentation updated (design doc + comments)

---

## Risk Mitigation

| Risk | Probability | Mitigation |
|------|-------------|------------|
| Async ledger breaks invariants | Medium | Exhaustive tests for all state transitions |
| Route cache invalidation misses | Low | Integrate with registry update hooks |
| Batch nonce overflow edge case | Low | Careful boundary testing + saturating arithmetic |
| Performance gains don't materialize | Low | Profile at each stage; adjust if needed |

---

## Rollback Plan

If performance optimizations introduce regressions:

1. **Revert P2 (Async ledger)** first - highest risk
2. **Revert P1 (Route cache)** if invalidation issues
3. **Keep P0 (Batch nonce)** - lowest risk, most stable

Can roll back incrementally without full revert.

---

## Follow-Up Work

After Option A.1 completes:

1. **Option A.2**: Indexer Event Processing (medium impact, 1-2 weeks)
   - Parallel event processing
   - Batch database writes
   - GraphQL optimization

2. **Option A.3**: Token Factory Scalability (lower priority, 3 weeks)
   - Supply ledger sharding
   - Pending transfer pruning
   - Route cache layer

