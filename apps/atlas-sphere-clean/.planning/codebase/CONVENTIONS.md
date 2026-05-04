# X3 Chain Coding Conventions

## Overview

The X3 Chain codebase demonstrates consistent patterns across multiple languages: **Rust** (primary), **TypeScript** (frontend), **Python** (tooling), and **Solidity** (contracts). This document catalogs observed conventions and styling patterns.

---

## Rust Conventions

### Module Organization

Rust modules follow a hierarchical structure with clear separation of concerns:

- **`lib.rs` / `main.rs`** - Module declarations and public re-exports
- **Domain modules** - Feature-specific code (e.g., `authority.rs`, `adapters.rs`, `types.rs`)
- **`tests.rs`** - Integration and behavior tests
- **`mock.rs`** - Test fixtures and mock runtimes (gated with `#[cfg(test)]`)
- **`error.rs`** - Error types and result type aliases
- **`config.rs`** - Configuration structures
- **`benchmarking.rs`** - Runtime benchmarks (feature-gated)

Example structure from `crates/x3-kernel/`:

```
src/
├── lib.rs          # Main module file with inline docs
├── adapters.rs     # VM adapters (EVM, SVM, X3)
├── authority.rs    # Authority set management
├── types.rs        # Domain types
├── tests.rs        # 2000+ line test suite
├── mock.rs         # Test runtime mock (327 lines)
├── error.rs        # Error types
├── benchmarking.rs # Substrate benchmarks
└── migrations.rs   # Runtime migrations
```

### Module-Level Documentation

Public modules include module-level doc comments that explain:
- **Purpose**: What the module provides
- **Architecture**: High-level design choices
- **Examples**: Usage patterns and examples
- **Security notes**: Important safety considerations (marked H-1, H-5, etc.)

**Example from `crates/x3-kernel/src/lib.rs`:**

```rust
//! # X3 Kernel Pallet
//!
//! The core orchestration layer for X3 Chain's dual-VM execution architecture.
//! Enables atomic cross-VM transactions (Comits) that execute on both EVM and SVM.
//!
//! ## Security Design Decisions
//!
//! ### H-1: prepare_root Verification (Input Commitment Design)
//!
//! The `prepare_root` field is a cryptographic commitment to the **input parameters** of a Comit,
//! NOT the execution outputs...
```

**Why this pattern:**
- Helps new maintainers understand design intent and constraints
- Documents security-critical decisions inline with code
- Provides context for design rationale

### Naming Conventions

**Functions and Variables** - `snake_case`:
```rust
pub fn new_test_ext()
pub fn submit_comit()
pub fn compute_prepare_root()
pub async fn apply_certificate()
```

**Types and Structs** - `PascalCase`:
```rust
pub struct ComitV2<AccountId, Balance>
pub struct AtomicExecutionProof
pub enum SwarmError
pub struct MockValidator
```

**Constants** - `UPPER_SNAKE_CASE`:
```rust
pub const MAX_REGISTERS: usize = 256;
pub const MAX_CALL_DEPTH: usize = 64;
pub const DEFAULT_GAS_LIMIT: u64 = 1_000_000;
pub const ALICE: AccountId = 1;
pub const INITIAL_BALANCE: Balance = 1_000_000_000_000;
```

**Type Aliases** - `PascalCase`:
```rust
pub type AccountId = u64;
pub type Balance = u128;
pub type SwarmResult<T> = Result<T, SwarmError>;
pub type CompilerResult<T> = Result<T, CompilerError>;
```

### Error Handling

All errors use the `thiserror` crate with custom error types:

**Pattern: Dedicated `error.rs` module with structured errors**

Files like `crates/gpu-swarm/src/error.rs`, `crates/x3-vm/src/error.rs`, and `crates/x3-compiler/src/error.rs` define:

```rust
use thiserror::Error;

pub type SwarmResult<T> = Result<T, SwarmError>;

#[derive(Error, Debug)]
pub enum SwarmError {
    #[error("Node not found: {}", hex::encode(&.0[..8]))]
    NodeNotFound([u8; 32]),

    #[error("Task not found: {0}")]
    TaskNotFound(Uuid),

    #[error("Insufficient stake: required {required}, have {available}")]
    InsufficientStake { required: u64, available: u64 },

    #[error("Task timed out after {0} seconds")]
    TaskTimeout(u64),
}
```

