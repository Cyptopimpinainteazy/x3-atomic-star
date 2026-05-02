# Cargo.toml Dependencies for X3 Advanced Testing

## Instructions

For each crate that needs advanced testing, add the relevant dev-dependencies below.

---

## For Property-Based Testing (proptest)

Add to: `crates/x3-swap-router/Cargo.toml`, `crates/x3-fees/Cargo.toml`, etc.

```toml
[dev-dependencies]
proptest = "1.4"
```

**Affected crates:**
- `crates/x3-swap-router` ← Property tests for swap math
- `crates/x3-fees` ← Property tests for fee calculations
- `crates/x3-atomic-trade` ← Property tests for trade matching
- `crates/cross-vm-bridge` ← Property tests for bridge transfers

---

## For Fuzzing (cargo-fuzz)

Already installed globally. To initialize fuzzing in a crate:

```bash
cd crates/x3-proof && cargo fuzz add extrinsic_parser
cd crates/x3-intent && cargo fuzz add intent_decode  
cd crates/cross-vm-bridge && cargo fuzz add bridge_proof_verify
```

No Cargo.toml changes needed (fuzzing crates auto-managed).

**Fuzzing crates:**
- `crates/x3-proof/fuzz` ← Proof format fuzzing
- `crates/x3-intent/fuzz` ← Intent parsing fuzzing
- `crates/cross-vm-bridge/fuzz` ← Bridge message fuzzing
- `crates/x3-gateway/fuzz` ← RPC payload fuzzing

---

## For Model Checking (Kani)

Kani harnesses go in `src/` with `#[cfg(kani)]` guards.

No dev-dependency needed, but add comment to top of module:

```rust
// Kani proofs in this module:
// - prove_fee_no_overflow: verifies fee calculation never overflows
// - prove_accounting_conserved: verifies fee + output = input
```

**Crates with Kani proofs:**
- `crates/x3-fees` ← Fee overflow proofs
- `crates/x3-atomic-trade` ← Trade settlement proofs
- `crates/x3-slash` ← Slash calculation proofs

---

## For Concurrency Testing (Loom)

Add to test crates: `crates/x3-gateway/Cargo.toml`

```toml
[dev-dependencies]
loom = "0.7"
```

**Affected crates:**
- `crates/x3-gateway` ← Mempool ordering tests (loom)
- `pallets/x3-sequencer` ← Sequencer concurrency tests (loom)

Run with: `RUSTFLAGS="--cfg loom" cargo +nightly test --lib`

---

## For Large-Scale Async Testing (Shuttle)

Add to async crates: `crates/x3-gateway/Cargo.toml`

```toml
[dev-dependencies]
shuttle = "0.7"
tokio = { version = "1", features = ["full"] }
```

**Affected crates:**
- `crates/x3-gateway` ← Async validator/gossip tests
- `crates/x3-relayer` ← Relay async tests
- `crates/x3-sidecar` ← Sidecar service tests

---

## For Mutation Testing (cargo-mutants)

Already installed globally. Create `mutants.toml` at workspace root:

```toml
[cargo-mutants]
# Only mutate critical crates
only = [
  "x3-fees",
  "x3-swap-router", 
  "x3-proof",
  "cross-vm-bridge",
]

# Use 4 parallel jobs
jobs = 4

# Timeout per mutation: 120 seconds
timeout = "120s"

# Test command
test-command = ["cargo", "test", "--lib"]
```

Run with: `cargo mutants --jobs 4`

---

## Summary: Quick Copy-Paste

### For x3-swap-router

```toml
[dev-dependencies]
proptest = "1.4"
```

### For x3-fees

```toml
[dev-dependencies]
proptest = "1.4"
```

### For x3-gateway

```toml
[dev-dependencies]
proptest = "1.4"
loom = "0.7"
shuttle = "0.7"
tokio = { version = "1", features = ["full"] }
```

### For cross-vm-bridge

```toml
[dev-dependencies]
proptest = "1.4"
```

---

## Running Tests After Setup

```bash
# All property tests
cargo test --test prop_*

# Fuzzing
cargo fuzz run extrinsic_parser

# Kani proofs
cargo +stable kani --harness prove_fee_no_overflow

# Loom (exhaustive concurrency)
RUSTFLAGS="--cfg loom" cargo +nightly test --lib loom_

# Shuttle (randomized async)
cargo +nightly test shuttle_

# Mutations
cargo mutants --jobs 4

# All together
./scripts/test-all-advanced.sh
```

---

## Troubleshooting

### "proptest not found"
Add `proptest = "1.4"` to `[dev-dependencies]` in Cargo.toml of the test crate.

### "Kani proofs don't compile"
Ensure Kani harnesses use `#[kani::proof]` attribute and `kani::any()` for symbolic inputs.

### "Loom tests timeout"
Loom exhaustively explores interleavings — this is slow. For quick feedback, use smaller scenarios or switch to Shuttle.

### "Fuzzer not found"
Run `cargo fuzz add <name>` in the crate first to initialize fuzz targets.

### "Sanitizers fail"
Use: `RUSTFLAGS="-Zsanitizer=address" cargo +nightly test --target x86_64-unknown-linux-gnu`
