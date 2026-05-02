# X3 Advanced Testing Infrastructure

This guide configures property-based testing, fuzzing, model checking, concurrency analysis, UB detection, sanitizers, and mutation testing for X3 blockchain critical paths.

## Quick Start

```bash
# Property-based testing (stable)
cargo test --test prop_* -- --nocapture

# Fuzzing (stable + libFuzzer)
cargo +stable fuzz run x3_swap_fuzzer

# Model checking with Kani (stable)
cargo +stable kani --harness prove_swap_overflow

# Concurrency testing with Loom (nightly)
RUSTFLAGS="--cfg loom" cargo +nightly test --lib concurrency_

# Undefined behavior detection (nightly)
MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test

# Rust sanitizers (stable with explicit flag)
RUSTFLAGS="-Zsanitizer=memory" cargo +nightly build

# Mutation testing (stable)
cargo mutants --in-place -j 4

# All tests
make test-all-advanced
```

---

## 1. Property-Based Testing with `proptest`

**Use for:** asset math, supply invariants, swap paths, fee math, nonce logic

### Setup

Add to relevant crates (e.g., `crates/x3-swap-router/Cargo.toml`):

```toml
[dev-dependencies]
proptest = "1.4"
```

### Example: Swap Router Math Properties

See: `crates/x3-swap-router/tests/prop_swap_math.rs`

Properties tested:
- **Commutative exchange:** `swap(A→B) + swap(B→A)` respects slippage bounds
- **Fee invariant:** fees never exceed input amount
- **No overflow on large inputs:** 128-bit path math is safe
- **Path optimality:** best path has minimum fee

### Running

```bash
# Single property test
cargo test --test prop_swap_math prop_swap_fee_invariant -- --nocapture

# All property tests with more examples
PROPTEST_MAX_SHRINK_ITERS=10000 cargo test --test prop_* -- --nocapture

# With custom seed (reproduce failure)
PROPTEST_REGRESSIONS=prop-regressions.txt cargo test --test prop_swap_math
```

---

## 2. Fuzzing with `cargo-fuzz` / `libFuzzer`

**Use for:** SCALE decoding, bridge proofs, extrinsic parsing, RPC payloads, intent parsing

### Setup

```bash
cd crates/x3-proof && cargo fuzz add extrinsic_parser
cd crates/x3-intent && cargo fuzz add intent_decode
cd crates/cross-vm-bridge && cargo fuzz add bridge_proof_verify
```

### Fuzzer Examples

See: `crates/*/fuzz/fuzz_targets/`

- `extrinsic_parser.rs` — feeds random bytes to extrinsic parser, catches panics/overflows
- `intent_decode.rs` — fuzzes intent SCALE decoding
- `bridge_proof_verify.rs` — fuzzes proof verification with malformed inputs
- `rpc_payload.rs` — fuzzes RPC JSON deserialization

### Running

```bash
# Run single fuzzer with 1M iterations
cargo fuzz run extrinsic_parser -- -max_len=4096 -max_total_time=60

# Run all fuzzers in parallel
for fuzzer in crates/x3-proof/fuzz/fuzz_targets/*.rs; do
  name=$(basename "$fuzzer" .rs)
  timeout 30 cargo fuzz run "$name" -- -max_total_time=30 &
done
wait

# With address sanitizer (catches memory errors)
SANITIZER=address cargo fuzz run extrinsic_parser
```

---

## 3. Bounded Model Checking with `Kani`

**Use for:** overflow, impossible states, bounded loops, accounting correctness

### Setup

```bash
# Already installed, or:
cargo install kani-verifier
```

### Harnesses

See: `crates/x3-fees/src/lib.rs` and `crates/x3-fees/src/proofs.kani`

Properties proven:
- **No overflow in fee calculation:** `fee = (amount * rate / 10000)` safe for all 128-bit inputs
- **Balance conservation:** in atomic swaps, `input - fee = output`
- **Nonce ordering:** nonces strictly increase

### Running

```bash
# Prove single harness
cargo +stable kani --harness prove_fee_no_overflow

# Prove all harnesses in crate
cargo +stable kani

# With detailed counterexample
cargo +stable kani --harness prove_swap_math --no-unwinding-with-non-trivial-loops
```

---

## 4. Concurrency Testing with `Loom`

**Use for:** mempool queues, reservation locks, nonce cache, RPC rotator, async gossip

### Setup

Add to relevant crates:

```toml
[dev-dependencies]
loom = "0.7"
```

### Test Example

See: `crates/x3-gateway/tests/loom_mempool_ordering.rs`

