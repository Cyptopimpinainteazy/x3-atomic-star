# Phase 2 Week 2-3 Implementation Roadmap

## Current Status

✅ **Phase 1 Week 1 Complete**
- Nested lock fix (gpu_memory_pool.rs)
- SeqCst → Relaxed optimization (multi_gpu_dispatcher.rs)  
- E2E TPS benchmark created
- Stress test harness with 5 test cases
- All tests passing

---

## Phase 2 Priority Queue

### Week 2: Real-Time Metrics + Batch Verification

#### 1. Real-Time TPS Sliding Window (6h) - HIGHEST PRIORITY

**Why**: Current metrics are passive. We can't measure TPS during stress tests.

**File**: `crates/x3-gpu-validator-swarm/src/metrics.rs`

**What to implement**:
```rust
pub struct SlidingWindowMetrics {
    window_size_secs: u64,  // e.g., 10 seconds
    submitted_tasks: Arc<VecDeque<(Instant, u64)>>,  // (timestamp, count)
    completed_tasks: Arc<VecDeque<(Instant, u64)>>,
    failed_tasks: Arc<VecDeque<(Instant, u64)>>,
}

impl SlidingWindowMetrics {
    pub fn tps_last_n_secs(&self, n: u64) -> f64 {
        // Calculate TPS over last N seconds
    }
    
    pub fn latency_p50(&self) -> f64 { /* ... */ }
    pub fn latency_p95(&self) -> f64 { /* ... */ }
    pub fn latency_p99(&self) -> f64 { /* ... */ }
}
```

**Integration**:
- Call from orchestrator.rs on task completion
- Export to Prometheus every 1 second
- Use Relaxed atomics (not SeqCst)

**Test**:
```bash
cargo test --test stress_harness stress_test_sustained_30s -- --nocapture
# Should print TPS every 5 seconds during test
```

**Expected Gain**: 
- Can measure TPS degradation over time
- Detect memory leaks (TPS drops after 10-15s)
- Identify GC pauses (TPS spikes)

---

#### 2. Batch Verification Pipeline (5h) - CRITICAL FOR 65K TPS

**Why**: GPU compute is 100µs-1ms, CPU verification is 1-5ms = 5-50× bottleneck.

**File**: `crates/x3-gpu-validator-swarm/src/orchestrator.rs` (new: `verification_pipeline.rs`)

**Current Flow** (sequential):
```
GPU compute (1ms) → CPU verify (5ms) → Network gossip (50-200ms)
```

**Target Flow** (parallelized):
```
Task 1: GPU compute (1ms) → CPU verify (5ms)
Task 2: GPU compute (1ms) → CPU verify (5ms)  ← Execute in parallel!
Task 3: GPU compute (1ms) → CPU verify (5ms)
```

**Implementation**:
```rust
pub struct VerificationPipeline {
    gpu_queue: Arc<Tokio::sync::mpsc::Sender<ComputeTask>>,  // 32 slot buffer
    verify_workers: Vec<JoinHandle<()>>,  // 4-8 CPU verification tasks
}

impl VerificationPipeline {
    pub async fn submit_batch(&self, tasks: Vec<ComputeTask>) {
        // Push all to gpu_queue (GPU compute in parallel)
        // CPU workers automatically pull from compute results
        // Returns as soon as GPU queue accepts (don't wait for CPU)
    }
}
```

**Test**:
```bash
# Without pipeline:
# Submitted: 5000, Completed: 5000, Duration: 25s, TPS: 200

# With pipeline:
# Submitted: 5000, Completed: 5000, Duration: 12.5s, TPS: 400 (+100%)
```

**Expected Gain**: +50-100% throughput

---

#### 3. Latency Percentile Continuous Tracking (4h)

**File**: `crates/x3-gpu-validator-swarm/src/metrics.rs`

**What to add**:
```rust
pub struct LatencyTracker {
    latencies: Arc<tokio::sync::Mutex<Vec<Duration>>>,  // Fixed-size ring buffer
}

impl LatencyTracker {
    pub async fn record(&self, latency: Duration) {
        // Add to ring buffer (keep last 10K samples)
    }
    
    pub async fn percentiles(&self) -> (f64, f64, f64) {
        // Return (p50, p95, p99) in milliseconds
    }
}
```

**Integration**:
- Record in task completion handler
- Export to Prometheus as `x3_latency_p50_ms`, `x3_latency_p95_ms`, `x3_latency_p99_ms`
- Update every 5 seconds

**Test**:
```bash
cargo test --test stress_harness stress_test_10k_tps -- --nocapture
# Output: Latency: p50=1.82ms, p95=2.57ms, p99=2.79ms
```

---

### Week 3: Advanced Optimizations

#### 4. Orchestrator Lock Contention Fix (3h)

**File**: `crates/x3-gpu-validator-swarm/src/orchestrator.rs` (lines 70-75)

**Current**:
```rust
pub struct SwarmOrchestrator {
    validators: RwLock<HashMap<String, Arc<Validator>>>,      // Line 70
    pending_tasks: RwLock<VecDeque<PendingTask>>,             // Line 71
    completed_tasks: RwLock<HashMap<String, TaskResult>>,     // Line 72
    _network: RwLock<Option<NetworkManager>>,                 // Line 73
    // Each write lock: 30-50µs (hot path!)
}
```

