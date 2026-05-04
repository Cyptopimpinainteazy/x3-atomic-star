# X3 Settlement Engine - Implementation Reference Guide

Quick reference for implementing missing features. Each section maps to a specific file and line numbers.

---

## 1. BTC SPV Proof Verification Implementation Guide

### File: `pallets/x3-settlement-engine/src/btc_gateway.rs`

#### Function to Implement: `verify_btc_merkle_proof()`

**Location**: Line 1302  
**Current Code**: Returns `Ok(false)`

```rust
fn verify_btc_merkle_proof(tx_id: &[u8; 32], merkle_path: &[[u8; 32]], leaf_index: u32) -> bool {
    // TODO: Implement actual merkle tree verification
    false
}
```

**Implementation Pseudocode**:
```
1. Start with tx_id (leaf hash)
2. For each hash in merkle_path:
   - If leaf_index is even: hash = sha256d(leaf || hash)
   - If leaf_index is odd: hash = sha256d(hash || leaf)
   - leaf_index >>= 1
3. Compare final hash with merkle root
4. Return true if matches
```

**Key Considerations**:
- Bitcoin uses SHA256D (double SHA256)
- Merkle paths are stored bottom-up (leaf to root)
- Leaf index determines left/right positioning
- Must handle odd number of nodes in merkle tree (duplicate parent)

**Test Case**: Use Bitcoin testnet block 2000000 as test vector

**Related Structures**:
```rust
pub struct BtcProof {
    pub tx_id: [u8; 32],
    pub merkle_path: Vec<[u8; 32]>,  // Line 178
    pub block_height: u32,
    pub leaf_index: u32,
}
```

**Helper Functions Available**:
- `sha256d()` - Already available in crate
- `BtcProof` parsing at line 1275

---

#### Function to Implement: `verify_btc_pow_target()`

**Location**: Line 1308  
**Current Code**: Returns `Ok(false)`

```rust
fn verify_btc_pow_target(block_hash: &[u8; 32], n_bits: u32) -> bool {
    // TODO: Implement PoW target verification
    false
}
```

**Implementation Pseudocode**:
```
1. Decode nBits to target value:
   - exponent = nBits >> 24
   - mantissa = nBits & 0xFFFFFF
   - target = mantissa * 2^(8 * (exponent - 3))
2. Convert block_hash to integer (little-endian)
3. Return block_hash_int < target
```

**Bitcoin nBits Format**:
- First byte (bits 24-31): Exponent
- Last 3 bytes (bits 0-23): Mantissa
- Target = Mantissa * 256^(Exponent - 3)

**Test Vectors**: Bitcoin difficulty adjustment blocks

**Related Code**:
```rust
pub block_header: BtcHeader {  // Line 800
    // 80-byte header includes nBits at bytes 72-75
}
```

---

## 2. EVM Receipt MPT Proof Verification Implementation Guide

### File: `pallets/x3-settlement-engine/src/escrow.rs`

#### Function to Implement: `verify_evm_receipt_proof()`

**Location**: Line 280  
**Current Code**: Only checks structure

```rust
fn verify_evm_receipt_proof(
    receipt_rlp: &[u8],
    proof: &[Vec<u8>],
    block_receipt_root: &[u8; 32],
    tx_index: u32,
) -> bool {
    // Currently only validates RLP structure
    // TODO: Implement full MPT verification
    false
}
```

**Implementation Strategy** (3 parts):

**Part 1: RLP Decoding**
- Decode receipt_rlp into (status, cumGasUsed, logs, contractAddress)
- Validate receipt structure

**Part 2: Receipt Hash Calculation**
- RLP encode the receipt
- SHA3-256 hash to get receipt hash

**Part 3: MPT Path Verification**
- Build merkle proof path from tx_index
- Traverse proof nodes
- Validate each node type (branch, leaf, extension)
- Confirm receipt hash matches at leaf

**Key Ethereum MPT Details**:
- Uses Keccak256 (not SHA256)
- Nodes can be: Branch (17 children), Leaf (key+value), Extension (key+child)
- Key path encoded as hex-prefix
- Proof provided as RLP-encoded nodes

**Test Vectors**: Ethereum testnet (Goerli/Sepolia) receipts

**Related Structures**:
```rust
pub struct EvmProof {
    pub receipt_rlp: Vec<u8>,      // Line 120
    pub proof: Vec<Vec<u8>>,       // MPT proof nodes
    pub tx_index: u32,
    pub block_number: u32,
}
```

**Recommended Library**: Consider using `ethereum-merkle-patricia-tree` crate

---

## 3. Solana Transaction Proof Verification Implementation Guide