**Error Struct Pattern** (for complex errors like verifier):

From `crates/x3-vm/src/error.rs`:

```rust
#[derive(Debug)]
pub struct VerifierError {
    pub kind: VerifierErrorKind,
    pub offset: Option<usize>,
}

impl VerifierError {
    pub fn new(kind: VerifierErrorKind, offset: usize) -> Self {
        Self {
            kind,
            offset: Some(offset),
        }
    }

    pub fn without_offset(kind: VerifierErrorKind) -> Self {
        Self { kind, offset: None }
    }
}
```

**Why this pattern:**
- Centralized error handling with clear context
- Detailed error messages for debugging
- Type-safe error propagation with `Result<T, E>`
- Integration with standard Rust error handling

### Struct Field Documentation

Struct fields should be documented with their purpose:

From `crates/x3-kernel/src/lib.rs`:

```rust
#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct Comit<AccountId, Balance> {
    /// Globally unique Comit identifier.
    pub comit_id: H256,
    /// Origin account that submitted the Comit.
    pub origin: AccountId,
    /// Payload destined for the EVM execution environment.
    pub evm_payload: Vec<u8>,
    /// Payload destined for the SVM execution environment.
    pub svm_payload: Vec<u8>,
    /// Sequential nonce scoped to the origin account.
    pub nonce: u64,
    /// Fee charged for processing the Comit.
    pub fee: Balance,
    /// Dual-VM prepare phase commitment root.
    pub prepare_root: H256,
}
```

### Code Style & Formatting

**Allowed Clippy Lints**

Modules suppress specific clippy warnings when justified:

From `crates/x3-common/src/lib.rs`:

```rust
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
```

From `node/src/lib.rs`:

```rust
#![warn(missing_docs, rust_2018_idioms)]
#![allow(
    clippy::result_large_err,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
```

**Why this pattern:**
- Suppresses false positives on domain-specific idioms
- Explicitly documents where Clippy is overridden and why

**WASM Build Flags**

From `.cargo/config.toml`:

```toml
[target.wasm32-unknown-unknown]
# Disable reference types which parity-wasm can't parse
# MVP feature set ensures broad compatibility with WASM parsers
rustflags = [
    "-C", "target-cpu=mvp",
    "-C", "target-feature=-sign-ext,-reference-types,-bulk-memory",
]
```

### Common Imports & Traits

**Substrate/FRAME Imports**

From `pallets/x3-kernel/src/lib.rs`:

```rust
use frame_support::pallet_prelude::*;
use frame_support::sp_runtime::traits::{AtLeast32BitUnsigned, CheckedAdd, SaturatedConversion};
use frame_support::traits::{Currency, UnixTime};
use frame_system::pallet_prelude::*;
```

**Serialization Derives**

Ubiquitous across types:

```rust
#[derive(Clone, Debug, PartialEq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
```

### Comments & Documentation

**Inline Comments** - Used sparingly for "why" not "what":

```rust
// Defer global logger initialization to the CLI/runner (some runtime
// components initialize logging themselves). We avoid calling
// `env_logger`/`tracing_subscriber` here to prevent double-initialization
// which causes the node to fail at startup.
```

**Doc Comments** - For public APIs:

```rust
/// Create a dummy span for testing purposes.
pub const fn dummy() -> Self {
    Self { start: 0, end: 0 }
}

/// Compute prepare_root using the pallet's canonical algorithm (L-3: Avoid duplication).
/// This delegates to the pallet's public `compute_prepare_root` function to ensure
/// tests use the same algorithm as production code.
fn compute_prepare_root(
    comit_id: H256,
    evm_payload: &[u8],
    svm_payload: &[u8],
    nonce: u64,
    fee: Balance,
) -> H256
```

### Logging & Debugging

From `node/src/logging.rs`:

```rust
use env_logger::Env;

/// Initialize a colorful logger with emojis and a light startup banner.
pub fn init() {
    let _env = Env::default().filter_or("RUST_LOG", "info");
    
    // Colorful startup banner with ASCII art (ANSI color)
    println!("\n\x1b[1;35m");
    println!("       ________          __                ");
    println!("🚀  X3 Chain Node — syncing the mesh ⚡️\x1b[0m\n");
}
```

**Why this pattern:**
- Uses environment variable `RUST_LOG` for configuration
- Provides visual feedback at startup
- Defers logging initialization to avoid double-init conflicts