Properties tested:
- **Mempool FIFO ordering:** multiple threads enqueueing, single thread dequeueing maintains order
- **Nonce cache coherency:** concurrent updates to nonce cache don't race
- **Reservation lock safety:** overlapping reservations correctly serialize

### Running

```bash
# Loom tests exhaustively explore thread interleavings
RUSTFLAGS="--cfg loom" cargo +nightly test --lib --test '*loom*' -- --nocapture

# Single loom test (slower, but checks all interleavings)
RUSTFLAGS="--cfg loom" cargo +nightly test loom_mempool_ordering -- --nocapture
```

---

## 5. Randomized Large Concurrency with `Shuttle`

**Use for:** async services, validator workers, gossip tasks, state divergence under load

### Setup

Add to relevant crates:

```toml
[dev-dependencies]
shuttle = "0.7"
```

### Test Example

See: `crates/x3-gateway/tests/shuttle_validator_consensus.rs`

**Scenario:**
- 10 validator workers + 100 gossip tasks
- Random task scheduling (not all interleavings, but larger scale)
- Verify consensus state never diverges

### Running

```bash
# Run with seed 42 (reproducible randomization)
SHUTTLE_SEED=42 cargo +nightly test shuttle_validator_consensus -- --nocapture

# Run 100 times to catch rare races
for i in {1..100}; do
  SHUTTLE_SEED=$i cargo +nightly test shuttle_validator_consensus --quiet || break
done
```

---

## 6. Undefined Behavior Detection with `Miri`

**Use for:** unsafe Rust, pointer-heavy VM/gpu/native code, FFI, networking

### Usage

```bash
# Run single test through Miri
cargo +nightly miri test --lib test_vector_memory_safety

# All tests
MIRIFLAGS="-Zmiri-strict-provenance -Zmiri-ignore-leaks" cargo +nightly miri test

# With poisoned bytes detection (catches use-after-free)
MIRIFLAGS="-Zmiri-preemption-rate=0" cargo +nightly miri test
```

### Example: GPU Bridge FFI

See: `crates/cross-chain-gpu-validator/src/lib.rs`

Miri will catch:
- Pointer arithmetic errors in bridge code
- Out-of-bounds GPU memory access
- Use-after-free in async pinning

---

## 7. Rust Sanitizers

**Use for:** memory bugs, use-after-free, leaks, thread races, integer overflows

### Memory Sanitizer

```bash
RUSTFLAGS="-Zsanitizer=memory" cargo +nightly build --lib -p x3-vm --target x86_64-unknown-linux-gnu
RUSTFLAGS="-Zsanitizer=memory" cargo +nightly test --lib -p x3-vm --target x86_64-unknown-linux-gnu
```

### Address Sanitizer (out-of-bounds, use-after-free)

```bash
RUSTFLAGS="-Zsanitizer=address" cargo +nightly test --lib -p x3-proof --target x86_64-unknown-linux-gnu
```

### Thread Sanitizer (race conditions)

```bash
RUSTFLAGS="-Zsanitizer=thread" cargo +nightly test --lib -p x3-gateway --target x86_64-unknown-linux-gnu
```

### Leak Sanitizer

```bash
RUSTFLAGS="-Zsanitizer=leak" cargo +nightly test --lib -p x3-sidecar --target x86_64-unknown-linux-gnu
```

---

## 8. Mutation Testing with `cargo-mutants`

**Use for:** Validating that your test suite actually catches bugs

### Usage

```bash
# Mutate 4 files in parallel, only run affected tests
cargo mutants --jobs 4 --package x3-swap-router

# Test only specific mutations
cargo mutants --file crates/x3-fees/src/lib.rs

# Show all mutations without running (dry run)
cargo mutants --list

# Keep intermediate files for inspection
cargo mutants --in-place --package x3-proof
```

### What cargo-mutants does:

1. Injects bugs (e.g., `a + b` → `a - b`, `if true` → `if false`)
2. Runs your test suite
3. Reports: tests passed = bad coverage, tests failed = good coverage

### Example Report

```
x3-swap-router:
  ✓ Survived: if remainder == 0 { return } → if false { return } (should test zero output)
  ✗ Killed: amount_out = (amount_in * 997) / 1000 → (amount_in * 999) / 1000
  ✗ Killed: supply_new = supply_old + in_amount → - in_amount
```

The survivors show test gaps!

---

## 9. X3-Specific Test Targets

### Critical Paths to Test

1. **Swap Router Math** (`crates/x3-swap-router`)
   - Property: fee invariant
   - Fuzz: path parsing
   - Kani: overflow proof
   - Mutants: coefficient correctness