### File: `pallets/x3-settlement-engine/src/escrow.rs`

#### Function to Implement: `verify_solana_tx_proof()`

**Location**: Line 315  
**Current Code**: Only checks structure

```rust
fn verify_solana_tx_proof(
    transaction: &[u8],
    signatures: &[Vec<u8>],
    block_hash: &[u8; 32],
) -> bool {
    // Currently only validates structure
    // TODO: Implement signature verification
    false
}
```

**Implementation (2 parts)**:

**Part 1: Signature Verification**
- Extract signer public keys from transaction
- Verify Ed25519 signatures using public keys
- All required signers must sign

**Part 2: Block Inclusion (Optional)**
- Verify transaction merkle inclusion in block
- Or use finality oracle to trust block_hash

**Key Solana Details**:
- Uses Ed25519 signature scheme
- Transaction format: 2-byte signature count + signatures + message
- Message format: header + static accounts + recent blockhash + instructions
- Multiple signatures required based on transaction type

**Test Vectors**: Solana testnet transactions

**Related Structures**:
```rust
pub struct SolanaProof {
    pub transaction: Vec<u8>,      // Line 130
    pub signatures: Vec<Vec<u8>>,
    pub block_hash: [u8; 32],
    pub slot: u64,
}
```

**Recommended Library**: `ed25519-dalek` for signature verification

---

## 4. Integration Test Implementation Guide

### File: `pallets/x3-settlement-engine/src/tests.rs`

#### Test Helper Functions to Create

**Location**: After line 438 (end of current tests)

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    // Helper 1: Create a basic settlement intent
    fn create_test_settlement(
        sender: AccountId,
        receiver: AccountId,
        amount_out: Balance,
    ) -> IntentId {
        // Implementation: Call create_intent extrinsic
        // Returns intent_id for use in subsequent tests
    }

    // Helper 2: Lock both legs of settlement
    fn lock_settlement_legs(intent_id: IntentId, bond: Balance) -> Result<(), DispatchError> {
        // Implementation: Call lock_escrow for sender and receiver legs
        // Requires bond for operator
    }

    // Helper 3: Submit proofs for both legs
    fn submit_settlement_proofs(
        intent_id: IntentId,
        sender_proof: Proof,
        receiver_proof: Proof,
    ) -> Result<(), DispatchError> {
        // Implementation: Call submit_proof for each leg
    }

    // Helper 4: Claim settlement
    fn claim_settlement(
        intent_id: IntentId,
        secret: [u8; 32],
    ) -> Result<(), DispatchError> {
        // Implementation: Call claim_settlement with secret reveal
    }

    // Helper 5: Finalize settlement
    fn finalize_settlement(intent_id: IntentId) -> Result<(), DispatchError> {
        // Implementation: Call finalize extrinsic
    }

    #[test]
    fn test_evm_to_x3_settlement() {
        // Flow: EVM sender -> X3 receiver
        // 1. Create intent
        // 2. Lock escrow for both legs
        // 3. Submit EVM receipt proof
        // 4. Claim with secret
        // 5. Verify finalization
    }

    #[test]
    fn test_btc_to_evm_settlement() {
        // Flow: BTC sender -> EVM receiver
        // 1. Create intent with BTC leg
        // 2. Lock escrow
        // 3. Submit BTC SPV proof
        // 4. Submit EVM receipt proof
        // 5. Claim and verify
    }

    #[test]
    fn test_timeout_refund() {
        // Flow: Create intent, lock, timeout, refund
        // 1. Create intent
        // 2. Lock escrow
        // 3. Advance block height past timeout
        // 4. Call refund_settlement()
        // 5. Verify assets returned
        // 6. Verify events
    }
}
```

**Test Utilities in Mock**:
```rust
// From mock.rs (already available):
pub struct MockBlock {
    pub number: BlockNumber,
    pub timestamp: Moment,
}

// Add time advancement:
fn advance_blocks(count: u32) {
    for _ in 0..count {
        System::set_block_number(System::block_number() + 1);
    }
}
```

---

## 5. Reentrancy Detection Implementation Guide

### File: `pallets/x3-settlement-engine/src/invariants.rs`

#### Function to Complete: `check_reentrancy()`

**Location**: Line 1338  
**Current Code**: Marked TODO

```rust
fn check_reentrancy(intent: &SettlementIntent) -> Result<(), Error> {
    // TODO: Check for cross-VM reentrancy
    // Currently incomplete
    Ok(())
}
```

**Implementation**:
```rust
fn check_reentrancy(intent: &SettlementIntent) -> Result<(), Error> {
    // Extract all VMs from legs
    let mut seen_vms = Vec::new();
    for leg in &intent.legs {
        let vm = leg.chain_type();
        
        // Check if VM appears twice (sender and receiver on same chain)
        if seen_vms.contains(&vm) {
            return Err(Error::ReentrancyDetected);
        }
        seen_vms.push(vm);
    }

    // Check for circular settlement chains
    // (e.g., A->B->A->B ...)
    check_circular_dependencies(intent)?;

    // Check for escrow deadlock
    check_deadlock_conditions(intent)?;

    Ok(())
}

