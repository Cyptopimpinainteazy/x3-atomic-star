// Configuration for X3 testing tools
// Controls behavior of proptest, Kani, Loom, Shuttle, and Miri

# Proptest configuration
# Location: Cargo.toml [profile.test] or PROPTEST_CONFIG env

[proptest]
# Number of test cases to generate
max_cases = 10000

# Maximum shrinking iterations when reducing failing cases
max_shrink_iters = 10000

# Random seed for reproducibility
seed = 1234

---

# Kani configuration
# Location: Cargo.toml with kani::proof attributes

[kani]
# Unwind limits prevent infinite loops
max_unwind = 5

# Enable all checks (overflow, underflow, panic, etc.)
checks = ["all"]

---

# Loom configuration (via test attributes)

#[test]
fn my_loom_test() {
    loom::model(|| {
        // Loom explores all possible interleavings
        // Disable if too slow: RUSTFLAGS="--cfg loom" cargo +nightly test -- --test-threads=1
    });
}

---

# Shuttle configuration (via environment)

# SHUTTLE_SEED=42 - reproducible randomization
# SHUTTLE_DEPRIORITIZED_FALLBACK=true - when stuck

---

# Miri configuration (via MIRIFLAGS)

MIRIFLAGS="-Zmiri-strict-provenance" \
  # Strict pointer arithmetic checking (catches void* errors)

MIRIFLAGS="-Zmiri-ignore-leaks" \
  # Ignore small leaked allocations (noisy in tests)

MIRIFLAGS="-Zmiri-preemption-rate=0" \
  # Disable preemption (speeds up race detection)

---

# Sanitizer configuration (via RUSTFLAGS)

RUSTFLAGS="-Zsanitizer=address" \
  # Address Sanitizer: memory bugs, out-of-bounds

RUSTFLAGS="-Zsanitizer=memory" \
  # Memory Sanitizer: uninitialized memory access

RUSTFLAGS="-Zsanitizer=thread" \
  # Thread Sanitizer: data races

---

# Mutation testing configuration
# Location: mutants.toml (or cargo-mutants CLI)

[cargo-mutants]
# Run only these tests on mutations
test-command = ["cargo", "test", "--lib"]

# Timeout per mutation test
timeout = "120"

# Parallel jobs
jobs = 4

# Only mutate these crates
only = ["x3-fees", "x3-swap-router", "x3-proof"]
