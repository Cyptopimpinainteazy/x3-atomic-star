# X3 Settlement Engine - Actionable Issues Backlog

This document translates the feature inventory into concrete, actionable GitHub issues for implementation.

---

## TIER 1: CRITICAL (Week 1-4)

### Issue T1-1: Implement BTC SPV Merkle Proof Verification
**Severity**: CRITICAL | **Timeline**: Week 1-2 | **Effort**: 3-4 days  
**Blocks**: All BTC settlements

**Description**:
The BTC SPV proof verification currently fails closed (returns false). Implement full merkle tree verification to validate BTC transaction inclusion.

**Acceptance Criteria**:
- [ ] Merkle tree algorithm correctly reconstructs root from transaction + path
- [ ] Validates transaction hash is at leaf of tree
- [ ] Tests with real BTC blocks (mainnet or testnet)
- [ ] Handles odd-length merkle paths correctly
- [ ] Performance acceptable (< 1ms per verification)

**Implementation Details**:
- Location: `pallets/x3-settlement-engine/src/btc_gateway.rs` line 1302
- Current code: `fn verify_btc_merkle_proof()` returns false
- Dependencies: SHA256 hashing (already available)
- Test vectors: Use Bitcoin testnet blocks for validation

**Related Code**:
```
btc_gateway.rs:1302 - verify_btc_merkle_proof()
btc_gateway.rs:1308 - verify_btc_pow_target()
types.rs:180 - BtcProof struct definition
```

**Tasks**:
1. Study Bitcoin merkle tree structure (32-byte hashes, sha256d)
2. Implement merkle path validation algorithm
3. Add test cases with real Bitcoin blocks
4. Benchmark performance
5. Document merkle proof format

---

### Issue T1-2: Implement BTC PoW Target Verification
**Severity**: CRITICAL | **Timeline**: Week 1-2 | **Effort**: 2 days  
**Blocks**: All BTC settlements (dependent on T1-1)

**Description**:
Verify BTC block headers meet required PoW difficulty target. Currently returns false.

**Acceptance Criteria**:
- [ ] Correctly interprets Bitcoin difficulty encoding (nBits format)
- [ ] Validates block hash meets target difficulty
- [ ] Handles difficulty adjustment periods
- [ ] Tests with real BTC blocks across multiple difficulty periods

**Implementation Details**:
- Location: `pallets/x3-settlement-engine/src/btc_gateway.rs` line 1308
- Current code: `fn verify_btc_pow_target()` returns false
- Dependencies: Hash256 generation from block header
- Bitcoin Spec: https://en.bitcoin.it/wiki/Difficulty

**Related Code**:
```
btc_gateway.rs:800 - BtcHeader struct (80 bytes)
btc_gateway.rs:1308 - verify_btc_pow_target()
types.rs:175 - BtcProof with nBits field
```

**Tasks**:
1. Understand Bitcoin nBits encoding
2. Implement difficulty target calculation
3. Implement PoW verification (hash < target)
4. Add test cases across difficulty adjustments
5. Document PoW format and validation rules

---

### Issue T1-3: Implement EVM Receipt MPT Proof Verification
**Severity**: CRITICAL | **Timeline**: Week 2-3 | **Effort**: 3-4 days  
**Blocks**: All EVM settlements

**Description**:
Verify EVM transaction receipt using Merkle Patricia Trie (MPT) proof. Currently only validates structure.

**Acceptance Criteria**:
- [ ] Reconstructs receipt root from RLP-encoded receipt
- [ ] Validates proof path from receipt to block root
- [ ] Correctly interprets MPT node types (branch, leaf, extension)
- [ ] Tests with real Ethereum blocks
- [ ] Handles various receipt types (contract creation, token transfer, etc.)

**Implementation Details**:
- Location: `pallets/x3-settlement-engine/src/escrow.rs` line 280
- Current code: `fn verify_evm_receipt_proof()` only checks structure
- Dependencies: RLP encoding/decoding (may need dependency)
- Ethereum Spec: https://ethereum.org/en/developers/docs/data-structures-and-encoding/patricia-merkle-trie/