2. **Bridge Proofs** (`crates/x3-bridge`, `crates/cross-vm-bridge`)
   - Fuzz: proof verification
   - Property: no double-spend proofs
   - Shuttle: validator consensus under reorg
   - Sanitizers: FFI safety

3. **Mempool / Ordering** (`crates/x3-gateway`, `crates/private-mempool`)
   - Loom: ordering under contention
   - Shuttle: large-scale validator scheduling
   - Mutants: nonce increment logic

4. **Intent Resolution** (`crates/x3-intent`)
   - Fuzz: intent parsing
   - Property: no orphaned intents
   - Kani: state machine proof
   - Mutants: settlement logic

5. **GPU Validator** (`crates/cross-chain-gpu-validator`)
   - Miri: FFI memory safety
   - Address sanitizer: GPU memory bounds
   - Shuttle: validator consensus divergence

---

## 10. Running Everything: Master Test Script

See: `scripts/test-all-advanced.sh`

```bash
./scripts/test-all-advanced.sh
```

This runs:
1. All property tests
2. All fuzzers (30s timeout each)
3. All Kani proofs
4. All Loom concurrency tests
5. All Shuttle randomized tests
6. Miri checks
7. Sanitizers
8. Mutation testing
9. Generates summary report

---

## 11. Interpreting Results

### Property Test Failure

```
thread 'prop_swap_fee_invariant' panicked at 'assertion failed: fee <= input'
Error: Fee exceeded input with amount=1000000000000000000, fee_rate=10001
```

→ **Action:** Fix fee calculation to clamp at 10000 basis points.

### Fuzzer Crash

```
ERROR: libFuzzer: deadly signal
    #0 0x7f... in panic at crates/x3-proof/src/verify.rs:42
    Input: [0xFF, 0xFF, ...]
```

→ **Action:** Add bounds checking before array access.

### Kani Counterexample

```
VERIFICATION FAILED

Counterexample:
  amount = 18446744073709551615
  result = 0 (overflow!)
```

→ **Action:** Use checked arithmetic or prove bounds on inputs.

### Loom Interleaving Failure

```
thread 'loom_mempool' panicked: 'expected nonce order [1, 2, 3], got [2, 1, 3]'
```

→ **Action:** Add mutex/ordering constraint.

### Miri UB Detection

```
error: unsupported operation: pointer arithmetic: 0x... cannot be used for arithmetic
  → crates/x3-vm/src/unsafe_code.rs:99
```

→ **Action:** Use safe pointer operations or prove safety invariant.

### Mutation Survived

```
Mutation SURVIVED: if fee <= max_fee { return Ok(...) } → if true { return Ok(...) }
  at crates/x3-fees/src/lib.rs:87
  No tests caught this!
```

→ **Action:** Add test case with `fee > max_fee` expecting rejection.

---

## 12. CI/CD Integration

### GitHub Actions

Create `.github/workflows/advanced-tests.yml`:

```yaml
name: Advanced Testing

on: [push, pull_request]

jobs:
  properties:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --test prop_* -- --nocapture

  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo fuzz run extrinsic_parser -- -max_total_time=30

  kani:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo kani

  loom:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - run: RUSTFLAGS="--cfg loom" cargo +nightly test --lib concurrency_

  mutants:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo mutants --jobs 2
```

---

## 13. Troubleshooting

### Miri not available on stable

```bash
# Solution: use nightly
cargo +nightly miri test
```

### Kani times out on complex proofs

```bash
# Solution: Bound the problem smaller
#[kani::proof]
#[kani::unwind(3)]  // Limit loop unwindings
```

### Loom tests extremely slow

```bash
# Solution: Test fewer threads/scenarios
#[test]
fn loom_test() {
    loom::model(|| {
        // Loom will explore interleavings automatically
        // Limit the number of threads/operations for speed
    });
}
```

### Sanitizer incompatible with proc-macros

```bash
# Solution: Compile dependencies with sanitizer too
RUSTFLAGS="-Zsanitizer=address" cargo +nightly build --target x86_64-unknown-linux-gnu
```

---

## Next Steps

1. **Start with properties** — easiest to add, catches most bugs
2. **Add fuzzers** for parsers and validators
3. **Prove critical math** with Kani
4. **Test concurrency** with Loom for mempool/bridge
5. **Find UB** with Miri in unsafe code
6. **Close test gaps** with mutation testing

**Goal:** Every critical path has multi-layer testing that can't be bypassed by a single clever attack.