fn check_circular_dependencies(intent: &SettlementIntent) -> Result<(), Error> {
    // Build graph of settlements
    // Use DFS to detect cycles
    // Return Err if cycle found
    Ok(())
}

fn check_deadlock_conditions(intent: &SettlementIntent) -> Result<(), Error> {
    // Check for situations where:
    // - Settlement A requires release from B
    // - Settlement B requires release from A
    // - But neither can proceed without the other
    Ok(())
}
```

---

## 6. BTC Release Confirmation Check Implementation Guide

### File: `pallets/x3-settlement-engine/src/invariants.rs`

#### Function to Complete: `check_btc_release_confirmation()`

**Location**: Line 1341  
**Current Code**: Marked TODO

```rust
fn check_btc_release_confirmation(intent: &SettlementIntent) -> Result<(), Error> {
    // TODO: Ensure BTC is not released without X3 confirmation
    Ok(())
}
```

**Implementation**:
```rust
fn check_btc_release_confirmation(intent: &SettlementIntent) -> Result<(), Error> {
    // Check if intent contains BTC leg
    let has_btc_leg = intent.legs.iter().any(|leg| leg.is_btc());

    if has_btc_leg {
        // Verify X3 received finalization signal
        // Check: Has X3_FINALIZED been called for this intent?
        // Check: All non-BTC legs are CLAIMED/FINALIZED?

        let intent_state = IntentStates::<T>::get(intent.id)
            .ok_or(Error::IntentNotFound)?;

        // BTC can only be released if:
        // 1. X3 has confirmed finalization
        // 2. All other legs are claimed
        // 3. Timeout has not occurred

        match intent_state {
            SettlementState::FINALIZED => Ok(()), // OK - X3 confirmed
            SettlementState::REFUNDED => Ok(()),  // OK - all returned
            _ => Err(Error::BtcReleaseWithoutConfirmation),
        }
    } else {
        Ok(())
    }
}
```

---

## 7. Bond Slashing Execution Implementation Guide

### File: `pallets/x3-settlement-engine/src/collateral.rs`

#### Function to Enhance: `slash_bond()`

**Location**: Line 100  
**Current Code**: Marks bond as slashed but doesn't execute transfer

```rust
pub fn slash_bond(bond_id: BondId, amount: Balance) -> Result<(), Error> {
    // Currently: Updates slashed_amount but doesn't transfer funds
    // TODO: Actually transfer slashed funds
    Ok(())
}
```

**Implementation Options**:

**Option A: Direct Execution** (Simplest)
```rust
pub fn slash_bond(bond_id: BondId, amount: Balance) -> Result<(), Error> {
    let bond = Bonds::<T>::get(bond_id).ok_or(Error::BondNotFound)?;

    // 1. Ensure amount doesn't exceed bond
    ensure!(amount <= bond.amount, Error::InsufficientBond);

    // 2. Deduct from operator's balance
    T::Currency::slash(&bond.operator, amount);

    // 3. Update bond record
    let remaining = bond.amount - amount;
    if remaining == 0 {
        Bonds::<T>::remove(bond_id);
    } else {
        bond.amount = remaining;
        Bonds::<T>::insert(bond_id, bond);
    }

    // 4. Emit event
    Self::deposit_event(Event::BondSlashed {
        bond_id,
        amount,
        operator: bond.operator,
    });

    Ok(())
}
```

**Option B: Off-Chain Worker** (For deferred processing)
- Store pending slashes in PendingSlashes storage map
- Off-chain worker processes batch of slashes
- Reduces block weight impact

**Option C: Scheduled Task**
- Use pallet-scheduler to process slashes
- Distribute load across blocks

**Related Storage**:
```rust
pub Bond {
    operator: AccountId,
    amount: Balance,
    slashed_amount: Balance,  // Line 50
}
```

---

## 8. Adaptor Signature Verification Implementation Guide

### File: `pallets/x3-settlement-engine/src/btc_gateway.rs`

#### Function to Implement: `verify_adaptor_signature()`

**Location**: Line 248  
**Current Code**: Deferred

```rust
fn verify_adaptor_signature(
    message: &[u8; 32],
    adaptor_signature: &AdaptorSignature,
    public_key: &[u8; 33],
) -> bool {
    // TODO: Implement ECDSA adaptor signature verification
    false
}
```

**Implementation**:
```rust
fn verify_adaptor_signature(
    message: &[u8; 32],
    adaptor_sig: &AdaptorSignature,
    public_key: &[u8; 33],
) -> bool {
    // Adaptor signature structure:
    // - (r, s) components of ECDSA signature
    // - Encrypted by adaptor
    // - Decryptable only with correct secret

    use k256::ecdsa::{Signature, VerifyingKey};

    // 1. Convert public key bytes to verifying key
    let vkey = match VerifyingKey::from_bytes(public_key) {
        Ok(k) => k,
        Err(_) => return false,
    };

    // 2. Create signature from components
    let sig = match Signature::from_bytes(&adaptor_sig.to_bytes()) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // 3. Verify signature
    vkey.verify(message, &sig).is_ok()
}
```

**Related Structures**:
```rust
pub struct AdaptorSignature {
    pub r: [u8; 32],      // Line 210
    pub s: [u8; 32],
}
```

**Recommended Library**: `k256` for secp256k1 ECDSA

---

## 9. Quick Code Locations Reference

### Core Files
- **lib.rs (1,444 LOC)**: Main extrinsics, storage, events
  - Line 280-350: create_intent() extrinsic
  - Line 450-550: lock_escrow() extrinsic
  - Line 600-700: refund_settlement() extrinsic
  - Line 800-900: claim_settlement() extrinsic

- **types.rs (380 LOC)**: Core types
  - Line 50-100: Intent struct
  - Line 120-160: Proof types (BTC, EVM, Solana)
  - Line 180-220: Events

- **escrow.rs (362 LOC)**: Cross-VM escrow
  - Line 100-150: Escrow state management
  - Line 200-280: Proof verification functions
  - Line 300-350: Release/refund logic

- **btc_gateway.rs (355 LOC)**: BTC operations
  - Line 800-850: BTC block header handling
  - Line 1200-1350: SPV proof verification
  - Line 1400-1450: BTC HTLC operations

- **invariants.rs (405 LOC)**: Safety checks
  - Line 200-300: Invariant enforcement
  - Line 1300-1400: Incomplete checks (TODOs)

- **tests.rs (438 LOC)**: Existing tests
  - Line 1-200: Bond operation tests
  - Line 200-400: Atomic lock tests
  - Line 400-438: Utility functions

### Key Constants
```rust
// From types.rs
const BTC_FINALITY_BLOCKS: u32 = 32;           // Line 10
const EVM_FINALITY_BLOCKS: u32 = 64;           // Line 11
const SETTLEMENT_TIMEOUT_MIN: u32 = 100;       // Line 12
const SETTLEMENT_TIMEOUT_MAX: u32 = 100_000;   // Line 13
const MAX_LEGS_PER_INTENT: usize = 10;         // Line 14
```

---

## 10. Testing Strategy

### Test File Structure
```
tests.rs
├── Unit Tests (already exist)
│   ├── Bond operations
│   ├── Atomic lock state transitions
│   └── Intent validation
├── Integration Tests (to add)
│   ├── Settlement flows (one test per VM combo)
│   ├── Timeout scenarios
│   ├── Proof verification
│   └── Cross-VM combinations
└── Edge Case Tests (to add)
    ├── Replay prevention
    ├── Timing edge cases
    ├── Boundary conditions
    └── Error scenarios
```

### Running Tests Locally
```bash
# Run all tests
cargo test -p pallet-x3-settlement-engine

# Run specific test
cargo test -p pallet-x3-settlement-engine test_evm_to_x3_settlement

# Run with output
cargo test -p pallet-x3-settlement-engine -- --nocapture
```

---

## 11. Common Pitfalls to Avoid

### Bitcoin SPV
❌ Forgetting SHA256D (double hash)  
❌ Leaf index tracking in merkle path traversal  
❌ nBits exponent off-by-one errors  
✅ Use test vectors from known blocks

### EVM MPT
❌ Confusing Keccak256 with SHA256  
❌ Not handling different node types correctly  
❌ Incorrect key path encoding  
✅ Use existing libraries when possible

### Solana
❌ Forgetting Ed25519 requires 64-byte signatures  
❌ Not validating all required signers  
❌ Incorrect transaction format parsing  
✅ Follow Solana documentation exactly

### Testing
❌ Forgetting to check storage state after operations  
❌ Not verifying events are emitted  
❌ Not testing error paths  
✅ Use assert! and assert_eq! liberally

---

*Generated: 2026-04-11 | Implementation reference for x3-settlement-engine | Status: READY FOR DEVELOPMENT*