**Related Code**:
```
escrow.rs:280 - verify_evm_receipt_proof()
types.rs:120 - EvmProof struct definition
types.rs:85 - EvmLeg with receipt_root
```

**Tasks**:
1. Study Ethereum MPT structure and encoding
2. Implement RLP decoding/encoding (or use library)
3. Implement MPT node validation
4. Implement proof path verification
5. Add test cases with real Ethereum blocks
6. Benchmark performance

**Note**: Consider using existing Rust library (e.g., `ethereum-merkle-patricia-tree`) to reduce complexity.

---

### Issue T1-4: Implement Solana Transaction Proof Verification
**Severity**: CRITICAL | **Timeline**: Week 3 | **Effort**: 2-3 days  
**Blocks**: All Solana settlements

**Description**:
Verify Solana transaction proof by validating transaction inclusion in block and instruction validation.

**Acceptance Criteria**:
- [ ] Validates transaction signature
- [ ] Confirms transaction in block via merkle validation
- [ ] Validates required instructions present
- [ ] Tests with real Solana testnet blocks
- [ ] Handles both legacy and versioned transactions

**Implementation Details**:
- Location: `pallets/x3-settlement-engine/src/escrow.rs` line 315
- Current code: `fn verify_solana_tx_proof()` only checks structure
- Dependencies: Ed25519 signature verification (may need library)
- Solana Spec: https://docs.solana.com/anatomy-of-a-transaction

**Related Code**:
```
escrow.rs:315 - verify_solana_tx_proof()
types.rs:130 - SolanaProof struct definition
types.rs:100 - SolanaLeg with block_hash
```

**Tasks**:
1. Study Solana transaction and block structure
2. Implement Ed25519 signature verification
3. Implement transaction merkle validation
4. Implement instruction validation
5. Add test cases with real Solana blocks
6. Benchmark performance

---

### Issue T1-5: Add Full Settlement Integration Tests
**Severity**: CRITICAL | **Timeline**: Week 3-4 | **Effort**: 2-3 days  
**Blocks**: Can't validate end-to-end settlement (dependent on T1-1, T1-2, T1-3, T1-4)

**Description**:
Write comprehensive integration tests for complete settlement lifecycles across all VM combinations.

**Acceptance Criteria**:
- [ ] Test full settlement flow: create → lock → prove → claim → finalize
- [ ] Test for each VM combination (BTC, EVM, Solana, X3)
- [ ] All flows run successfully without errors
- [ ] Events emitted correctly at each stage
- [ ] Storage states updated correctly

**Implementation Details**:
- Location: `pallets/x3-settlement-engine/src/tests.rs`
- Test framework: Substrate test primitives
- Mock setup: Already exists in `mock.rs`

**Test Matrix** (minimum 12 tests):
```
- Single-leg X3 settlement
- Two-leg EVM ↔ X3 settlement
- Two-leg BTC ↔ X3 settlement
- Two-leg Solana ↔ X3 settlement
- Three-leg EVM ↔ BTC ↔ X3 settlement
- Three-leg EVM ↔ Solana ↔ X3 settlement
- Partial claim (one leg only) - should fail
- Proof verification failure - should fail
- State transitions validation
- Event emission validation
- Storage consistency checks
```

**Tasks**:
1. Create test helper functions (create_settlement, submit_proofs, claim, finalize)
2. Implement test cases for each VM type
3. Implement cross-VM combination tests
4. Verify all events are emitted
5. Verify all storage is updated correctly
6. Add documentation for test patterns

---

### Issue T1-6: Write Timeout Refund Integration Test
**Severity**: CRITICAL | **Timeline**: Day 1 | **Effort**: 1 day  
**Blocks**: Can't verify refund flow works (can start immediately)

**Description**:
Write integration test for timeout refund flow - the happy path for failed settlements.