**Fix**: Use `DashMap` (concurrent HashMap, no write lock needed)
```rust
use dashmap::DashMap;

pub struct SwarmOrchestrator {
    validators: DashMap<String, Arc<Validator>>,     // No write lock!
    pending_tasks: Arc<Tokio::sync::mpsc::Channel>,  // Queue, not vec
    completed_tasks: DashMap<String, TaskResult>,    // No write lock!
}
```

**Expected Gain**: +20-30% on high concurrency (16+ submitters)

---

#### 5. Priority-Based GPU Allocation (3h)

**File**: `crates/x3-gpu-validator-swarm/src/multi_gpu_dispatcher.rs` (new method)

**Current**: Round-robin (ignores task priority)
```rust
pub fn next_device(&self) -> Option<u32> {
    // Assigns job to GPU 0, 1, 2, 0, 1, 2...
    // Doesn't care if GPU 0 is already at 90% capacity
}
```

**Add**: Load-aware selection
```rust
pub fn next_device_priority(&self, priority: TaskPriority) -> Option<u32> {
    // High priority → least-loaded GPU
    // Normal priority → next in round-robin
    // Low priority → wait if all GPUs busy
}
```

---

## Validation & Testing Strategy

### Step 1: Baseline Measurement (after all Week 2 fixes)
```bash
cd crates/x3-gpu-validator-swarm

# Run 10K TPS for 30 seconds
cargo test --test stress_harness stress_test_sustained_30s -- --nocapture

# Expected output:
# TPS curve: 10K → 10K → 10K (flat = no memory leak)
# Latency: p50=2.0ms, p95=3.0ms, p99=4.0ms (consistent)
```

### Step 2: Identify Bottleneck
```
If TPS drops after 10s → Memory leak (check gc, allocs)
If p99 spikes after 10s → CPU stalled (check verification queue)
If TPS flat but p50 increases → Lock contention (profile)
If TPS doesn't reach 10K → Network/consensus is bottleneck
```

### Step 3: Run 65K TPS Feasibility Test
```bash
# Create test case (add to tests/stress_harness.rs)
#[tokio::test]
#async fn stress_test_65k_tps() {
#    let config = StressTestConfig {
#        target_tps: 65_000,
#        duration_secs: 10,
#        ...
#    };
#}

cargo test --test stress_harness stress_test_65k_tps -- --nocapture
```

---

## Files to Modify (Phase 2)

```
crates/x3-gpu-validator-swarm/src/
├── metrics.rs              (ADD: SlidingWindowMetrics, LatencyTracker)
├── verification_pipeline.rs (NEW: VerificationPipeline struct)
├── orchestrator.rs         (MODIFY: Replace RwLock with DashMap)
└── multi_gpu_dispatcher.rs (ADD: next_device_priority method)

tests/
└── stress_harness.rs       (ADD: stress_test_65k_tps, bottleneck_detection)
```

---

## Expected Performance After Phase 2

### Before Phase 2
- Measured TPS: 29K (mock)
- Bottleneck: Unclear (no metrics)
- Sustained test: Unknown if memory leak exists
- 65K TPS: Untestable

### After Phase 2
- Measured TPS: 50-60K (with batch pipeline)
- Bottleneck: Clearly identified (sliding window metrics)
- Sustained test: 30s proven leak-free
- 65K TPS: Feasibility demonstrated
- Latency: p50<3ms, p99<10ms

---

## Decision Tree for Next Agent

```
IF previous agent ran stress_test_sustained_30s:
  → Check output
  → If TPS flat: Proceed with batch verification pipeline
  → If TPS degrades: Investigate memory leak first
  
IF TPS achieves 50K+:
  → Run stress_test_65k_tps
  → Measure if 65K is achievable
  
IF bottleneck identified:
  → Profile hot path (use cargo flamegraph)
  → Fix highest-impact item first
  → Re-test
```

---

## Quick Start for Phase 2

```bash
# 1. Check what metrics exist
grep -r "Prometheus\|MetricsCollector" crates/x3-gpu-validator-swarm/src/

# 2. Check orchestrator hot path
grep -A 20 "fn process_pending_tasks" crates/x3-gpu-validator-swarm/src/orchestrator.rs

# 3. Run baseline stress test
cd crates/x3-gpu-validator-swarm
cargo test --test stress_harness stress_test_sustained_30s -- --nocapture

# 4. Implement SlidingWindowMetrics (highest priority)
# 5. Integrate into orchestrator (measurement critical path)
# 6. Re-run stress test and confirm TPS tracking works
```

---

## Success Criteria for Phase 2

- [ ] Real-time TPS tracked during stress tests (not after)
- [ ] Latency p50/p95/p99 continuously calculated
- [ ] Batch verification pipeline increases throughput 50%+
- [ ] Orchestrator uses DashMap (no write lock contention)
- [ ] All stress tests pass with 30s sustained load
- [ ] 65K TPS feasibility confirmed (>80% of target)
- [ ] Bottleneck clearly identified and documented
- [ ] Zero memory leaks detected over 30s tests

---

**Expected Delivery**: End of Week 3
**Total Effort**: ~16 hours
**Expected Result**: 65K TPS proven achievable, 1M TPS identified as network-limited