### Configuration & Constants Management

**Configuration Structures**

From `crates/x3-sidecar/src/config.rs`:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VmConfig {
    /// Maximum gas per execution
    pub max_gas: u64,
    /// Memory limit in bytes
    pub memory_limit: usize,
    /// Stack size limit
    pub stack_limit: usize,
    /// Enable JIT compilation
    pub jit_enabled: bool,
    /// JIT threshold (calls before JIT)
    pub jit_threshold: u32,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            max_gas: 10_000_000,
            memory_limit: 64 * 1024 * 1024, // 64 MB
```

### Common Design Patterns

**Result Wrapper Types**

Every error module defines a canonical result:

```rust
// From crates/x3-compiler/src/error.rs
pub type CompilerResult<T> = Result<T, CompilerError>;

// From crates/gpu-swarm/src/error.rs
pub type SwarmResult<T> = Result<T, SwarmError>;
```

**Builder/Factory Pattern**

From `node/src/service.rs` and test fixtures:

```rust
pub struct ExtBuilder { /* config fields */ }

impl ExtBuilder {
    pub fn build() -> sp_io::TestExternalities {
        /* build test runtime */
    }
}

// Usage:
let mut ext = ExtBuilder::default().build();
ext.execute_with(|| { /* test code */ });
```

**Adapter Pattern** (for pluggable VMs)

From `crates/x3-kernel/src/adapters.rs`:

```rust
pub trait EvmExecutorAdapter {
    type Error;
    fn execute(&self, payload: &[u8]) -> Result<Vec<u8>, Self::Error>;
}

pub struct FrontierEvmAdapter { /* ... */ }
pub struct MockEvmAdapter { /* ... */ }
```

---

## TypeScript/JavaScript Conventions

### Test File Naming

From `tests/`:

```
L1_CONSENSUS_AND_ATOMICITY.test.ts
L1_ISOLATION_AND_ATTACKS.test.ts
L1_LOAD_AND_FORMAL.test.ts
```

**Pattern:** `{Layer}_{TestTopic}.test.ts`

### Test Structure (Vitest/Jest)

From `tests/L1_CONSENSUS_AND_ATOMICITY.test.ts`:

```typescript
import { describe, it, expect, beforeEach, vi } from 'vitest';

describe('Consensus Protocol Testing', () => {
  let state: ConsensusState;

  beforeEach(() => {
    state = {
      canonicalChain: [],
      forkDetected: false,
      finalized: new Set(),
      validators: new Map([/* ... */]),
    };
  });

  describe('Invariant: Single Canonical Chain', () => {
    it('should reject competing chains at same height', () => {
      // Test implementation
      expect(true).toBe(true);
    });
  });
});
```

**Pattern:**
- Use `describe` for test suites
- Organized by invariant/property
- `beforeEach` for setup
- `expect().toBe()` for assertions

### Configuration

From `package.json`:

```json
{
  "devDependencies": {
    "@types/jest": "^30.0.0",
    "cypress": "^15.12.0",
    "jest": "^29.7.0",
    "vitest": "^4.0.18",
    "playwright": "^1.58.2"
  }
}
```

**Pattern:** Projects use both Jest (legacy) and Vitest (new) with Cypress for E2E.

---

## Python Conventions

### Function Definitions

From `tests/evm_integration_test.py`:

```python
def rpc_call(method, params=[]):
    """Make an RPC call to the node."""
    payload = {
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1
    }
```

**Pattern:**
- Docstrings immediately after function definition
- Triple-quoted docstrings with description

### Class Definitions

From `cross-chain-gpu-validator/src/cross_chain_gpu_validator/orchestrator/orchestrator.py`:

```python
@dataclass(frozen=True)
class MultiChainSwapPayload:
    """Generic multi-chain atomic swap payload."""

    swap_id: str
    chain_transactions: dict[str, list[ChainTransaction]]
    timeout_seconds: int
```

**Pattern:**
- Use `@dataclass` decorator
- Include docstring
- Type hints on all fields

### Error Handling

From `tests/evm_integration_test.py`:

```python
if result.returncode != 0:
    raise Exception(f"RPC call failed: {result.stderr}")

response = json.loads(result.stdout)

if "error" in response:
    raise Exception(f"RPC error: {response['error']}")
```

**Pattern:**
- Check return codes and error responses
- Raise exceptions with descriptive messages
- Use f-strings for formatting

---

## Solidity Conventions

### Contract Documentation

From `contracts/core/contracts/X3AtomicExecutor.sol`:

```solidity
// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.20;

/**
 * @title X3AtomicExecutor
 * @notice External chain bridge contract for X3 Atomic Execution verification.
 *
 * This contract allows external EVM chains (Ethereum, Polygon, Arbitrum, etc.)
 * to verify X3 PoAE (Proof of Atomic Execution) proofs and settle cross-chain
 * atomic bundles.
 *
 * ## Verification Flow
 *
 * 1. A relayer submits a PoAE proof from X3 chain.
 * 2. This contract verifies the proof structure.
 * 3. If valid, the bundle is marked as verified and side-effects can execute.
 *
 * ## Production Notes
 *
 * - v0: Structural verification only (hash checks, non-zero fields).
 * - v1: Add GRANDPA/Flash Finality light client verification.
 * - v2: Add ZK proof verification for full trustless settlement.
 */
contract X3AtomicExecutor {
```

**Pattern:**
- SPDX license declaration
- High-level `@title`, `@notice` tags
- Markdown sections (Flow, Notes, etc.)
- Version history and design notes

### Struct Documentation

```solidity
/// @notice On-chain record for a verified bundle.
struct BundleRecord {
    BundleStatus status;
    bytes32      receiptRoot;
    bytes32      finalityCert;
    uint64       finalizedBlock;
    uint32       legCount;
    uint256      verifiedAt;     // Block timestamp when verified
    address      relayer;        // Who submitted the proof
}
```

### Event Documentation

```solidity
event BundleVerified(
    bytes32 indexed bundleId,
    bytes32 receiptRoot,
    bytes32 finalityCert,
    uint64  finalizedBlock,
    uint32  legCount,
    address relayer
);
```

**Pattern:**
- Use `indexed` for important filter fields
- Include all relevant context in event data

---

## Cross-Language Patterns

### Invariant-Driven Testing

Across all languages, tests are organized around invariants:

**Rust (from `pallets/x3-kernel/src/tests.rs`):**
```rust
#[test]
fn submit_comit_successful_flow() {
    new_test_ext().execute_with(|| {
        // Test implementation
        assert_eq!(Nonces::<Test>::get(ALICE), 1);
    });
}
```

**TypeScript (from `tests/L1_CONSENSUS_AND_ATOMICITY.test.ts`):**
```typescript
describe('Invariant: Single Canonical Chain', () => {
  it('should reject competing chains at same height', () => {
    expect(true).toBe(true);
  });
});
```

**Pattern:** Tests verify that invariants hold after operations.

### Logging

- **Rust:** `env_logger`, `tracing` crate
- **TypeScript:** `console.log()`, structured logging libraries
- **Python:** `logging` module
- **Configuration:** Environment variables (`RUST_LOG`, etc.)

### Type Safety

All languages leverage **static/static typing**:
- **Rust:** Strong type system, compile-time guarantees
- **TypeScript:** Type annotations on interfaces and functions
- **Python:** Type hints (Python 3.6+)
- **Solidity:** Explicit types for storage safety

---

## Architectural Patterns

### Separation of Concerns

Each module/crate has a single responsibility:

- `x3-kernel` - Cross-VM orchestration
- `x3-lexer` - Tokenization
- `x3-parser` - Parsing (input to AST)
- `x3-typeck` - Type checking
- `x3-vm` - Runtime execution

### Feature Flags

Rust uses Cargo features to conditionally compile:

```toml
[features]
default = ["std"]
std = ["serde/std"]
runtime-benchmarks = ["frame-benchmarking"]
```

Files use `#[cfg(feature = "...")]` to gate code.

### Security-First Documentation

Many modules include security headers:

```rust
//! ## Security Design Decisions
//!
//! ### H-1: prepare_root Verification
//! ### H-5: VM Adapter Production Status
```

**Why this pattern:**
- Makes security assumptions explicit
- Serves as audit trail for design decisions

---

## Summary

The X3 Chain codebase demonstrates:

1. **Consistency:** Similar patterns across multiple languages
2. **Documentation:** Extensive module and API-level docs
3. **Type Safety:** Static typing prioritized where possible
4. **Error Handling:** Structured, centralized error types
5. **Testing:** Multi-level (unit, integration, E2E) with clear organization
6. **Security Focus:** Explicit documentation of design rationale and constraints