**Acceptance Criteria**:
- [ ] Create intent and lock escrow
- [ ] Advance time past timeout
- [ ] Call refund_settlement() extrinsic
- [ ] Verify assets are returned to original sender
- [ ] Verify X3Refunded event emitted
- [ ] Verify atomic lock released

**Implementation Details**:
- Location: `pallets/x3-settlement-engine/src/tests.rs`
- Related extrinsics: `refund_settlement()` in lib.rs line 680
- Ready to implement now (doesn't need proof verification)

**Tasks**:
1. Create test helper for time advancement
2. Create intent and lock escrow
3. Advance time past timeout
4. Call refund_settlement()
5. Verify return state and events
6. Test with multiple legs
7. Test with different VMs

---

## TIER 2: HIGH (Week 4-6)

### Issue T2-1: Complete Reentrancy Detection Logic
**Severity**: HIGH | **Timeline**: Week 2 | **Effort**: 1-2 days  
**Blocks**: Production safety

**Description**:
Finish cross-VM reentrancy detection marked as TODO in invariants.rs.

**Acceptance Criteria**:
- [ ] Detect when same VM appears as both sender and receiver
- [ ] Detect cross-chain escrow deadlock scenarios
- [ ] Prevent circular settlement chains
- [ ] Tests for each detection scenario

**Implementation Details**:
- Location: `pallets/x3-settlement-engine/src/invariants.rs` line 1338
- Current code: `check_reentrancy()` incomplete
- Blocked on: Nothing (can implement now)

**Related Code**:
```
invariants.rs:1338 - check_reentrancy() function
intent.rs:200 - Settlement leg planning
```

**Tasks**:
1. Define reentrancy patterns to detect
2. Implement detection algorithm
3. Add return error cases
4. Write tests for each pattern
5. Document detection logic

---

### Issue T2-2: Complete BTC Release Confirmation Check
**Severity**: HIGH | **Timeline**: Week 2 | **Effort**: 1 day  
**Blocks**: BTC invariant enforcement

**Description**:
Implement check that BTC can only be released with X3 confirmation (marked TODO).

**Acceptance Criteria**:
- [ ] Verify X3 confirmation received before BTC release
- [ ] Prevent BTC release without matching X3 finalization
- [ ] Tests for valid and invalid release scenarios

**Implementation Details**:
- Location: `pallets/x3-settlement-engine/src/invariants.rs` line 1341
- Current code: `check_btc_release_confirmation()` incomplete
- Blocked on: Nothing

**Tasks**:
1. Define BTC release confirmation requirements
2. Implement confirmation check
3. Add validation in release flow
4. Write tests for scenarios

---

### Issue T2-3: Implement Actual Bond Slashing Execution
**Severity**: HIGH | **Timeline**: Week 3 | **Effort**: 2 days  
**Blocks**: Operator penalties don't actually happen

**Description**:
Currently, bond slashing is queued but not executed. Implement actual fund slashing via off-chain worker or scheduled extrinsic.

**Acceptance Criteria**:
- [ ] Bond balance actually decreases when slashed
- [ ] Slashed funds are transferred (to treasury or burn)
- [ ] Slashing events properly recorded
- [ ] Tests verify actual fund movement

**Implementation Details**:
- Location: `pallets/x3-settlement-engine/src/collateral.rs` and `lib.rs`
- Current state: Slashing event queued but no execution
- Options:
  1. Off-chain worker to process pending slashes
  2. Scheduled extrinsic to process pending slashes
  3. Direct execution on violation report

**Related Code**:
```
collateral.rs:100 - slash_bond() function
lib.rs:1050 - slash_bond() extrinsic marked "testnet only"
```

**Tasks**:
1. Choose implementation strategy (worker vs scheduled)
2. Implement actual fund transfer logic
3. Update collateral tracking
4. Add tests for actual fund slashing
5. Verify treasury/burn recipients
6. Add configuration for slash destination

---

### Issue T2-4: Implement Adaptor Signature ECDSA Verification
**Severity**: HIGH | **Timeline**: Week 5 | **Effort**: 2 days  
**Blocks**: Cryptographic binding completeness

**Description**:
Currently adaptor signature verification is deferred. Implement ECDSA verification for adaptor signatures.

**Acceptance Criteria**:
- [ ] Verify adaptor signatures with ECDSA
- [ ] Prevent invalid signature acceptance
- [ ] Tests with valid and invalid signatures
- [ ] Performance acceptable

**Implementation Details**:
- Location: `pallets/x3-settlement-engine/src/btc_gateway.rs` line 248
- Current code: `verify_adaptor_signature()` deferred
- Dependencies: ECDSA library (may need to add)

**Related Code**:
```
btc_gateway.rs:248 - verify_adaptor_signature() deferred
types.rs:200 - AdaptorSignature struct
```

**Tasks**:
1. Choose ECDSA library for Substrate
2. Implement signature verification
3. Understand adaptor signature scheme
4. Add test cases with valid/invalid signatures
5. Benchmark performance

---

### Issue T2-5: Comprehensive Cross-VM Settlement Tests
**Severity**: HIGH | **Timeline**: Week 4-5 | **Effort**: 2-3 days  
**Blocks**: Can't validate all VM combinations (dependent on T1-1 through T1-4)

**Description**:
Add extensive tests for all possible cross-VM settlement combinations.

**Acceptance Criteria**:
- [ ] Test all 9 VM pair combinations (BTC↔EVM, BTC↔SVM, EVM↔SVM, etc.)
- [ ] Test multi-leg settlements (3+ VMs)
- [ ] Test with multiple settlement amounts
- [ ] Test with different timeout values
- [ ] All tests pass without errors

**Test Matrix** (comprehensive):
```
Two-leg settlements (9 combinations):
- BTC ↔ EVM
- BTC ↔ SVM
- BTC ↔ X3
- EVM ↔ SVM
- EVM ↔ X3
- SVM ↔ X3
- EVM ↔ BTC (reverse)
- SVM ↔ BTC (reverse)
- X3 ↔ X3 (multi-account)

Three-leg settlements (examples):
- BTC ↔ EVM ↔ SVM
- EVM ↔ SVM ↔ X3
- BTC ↔ SVM ↔ X3
- All three plus X3
```

**Tasks**:
1. Create test harness for multi-VM settlements
2. Implement test cases for each pair
3. Implement test cases for 3-leg settlements
4. Verify atomicity across all combinations
5. Verify event emission for each leg

---

### Issue T2-6: Add Invariant Violation Detection Tests
**Severity**: HIGH | **Timeline**: Week 4 | **Effort**: 1-2 days  
**Blocks**: Can't verify violation detection works (dependent on T2-1, T2-2)

**Description**:
Write tests that trigger each invariant violation and verify it's detected and reported.

**Acceptance Criteria**:
- [ ] Test each of 5 invariants can be violated
- [ ] Violations are properly detected
- [ ] InvariantViolation event is emitted
- [ ] Settlement is blocked/refunded on violation

**Test Cases**:
1. Partial execution attempt (should fail)
2. BTC release without confirmation (should fail)
3. Timeout without user favoring (should slash operator)
4. Reentrancy attempt (should fail)
5. Invalid escrow state (should fail)

**Tasks**:
1. Create violation triggering scenarios
2. Implement detection verification
3. Verify event emission
4. Verify settlement protection
5. Document violation scenarios

---

## TIER 3: MEDIUM (Post-MVP)

### Issue T3-1: Implement on_initialize() Hook
**Severity**: MEDIUM | **Timeline**: Post-MVP | **Effort**: 1-2 days

**Description**:
Process expired settlements in on_initialize() instead of lazy evaluation. Currently marked as TODO.

**Acceptance Criteria**:
- [ ] on_initialize() finds and processes expired intents
- [ ] Weight calculations prevent block overload
- [ ] Storage is cleaned up properly
- [ ] Tests verify processing happens

**Implementation Details**:
- Location: `pallets/x3-settlement-engine/src/lib.rs` line 493
- Concern: Weight impact on every block
- Option: Process in batches with weight limits

**Tasks**:
1. Implement batch processing with weight limit
2. Determine optimal batch size
3. Add weight calculation
4. Benchmark impact on block time
5. Add tests for processing

---

### Issue T3-2: Enhance Rate Limiting
**Severity**: MEDIUM | **Timeline**: Post-MVP | **Effort**: 1 day

**Description**:
Add more sophisticated rate limiting beyond pending intent count.

**Possible Enhancements**:
- Per-leg volume limits
- Time-window based limits
- Sliding window rate limiting
- Adaptive rate limiting based on network congestion

**Current State**: Basic pending intent counter

**Tasks**:
1. Define rate limiting requirements
2. Implement enhanced rate limiting
3. Add configuration parameters
4. Test limit enforcement

---

### Issue T3-3: Expand Collateral Manager
**Severity**: MEDIUM | **Timeline**: Post-MVP | **Effort**: 2-3 days

**Description**:
Expand skeletal collateral manager for more sophisticated bond handling.

**Enhancements**:
- Variable collateral requirements per operator reputation
- Bond tiering based on settlement size
- Collateral recovery on settlement success
- Collateral augmentation for new operators

**Current State**: Basic deposit/withdraw/slash

**Tasks**:
1. Define enhanced collateral requirements
2. Implement reputation tracking
3. Implement dynamic requirements
4. Add configuration options
5. Test all scenarios

---

### Issue T3-4: Implement Settlement Time Tracking
**Severity**: MEDIUM | **Timeline**: Post-MVP | **Effort**: 0.5 days

**Description**:
Currently settlement_time is hardcoded to 0. Calculate actual settlement duration.

**Details**:
- Location: `pallets/x3-settlement-engine/src/lib.rs` line 211
- Calculate: finalize block - create block
- Emit in X3Finalized event

**Tasks**:
1. Store creation block number
2. Calculate duration in finalization
3. Include in event
4. Add to tests

---

## TIER 4: LOW (Future Work)

### Issue T4-1: Light Client State Verification
**Severity**: LOW | **Timeline**: Future | **Effort**: 4-5 days

**Description**:
Implement light client state verification for better proof validation.

**Scope**: Post-MVP optimization

---

### Issue T4-2: Cross-Chain Proof Aggregation
**Severity**: LOW | **Timeline**: Future | **Effort**: 3-4 days

**Description**:
Enable aggregating multiple proofs for efficiency.

**Scope**: Future optimization

---

## Implementation Checklist

Use this checklist to track progress:

### Week 1-2: Proof Verification
- [ ] T1-1: BTC SPV Merkle verification
- [ ] T1-2: BTC PoW target verification
- [ ] T1-6: Timeout refund test (run in parallel)

### Week 2-3: Additional Verification
- [ ] T1-3: EVM MPT proof verification
- [ ] T1-4: Solana transaction verification
- [ ] T2-1: Complete reentrancy detection
- [ ] T2-2: Complete BTC release check

### Week 3-4: Integration Testing
- [ ] T1-5: Full settlement integration tests
- [ ] T2-4: Adaptor signature verification
- [ ] T2-5: Cross-VM settlement tests (heavy)
- [ ] T2-6: Invariant violation tests

### Week 4-6: Safety & Hardening
- [ ] T2-3: Actual bond slashing execution
- [ ] Governance integration for slash_bond()
- [ ] Edge case testing
- [ ] Performance benchmarking

### Week 6+: Post-MVP
- [ ] T3-1: on_initialize() hook
- [ ] T3-2: Enhanced rate limiting
- [ ] T3-3: Expanded collateral manager
- [ ] T3-4: Settlement time tracking

---

*Generated: 2026-04-11 | For: x3-settlement-engine pallet | Status: ACTIONABLE ISSUES READY*
