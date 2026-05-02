# S0-005 ATOMIC ROLLBACK DESIGN SPECIFICATION

**Blocker ID:** S0-005 - atomic_rollback_missing  
**Severity:** S0 (Catastrophic)  
**Phase:** 2/6 - Design Rollback Mechanism  
**Status:** IN PROGRESS  
**Date:** 2026-04-26

---

## EXECUTIVE SUMMARY

This document specifies the complete design for implementing atomic rollback mechanisms in X3's cross-VM transaction execution. The design addresses the critical vulnerability where failed atomic operations leave partial state changes across EVM, SVM, and X3VM domains.

**Root Cause:** Missing storage transaction boundaries (`with_storage_layer`) in cross-VM operations  
**Solution:** Two-phase commit pattern with comprehensive rollback guarantees  
**Impact:** Prevents fund loss scenarios where EVM leg succeeds but SVM leg fails

---

## 1. STORAGE TRANSACTION BOUNDARIES

### 1.1 Overview

All cross-VM operations must be wrapped in Substrate's `with_storage_layer` to ensure atomic commits or complete rollbacks. Any error in the transaction scope triggers automatic reversion of ALL storage modifications.

### 1.2 Pattern Specification

**Core Pattern:**
```rust
pub fn cross_vm_operation(...) -> DispatchResult {
    with_storage_layer(|| {
        // All storage operations inside this closure
        // Buffered until Ok(()) is returned
        // Any Err(...) triggers complete rollback
        
        operation_1()?;  // If fails, nothing persists
        operation_2()?;  // If fails, operation_1 reverts
        operation_3()?;  // If fails, both previous revert
        
        Ok(())  // Only commit if ALL operations succeed
    })
}
```

### 1.3 Required Wrapper Locations

#### **File: pallets/x3-cross-vm-router/src/lib.rs**

**Function 1: `do_initiate_transfer` (lines ~450-550)**

**Current Vulnerable Code:**
```rust
fn do_initiate_transfer(
    message_id: H256,
    record: TransferRecord<T>,
) -> DispatchResult {
    let msg = &record.message;
    let asset_id = msg.asset_id;
    let source = msg.source_domain;
    let amount = msg.amount;

    // ❌ NO TRANSACTION BOUNDARY - partial state possible
    T::Ledger::debit_source_to_pending(&asset_id, source, amount)?;
    let record = Self::advance(record, TransferStatus::SourceDebited)?;
    Transfers::<T>::insert(message_id, record);
    UsedMessages::<T>::insert(message_id, ());
    
    Self::deposit_event(Event::TransferInitiated { message_id });
    Ok(())
}
```

**Required Fix:**
```rust
fn do_initiate_transfer(
    message_id: H256,
    record: TransferRecord<T>,
) -> DispatchResult {
    // ✅ WRAP ENTIRE OPERATION IN STORAGE TRANSACTION
    with_storage_layer(|| {
        let msg = &record.message;
        let asset_id = msg.asset_id;
        let source = msg.source_domain;
        let amount = msg.amount;

        // All operations now buffered - commit only if ALL succeed
        T::Ledger::debit_source_to_pending(&asset_id, source, amount)?;
        let record = Self::advance(record, TransferStatus::SourceDebited)?;
        Transfers::<T>::insert(message_id, record);
        UsedMessages::<T>::insert(message_id, ());
        
        Self::deposit_event(Event::TransferInitiated { message_id });
        Ok(())
    })
}
```

**Function 2: `do_complete_transfer` (lines ~550-600)**

**Current Vulnerable Code:**
```rust
fn do_complete_transfer(message_id: H256) -> DispatchResult {
    let record = Transfers::<T>::get(message_id)
        .ok_or(Error::<T>::UnknownMessage)?;
    
    ensure!(
        record.status == TransferStatus::SourceDebited,
        Error::<T>::WrongStatus
    );

    let msg = &record.message;
    
    // ❌ NO ATOMIC GUARANTEES - EVM success + SVM failure = partial state
    T::Ledger::credit_destination_from_pending(
        &msg.asset_id,
        msg.destination_domain,
        msg.amount
    )?;
    
    let r1 = Self::advance(record, TransferStatus::DestinationCredited)?;
    let r2 = Self::advance(r1, TransferStatus::Finalized)?;
    Transfers::<T>::insert(message_id, r2);
    
    Self::deposit_event(Event::TransferCompleted { message_id });
    Ok(())
}
```

**Required Fix:**
```rust
fn do_complete_transfer(message_id: H256) -> DispatchResult {
    // ✅ WRAP ENTIRE COMPLETION IN STORAGE TRANSACTION
    with_storage_layer(|| {
        let record = Transfers::<T>::get(message_id)
            .ok_or(Error::<T>::UnknownMessage)?;
        
        ensure!(
            record.status == TransferStatus::SourceDebited,
            Error::<T>::WrongStatus
        );

        let msg = &record.message;
        
        // All VM operations now atomic - any failure rolls back everything
        T::Ledger::credit_destination_from_pending(
            &msg.asset_id,
            msg.destination_domain,
            msg.amount
        )?;
        
        let r1 = Self::advance(record, TransferStatus::DestinationCredited)?;
        let r2 = Self::advance(r1, TransferStatus::Finalized)?;
        Transfers::<T>::insert(message_id, r2);
        
        Self::deposit_event(Event::TransferCompleted { message_id });
        Ok(())
    })
}
```

**Function 3: `cancel_expired_xvm_transfer` (if exists)**

**Required Pattern:**
```rust
pub fn cancel_expired_xvm_transfer(
    origin: OriginFor<T>,
    message_id: H256,
) -> DispatchResult {
    // ✅ WRAP CANCELLATION IN STORAGE TRANSACTION
    with_storage_layer(|| {
        let record = Transfers::<T>::get(message_id)
            .ok_or(Error::<T>::UnknownMessage)?;
        
        // Verify expiration + proper status
        ensure!(
            record.status == TransferStatus::SourceDebited,
            Error::<T>::CannotCancelAfterCompletion
        );
        
        let current_block = <frame_system::Pallet<T>>::block_number();
        ensure!(
            current_block > record.deadline,
            Error::<T>::TransferNotExpired
        );
        
        // Refund source from pending (atomic with status change)
        T::Ledger::refund_source_from_pending(
            &record.message.asset_id,
            record.message.source_domain,
            record.message.amount
        )?;
        
        let cancelled = Self::advance(record, TransferStatus::Cancelled)?;
        Transfers::<T>::insert(message_id, cancelled);
        
        Self::deposit_event(Event::TransferCancelled { message_id });
        Ok(())
    })
}
```

#### **File: pallets/x3-atomic-kernel/src/lib.rs**

**Function 4: `rollback_atomic_bundle` (lines ~682-750)**

**Current Vulnerable Code:**
```rust
pub fn rollback_atomic_bundle(
    origin: OriginFor<T>,
    bundle_id: H256,
    reason: Vec<u8>,
) -> DispatchResult {
    ensure_signed(origin)?;
    
    let mut record = Bundles::<T>::get(bundle_id)
        .ok_or(Error::<T>::UnknownBundle)?;
    
    ensure!(
        record.status == BundleStatus::Pending,
        Error::<T>::CannotRollbackFinalized
    );
    
    // ❌ ONLY UPDATES STATUS - DOES NOT REVERT VM STATE CHANGES
    record.status = BundleStatus::RolledBack;
    record.completion_block = Some(<frame_system::Pallet<T>>::block_number());
    
    let slash = record.bond / 2u32.into();  // Slash 50% of bond
    T::Currency::slash(&record.submitter, slash)?;
    
    Bundles::<T>::insert(bundle_id, record);
    
    Self::deposit_event(Event::BundleRolledBack { bundle_id, reason });
    Ok(())
}
```

**Required Fix:**
```rust
pub fn rollback_atomic_bundle(
    origin: OriginFor<T>,
    bundle_id: H256,
    reason: Vec<u8>,
) -> DispatchResult {
    ensure_signed(origin)?;
    
    // ✅ WRAP ENTIRE ROLLBACK IN STORAGE TRANSACTION
    with_storage_layer(|| {
        let mut record = Bundles::<T>::get(bundle_id)
            .ok_or(Error::<T>::UnknownBundle)?;
        
        ensure!(
            record.status == BundleStatus::Pending,
            Error::<T>::CannotRollbackFinalized
        );
        
        // ✅ ACTUALLY REVERT VM STATE CHANGES
        // For each leg in the bundle, call the corresponding VM's rollback
        for (leg_index, leg) in record.legs.iter().enumerate() {
            match leg.vm_type {
                VmType::Evm => {
                    // Revert EVM state using cross-VM router
                    T::CrossVmRouter::revert_vm_leg(
                        VmType::Evm,
                        &leg.payload,
                        &leg.prepare_root,
                    ).map_err(|_| Error::<T>::EvmRollbackFailed)?;
                },
                VmType::Svm => {
                    // Revert SVM state using cross-VM router
                    T::CrossVmRouter::revert_vm_leg(
                        VmType::Svm,
                        &leg.payload,
                        &leg.prepare_root,
                    ).map_err(|_| Error::<T>::SvmRollbackFailed)?;
                },
                VmType::X3Native => {
                    // Revert X3VM state using cross-VM router
                    T::CrossVmRouter::revert_vm_leg(
                        VmType::X3Native,
                        &leg.payload,
                        &leg.prepare_root,
                    ).map_err(|_| Error::<T>::X3VmRollbackFailed)?;
                },
            }
        }
        
        // Update status and slash bond (atomic with state reversion)
        record.status = BundleStatus::RolledBack;
        record.completion_block = Some(<frame_system::Pallet<T>>::block_number());
        
        let slash = record.bond / 2u32.into();
        T::Currency::slash(&record.submitter, slash)?;
        
        Bundles::<T>::insert(bundle_id, record);
        
        Self::deposit_event(Event::BundleRolledBack { bundle_id, reason });
        Ok(())
    })
}
```

**Function 5: `finalize_atomic_bundle` (lines ~567-650)**

**Required Pattern:**
```rust
pub fn finalize_atomic_bundle(
    origin: OriginFor<T>,
    bundle_id: H256,
    receipts: Vec<ExecutionReceipt>,
    finality_cert: FinalityCertificate,
) -> DispatchResult {
    ensure_signed(origin)?;
    
    // ✅ WRAP FINALIZATION IN STORAGE TRANSACTION
    with_storage_layer(|| {
        let mut record = Bundles::<T>::get(bundle_id)
            .ok_or(Error::<T>::UnknownBundle)?;
        
        ensure!(
            record.status == BundleStatus::Pending,
            Error::<T>::BundleNotPending
        );
        
        // Verify finality certificate (must be on-chain anchored)
        ensure!(
            Self::verify_finality_certificate(&finality_cert),
            Error::<T>::InvalidFinalityCertificate
        );
        
        // Verify all receipts match prepare_root commitments
        for (leg, receipt) in record.legs.iter().zip(receipts.iter()) {
            ensure!(
                Self::verify_receipt_against_prepare_root(leg, receipt),
                Error::<T>::ReceiptMismatch
            );
        }
        
        // Apply all state changes atomically
        for (leg, receipt) in record.legs.iter().zip(receipts.iter()) {
            T::CrossVmRouter::apply_vm_state_diff(
                leg.vm_type,
                &receipt.state_diff,
            )?;
        }
        
        // Update status and unreserve bond
        record.status = BundleStatus::Finalized;
        record.completion_block = Some(<frame_system::Pallet<T>>::block_number());
        record.finality_cert = Some(finality_cert);
        
        T::Currency::unreserve(&record.submitter, record.bond);
        
        Bundles::<T>::insert(bundle_id, record);
        
        Self::deposit_event(Event::BundleFinalized { bundle_id });
        Ok(())
    })
}
```

### 1.4 New Required Trait Methods

The `CrossVmRouter` trait needs new methods for state reversion:

**File: pallets/x3-cross-vm-router/src/lib.rs (or traits.rs)**

```rust
pub trait CrossVmRouter<AssetId, Balance, Domain> {
    /// Existing methods...
    
    /// Revert VM state changes for a specific leg
    /// MUST be implemented with storage transactions
    fn revert_vm_leg(
        vm_type: VmType,
        payload: &[u8],
        prepare_root: &H256,
    ) -> Result<(), DispatchError>;
    
    /// Apply VM state diff from execution receipt
    /// MUST be implemented with storage transactions
    fn apply_vm_state_diff(
        vm_type: VmType,
        state_diff: &StateDiff,
    ) -> Result<(), DispatchError>;
    
    /// Verify VM state consistency after operation
    fn verify_vm_state_consistency(
        vm_type: VmType,
        expected_root: &H256,
    ) -> Result<bool, DispatchError>;
}
```

### 1.5 Transaction Boundary Rules

**Rule 1: One Transaction Per User-Facing Operation**
- Each extrinsic (user-callable function) gets exactly ONE `with_storage_layer` wrapper
- Internal helper functions do NOT have their own wrappers (nested transactions not needed)

**Rule 2: Error Propagation with `?` Operator**
- All fallible operations inside transaction use `?` to propagate errors
- Any error automatically triggers rollback of entire transaction

**Rule 3: No Partial Success**
- Either ALL operations succeed and commit, or ALL fail and rollback
- No intermediate states possible

**Rule 4: Event Emission Inside Transaction**
- Events deposited inside transaction only emitted if transaction commits
- Failed transactions do not emit events

---

## 2. ROLLBACK TRIGGER CONDITIONS

### 2.1 VM Execution Failures

**Trigger:** Any VM leg returns error during execution

**EVM Failures:**
- Out of gas
- Revert opcode executed
- Invalid bytecode
- Stack overflow/underflow
- Invalid memory access
- Call depth exceeded

**SVM Failures:**
- Transaction simulation failed
- Account data verification failed
- Insufficient rent
- Program execution error
- Cross-program invocation failed

**X3VM Failures:**
- Invalid opcode
- Type mismatch
- Arithmetic overflow
- Resource limit exceeded
- Capability check failed

**Action:** Immediate rollback of ALL legs (EVM + SVM + X3VM)

### 2.2 Timeout / Deadline Exceeded

**Trigger:** Current block number > bundle.deadline

**Detection Points:**
1. Before execution starts (in `execute_atomic_bundle`)
2. During execution (periodic checks)
3. Before finalization (in `finalize_atomic_bundle`)

**Action:**
- Reject execution if deadline already passed
- Allow cancellation via `cancel_expired_atomic_bundle`
- Automatic rollback if detected during execution

### 2.3 Validation Failures

**Prepare Root Mismatch:**
- Execution receipt's prepare_root ≠ bundle's prepare_root
- Indicates execution didn't follow declared parameters
- **Action:** Reject finalization, trigger rollback

**Finality Certificate Invalid:**
- Certificate not anchored to valid GRANDPA justification
- Certificate signatures don't reach supermajority threshold
- Certificate validator set hash mismatch
- **Action:** Reject finalization, trigger rollback

**State Consistency Violation:**
- Post-execution state doesn't match expected invariants
- Balance sum invariant broken
- Access set violation (touched accounts not in declared set)
- **Action:** Immediate rollback

### 2.4 Resource Exhaustion

**Gas Exhaustion:**
- Any VM leg runs out of gas mid-execution
- **Action:** Rollback all legs

**Storage Quota Exceeded:**
- Operation would exceed storage limits
- **Action:** Rollback before commit

**Memory Limit:**
- VM execution exceeds memory bounds
- **Action:** Rollback all legs

### 2.5 Access Control Violations

**Unauthorized Mint:**
- Minting operation without proper authority proof
- **Action:** Reject and rollback

**Governance Bypass Attempt:**
- Operation requires governance approval but lacks it
- **Action:** Reject and rollback

**Insufficient Permissions:**
- Origin lacks required permissions for operation
- **Action:** Reject and rollback

### 2.6 Balance / Accounting Errors

**Insufficient Balance:**
- Source domain lacks sufficient asset balance
- **Action:** Rollback initiation

**Balance Invariant Violation:**
- Total supply changes unexpectedly
- Source + destination + pending ≠ original total
- **Action:** Panic and rollback (critical invariant)

**Overflow / Underflow:**
- Arithmetic operation would overflow
- **Action:** Rollback transaction

### 2.7 Replay / Nonce Violations

**Message Already Used:**
- `UsedMessages` already contains message_id
- **Action:** Reject with `Error::MessageAlreadyProcessed`

**Nonce Mismatch:**
- Provided nonce ≠ expected nonce for sender
- **Action:** Reject with `Error::InvalidNonce`

---

## 3. ERROR PROPAGATION STRATEGY

### 3.1 Error Flow Architecture

```
User Extrinsic
    └─> with_storage_layer(|| {
            └─> VM Leg 1 (EVM)
                └─> Result<Receipt, Error>  ──┐
                                               │
            └─> VM Leg 2 (SVM)                 │ Any Err(...) here
                └─> Result<Receipt, Error>  ──┤ triggers rollback
                                               │ of ENTIRE transaction
            └─> VM Leg 3 (X3VM)                │
                └─> Result<Receipt, Error>  ──┘
                
            └─> Consistency Checks
                └─> Result<(), Error>
                
            Ok(())  ← Only reached if ALL succeed
        })
```

### 3.2 Cross-VM Error Aggregation

**Sequential Execution Model:**
```rust
pub fn execute_atomic_bundle(bundle_id: H256) -> DispatchResult {
    with_storage_layer(|| {
        let record = Bundles::<T>::get(bundle_id)
            .ok_or(Error::<T>::UnknownBundle)?;
        
        let mut receipts = Vec::new();
        
        // Execute each leg sequentially
        for (leg_index, leg) in record.legs.iter().enumerate() {
            let receipt = match leg.vm_type {
                VmType::Evm => {
                    // If EVM fails, ? propagates error and rolls back
                    T::EvmAdapter::execute(&leg.payload)
                        .map_err(|_| Error::<T>::EvmExecutionFailed)?
                },
                VmType::Svm => {
                    // If SVM fails after EVM succeeded, BOTH roll back
                    T::SvmAdapter::execute(&leg.payload)
                        .map_err(|_| Error::<T>::SvmExecutionFailed)?
                },
                VmType::X3Native => {
                    T::X3VmAdapter::execute(&leg.payload)
                        .map_err(|_| Error::<T>::X3VmExecutionFailed)?
                },
            };
            
            receipts.push(receipt);
        }
        
        // Verify all receipts before committing
        Self::verify_all_receipts(&record, &receipts)?;
        
        // Store receipts for finalization
        BundleReceipts::<T>::insert(bundle_id, receipts);
        
        Ok(())  // Only commits if ALL legs succeeded
    })
}
```

**Key Property:** First failure stops execution AND rolls back all prior successful legs

### 3.3 Error Type Design

**New Error Variants Required:**

```rust
#[pallet::error]
pub enum Error<T> {
    // Existing errors...
    
    /// EVM leg execution failed during atomic bundle
    EvmExecutionFailed,
    
    /// SVM leg execution failed during atomic bundle
    SvmExecutionFailed,
    
    /// X3VM leg execution failed during atomic bundle
    X3VmExecutionFailed,
    
    /// EVM state rollback failed (CRITICAL)
    EvmRollbackFailed,
    
    /// SVM state rollback failed (CRITICAL)
    SvmRollbackFailed,
    
    /// X3VM state rollback failed (CRITICAL)
    X3VmRollbackFailed,
    
    /// State consistency check failed after execution
    StateConsistencyViolation,
    
    /// Balance invariant violated (sum mismatch)
    BalanceInvariantViolation,
    
    /// Prepare root mismatch (execution didn't follow plan)
    PrepareRootMismatch,
    
    /// Finality certificate validation failed
    InvalidFinalityCertificate,
    
    /// Bundle deadline exceeded
    BundleDeadlineExceeded,
    
    /// Receipt verification failed
    ReceiptVerificationFailed,
}
```

### 3.4 Critical Error Handling

**Unrecoverable Errors:**
If a rollback operation itself fails, this indicates critical system corruption:

```rust
pub fn rollback_atomic_bundle(...) -> DispatchResult {
    with_storage_layer(|| {
        // ... rollback logic ...
        
        for leg in record.legs.iter() {
            match T::CrossVmRouter::revert_vm_leg(...) {
                Ok(_) => {}, // Rollback succeeded
                Err(e) => {
                    // CRITICAL: Rollback failed!
                    // Log error and HALT validator
                    log::error!(
                        target: "x3-atomic-kernel",
                        "CRITICAL: VM rollback failed for bundle {:?}, leg {:?}: {:?}",
                        bundle_id, leg.vm_type, e
                    );
                    
                    // Emit critical event
                    Self::deposit_event(Event::CriticalRollbackFailure {
                        bundle_id,
                        vm_type: leg.vm_type,
                    });
                    
                    // Return error (transaction still rolls back)
                    return Err(Error::<T>::EvmRollbackFailed.into());
                }
            }
        }
        
        Ok(())
    })
}
```

**Action on Critical Failure:**
1. Log error with full context
2. Emit `CriticalRollbackFailure` event
3. Return error (outer `with_storage_layer` still rolls back storage)
4. Validator should monitor these events and alert operators
5. Consider halting chain if multiple critical failures occur

---

## 4. ATOMIC OPERATION SCOPE

### 4.1 Three-Phase Workflow

**Phase 1: PREPARE** (Off-Chain / Light On-Chain)
- Reserve fees and bonds
- Lock accounts with deterministic ordering
- Validate all inputs (amounts, addresses, routes)
- Compute prepare_root commitment
- **Storage:** Store bundle record with status `Pending`

**Phase 2: EXECUTE** (Off-Chain with Proof Capture)
- Execute EVM leg with state capture
- Execute SVM leg with state capture
- Execute X3VM leg if present
- Collect all receipts and state diffs
- **Storage:** Store receipts (indexed by bundle_id)

**Phase 3: COMMIT or ROLLBACK** (On-Chain Atomic)
- **COMMIT PATH:**
  1. Verify prepare_root matches execution
  2. Verify finality certificate
  3. Apply all state changes atomically via `with_storage_layer`
  4. Update status to `Finalized`
  5. Unreserve bonds
  6. Release locks
  7. Emit success events
  
- **ROLLBACK PATH:**
  1. Revert ALL VM state changes via `revert_vm_leg`
  2. Update status to `RolledBack`
  3. Slash submitter bond (penalty)
  4. Release locks
  5. Emit rollback events

### 4.2 Phase 1: PREPARE Implementation

```rust
pub fn submit_atomic_bundle(
    origin: OriginFor<T>,
    legs: Vec<BundleLeg>,
    deadline: T::BlockNumber,
) -> DispatchResult {
    let submitter = ensure_signed(origin)?;
    
    // ✅ WRAP PREPARE PHASE IN STORAGE TRANSACTION
    with_storage_layer(|| {
        // Validate bundle structure
        ensure!(
            legs.len() <= T::MaxLegsPerBundle::get(),
            Error::<T>::TooManyLegs
        );
        ensure!(
            deadline > <frame_system::Pallet<T>>::block_number(),
            Error::<T>::DeadlineInPast
        );
        
        // Compute prepare_root (commitment to execution plan)
        let prepare_root = Self::compute_prepare_root(&legs)?;
        
        // Reserve bond (economic security)
        let bond = T::MinBond::get();
        T::Currency::reserve(&submitter, bond)?;
        
        // Lock accounts with deterministic ordering (prevent deadlocks)
        let locked_accounts = Self::lock_accounts_for_bundle(&legs)?;
        
        // Create bundle record
        let bundle_id = Self::generate_bundle_id(&submitter, &legs, deadline);
        let record = BundleRecord {
            submitter: submitter.clone(),
            legs,
            prepare_root,
            deadline,
            bond,
            locked_accounts,
            status: BundleStatus::Pending,
            submission_block: <frame_system::Pallet<T>>::block_number(),
            completion_block: None,
            finality_cert: None,
        };
        
        // Store bundle
        Bundles::<T>::insert(bundle_id, record);
        
        Self::deposit_event(Event::BundleSubmitted {
            bundle_id,
            submitter,
            prepare_root,
            deadline,
        });
        
        Ok(())
    })
}
```

**Prepare Phase Atomicity:**
- Bond reservation + account locking + bundle storage = ONE transaction
- If any step fails, everything rolls back (no partial preparation)

### 4.3 Phase 2: EXECUTE Implementation

**Note:** Execution happens OFF-CHAIN by executor clients, but receipt storage is on-chain:

```rust
/// Called by off-chain executor after executing bundle
pub fn store_execution_receipts(
    origin: OriginFor<T>,
    bundle_id: H256,
    receipts: Vec<ExecutionReceipt>,
) -> DispatchResult {
    ensure_signed(origin)?;
    
    // ✅ WRAP RECEIPT STORAGE IN STORAGE TRANSACTION
    with_storage_layer(|| {
        let record = Bundles::<T>::get(bundle_id)
            .ok_or(Error::<T>::UnknownBundle)?;
        
        ensure!(
            record.status == BundleStatus::Pending,
            Error::<T>::BundleNotPending
        );
        
        ensure!(
            receipts.len() == record.legs.len(),
            Error::<T>::ReceiptCountMismatch
        );
        
        // Store receipts for later finalization
        BundleReceipts::<T>::insert(bundle_id, receipts.clone());
        
        Self::deposit_event(Event::ExecutionReceiptsStored {
            bundle_id,
            receipt_count: receipts.len() as u32,
        });
        
        Ok(())
    })
}
```

### 4.4 Phase 3: COMMIT Implementation

```rust
pub fn finalize_atomic_bundle(
    origin: OriginFor<T>,
    bundle_id: H256,
    finality_cert: FinalityCertificate,
) -> DispatchResult {
    ensure_signed(origin)?;
    
    // ✅ WRAP ENTIRE FINALIZATION IN STORAGE TRANSACTION
    with_storage_layer(|| {
        let mut record = Bundles::<T>::get(bundle_id)
            .ok_or(Error::<T>::UnknownBundle)?;
        
        ensure!(
            record.status == BundleStatus::Pending,
            Error::<T>::BundleNotPending
        );
        
        let receipts = BundleReceipts::<T>::get(bundle_id)
            .ok_or(Error::<T>::ReceiptsNotFound)?;
        
        // === VERIFICATION PHASE (all must pass) ===
        
        // 1. Verify finality certificate (Ed25519 signatures + supermajority)
        T::FinalityVerifier::verify_certificate(&finality_cert)
            .map_err(|_| Error::<T>::InvalidFinalityCertificate)?;
        
        // 2. Verify each receipt against prepare_root
        for (leg, receipt) in record.legs.iter().zip(receipts.iter()) {
            ensure!(
                Self::verify_receipt_against_prepare_root(leg, receipt),
                Error::<T>::PrepareRootMismatch
            );
        }
        
        // 3. Verify state consistency (no invariant violations)
        Self::verify_state_consistency(&record, &receipts)?;
        
        // === APPLICATION PHASE (atomic commit) ===
        
        // Apply all state diffs from all VMs atomically
        for (leg, receipt) in record.legs.iter().zip(receipts.iter()) {
            T::CrossVmRouter::apply_vm_state_diff(
                leg.vm_type,
                &receipt.state_diff,
            )?;
        }
        
        // === CLEANUP PHASE ===
        
        // Update bundle status
        record.status = BundleStatus::Finalized;
        record.completion_block = Some(<frame_system::Pallet<T>>::block_number());
        record.finality_cert = Some(finality_cert);
        
        // Unreserve bond (successful execution)
        T::Currency::unreserve(&record.submitter, record.bond);
        
        // Release locked accounts
        Self::unlock_accounts(&record.locked_accounts)?;
        
        // Update storage
        Bundles::<T>::insert(bundle_id, record);
        
        Self::deposit_event(Event::BundleFinalized { bundle_id });
        
        Ok(())
    })
    // If ANY verification or application step fails, ENTIRE transaction rolls back
    // No partial finalization possible
}
```

### 4.5 Phase 3: ROLLBACK Implementation

See Section 1.3 Function 4 for complete `rollback_atomic_bundle` implementation.

**Key Properties:**
1. Revert ALL VM state changes for ALL legs
2. Update bundle status to `RolledBack`
3. Slash submitter bond (economic penalty)
4. Release locked accounts
5. Everything wrapped in `with_storage_layer` for atomicity

---

## 5. CROSS-VM CONSISTENCY VERIFICATION

### 5.1 Balance Conservation Invariant

**Invariant:** `Total Supply Before = Total Supply After`

```rust
fn verify_balance_invariant(
    asset_id: &AssetId,
    pre_supply: Balance,
    post_supply: Balance,
) -> Result<(), DispatchError> {
    ensure!(
        pre_supply == post_supply,
        Error::<T>::BalanceInvariantViolation
    );
    Ok(())
}
```

**Application:**
- Check before and after cross-VM transfer
- Sum: `source_balance + destination_balance + pending_balance`
- Must equal original total

### 5.2 Transfer Status Consistency

**Invariant:** Status transitions must follow valid state machine

```
Pending → SourceDebited → DestinationCredited → Finalized
                ↓
            Cancelled (only if SourceDebited + expired)
```

**Verification:**
```rust
fn verify_transfer_status_transition(
    current: TransferStatus,
    next: TransferStatus,
) -> Result<(), DispatchError> {
    let valid = match (current, next) {
        (TransferStatus::Pending, TransferStatus::SourceDebited) => true,
        (TransferStatus::SourceDebited, TransferStatus::DestinationCredited) => true,
        (TransferStatus::SourceDebited, TransferStatus::Cancelled) => true,  // If expired
        (TransferStatus::DestinationCredited, TransferStatus::Finalized) => true,
        _ => false,
    };
    
    ensure!(valid, Error::<T>::InvalidStatusTransition);
    Ok(())
}
```

### 5.3 Replay Protection Consistency

**Invariant:** Each message ID processed exactly once

```rust
fn verify_no_replay(message_id: H256) -> Result<(), DispatchError> {
    ensure!(
        !UsedMessages::<T>::contains_key(message_id),
        Error::<T>::MessageAlreadyProcessed
    );
    Ok(())
}
```

**Application:**
- Check BEFORE processing any bridge message
- Mark message as used INSIDE transaction (atomic with processing)

### 5.4 VM State Consistency

**Invariant:** EVM, SVM, and X3VM states match expected roots

```rust
fn verify_vm_state_consistency(
    bundle: &BundleRecord<T>,
    receipts: &[ExecutionReceipt],
) -> Result<(), DispatchError> {
    for (leg, receipt) in bundle.legs.iter().zip(receipts.iter()) {
        // Verify state root after applying diff matches expected
        let actual_root = T::CrossVmRouter::get_vm_state_root(leg.vm_type)?;
        let expected_root = receipt.post_state_root;
        
        ensure!(
            actual_root == expected_root,
            Error::<T>::StateConsistencyViolation
        );
    }
    
    Ok(())
}
```

### 5.5 Account Locking Consistency

**Invariant:** Accounts locked for bundle must be unlocked after completion/rollback

```rust
fn verify_no_dangling_locks() -> Result<(), DispatchError> {
    // This check runs periodically (e.g., in on_initialize)
    let current_block = <frame_system::Pallet<T>>::block_number();
    
    for (bundle_id, record) in Bundles::<T>::iter() {
        if record.status == BundleStatus::Pending 
            && current_block > record.deadline + T::GracePeriod::get() 
        {
            // Found stale bundle with locked accounts
            log::warn!(
                target: "x3-atomic-kernel",
                "Stale bundle {:?} with status Pending past deadline, forcing cleanup",
                bundle_id
            );
            
            // Force rollback to release locks
            Self::force_rollback_stale_bundle(bundle_id)?;
        }
    }
    
    Ok(())
}
```

### 5.6 Comprehensive Consistency Check Function

```rust
/// Master consistency verification - MUST pass before commit
fn verify_all_consistency_invariants(
    bundle: &BundleRecord<T>,
    receipts: &[ExecutionReceipt],
) -> Result<(), DispatchError> {
    // 1. Balance conservation
    for (leg, receipt) in bundle.legs.iter().zip(receipts.iter()) {
        let pre_balance = receipt.pre_state.total_balance;
        let post_balance = receipt.post_state.total_balance;
        Self::verify_balance_invariant(&leg.asset_id, pre_balance, post_balance)?;
    }
    
    // 2. VM state consistency
    Self::verify_vm_state_consistency(bundle, receipts)?;
    
    // 3. Prepare root verification (execution followed plan)
    for (leg, receipt) in bundle.legs.iter().zip(receipts.iter()) {
        ensure!(
            receipt.prepare_root == bundle.prepare_root,
            Error::<T>::PrepareRootMismatch
        );
    }
    
    // 4. No locked account conflicts
    ensure!(
        !Self::has_lock_conflicts(&bundle.locked_accounts),
        Error::<T>::LockConflict
    );
    
    // 5. Receipt signatures valid (if signed by executors)
    for receipt in receipts.iter() {
        if let Some(signature) = &receipt.signature {
            Self::verify_receipt_signature(receipt, signature)?;
        }
    }
    
    Ok(())
}
```

---

## 6. TEST SCENARIOS

### 6.1 Primary Test Case (from SECURITY_BLOCKER_PROGRESS.md)

**Test ID:** S0-005-T01  
**Name:** EVM Success + SVM Failure → Full Rollback  
**Priority:** P0 (CRITICAL)

**Setup:**
1. Create atomic bundle with 2 legs:
   - Leg 1: EVM transfer of 100 tokens from Alice to Bob
   - Leg 2: SVM transfer of 200 tokens from Carol to Dave
2. Configure SVM to fail (e.g., insufficient balance for Carol)

**Execution:**
1. Submit bundle via `submit_atomic_bundle`
2. Execute bundle off-chain
3. EVM leg succeeds (Bob receives 100 tokens)
4. SVM leg fails (Carol has insufficient balance)
5. Attempt finalization via `finalize_atomic_bundle`

**Expected Result:**
- Finalization REJECTS (SVM receipt shows failure)
- Automatic rollback triggered
- **VERIFY:** Bob's balance REVERTED (no longer has 100 tokens)
- **VERIFY:** Alice's balance RESTORED (still has original amount)
- **VERIFY:** Carol and Dave balances UNCHANGED
- **VERIFY:** Bundle status = `RolledBack`
- **VERIFY:** Submitter bond SLASHED

**Assertion:**
```rust
assert_eq!(
    Balances::<T>::get(BOB_ACCOUNT, TOKEN_ID),
    BOB_ORIGINAL_BALANCE  // NOT BOB_ORIGINAL_BALANCE + 100
);
assert_eq!(
    Balances::<T>::get(ALICE_ACCOUNT, TOKEN_ID),
    ALICE_ORIGINAL_BALANCE  // Fully restored
);
```

### 6.2 Test Case 2: SVM Success + EVM Failure

**Test ID:** S0-005-T02  
**Name:** SVM Success + EVM Failure → Full Rollback

**Setup:**
1. Bundle with 2 legs (SVM first, EVM second)
2. Configure EVM to fail (e.g., out of gas)

**Expected Result:**
- SVM changes rolled back even though SVM leg succeeded
- NO partial state (SVM revert happens atomically with EVM failure)

### 6.3 Test Case 3: Three-VM Rollback

**Test ID:** S0-005-T03  
**Name:** X3VM Success + EVM Success + SVM Failure → Full Rollback

**Setup:**
1. Bundle with 3 legs (X3VM, EVM, SVM)
2. First two succeed, third fails

**Expected Result:**
- ALL THREE VMs rolled back
- No partial state across any VM

### 6.4 Test Case 4: Partial State Detection

**Test ID:** S0-005-T04  
**Name:** Detect Partial State Violation

**Setup:**
1. Simulate scenario where rollback mechanism doesn't exist
2. Manually create partial state (EVM success without SVM)

**Expected Result:**
- Consistency check FAILS
- `StateConsistencyViolation` error raised

### 6.5 Test Case 5: Cross-VM Consistency Verification

**Test ID:** S0-005-T05  
**Name:** Cross-VM Balance Sum Invariant

**Setup:**
1. Create bundle that transfers tokens across VMs
2. Verify balance sum before and after

**Expected Result:**
- `verify_balance_invariant` passes
- Total supply conserved across all VMs

### 6.6 Test Case 6: Concurrent Operations

**Test ID:** S0-005-T06  
**Name:** No Deadlocks with Deterministic Locking

**Setup:**
1. Submit two bundles with overlapping account sets
2. Accounts: {Alice, Bob, Carol}
3. Bundle 1 locks: [Alice, Bob]
4. Bundle 2 locks: [Bob, Carol]

**Expected Result:**
- Deterministic lock ordering prevents deadlock
- Both bundles complete successfully (or one waits for other)

### 6.7 Test Case 7: Timeout-Triggered Rollback

**Test ID:** S0-005-T07  
**Name:** Bundle Deadline Exceeded → Automatic Cancellation

**Setup:**
1. Submit bundle with deadline = current_block + 10
2. Wait for deadline to pass without finalization
3. Call `cancel_expired_xvm_transfer`

**Expected Result:**
- Cancellation succeeds
- Source funds refunded from pending
- Bundle status = `Cancelled`

### 6.8 Test Case 8: Invalid State Transition Rollback

**Test ID:** S0-005-T08  
**Name:** Reject Invalid Status Transition

**Setup:**
1. Create transfer with status `DestinationCredited`
2. Attempt to transition directly to `Cancelled` (invalid path)

**Expected Result:**
- Transaction rejected with `InvalidStatusTransition` error
- No state change occurs

### 6.9 Test Case 9: Replay Protection with Rollback

**Test ID:** S0-005-T09  
**Name:** Nonces Remain Valid After Rollback

**Setup:**
1. Submit bundle with nonce N
2. Bundle fails and rolls back
3. Resubmit bundle with same nonce N

**Expected Result:**
- Nonce N still valid (not consumed by failed attempt)
- Second submission succeeds

### 6.10 Test Case 10: Fee Handling on Rollback

**Test ID:** S0-005-T10  
**Name:** Proper Fee Refund on Rollback

**Setup:**
1. Submit bundle with fee reservation
2. Bundle fails during execution
3. Rollback triggered

**Expected Result:**
- Reserved fees REFUNDED to submitter
- Bond partially slashed (penalty)
- No fee leak

### 6.11 Test Case 11: Multi-Leg Bundle Rollback

**Test ID:** S0-005-T11  
**Name:** 5-Leg Bundle with Mid-Failure Rollback

**Setup:**
1. Bundle with 5 legs across all VM types
2. Leg 3 (middle) configured to fail

**Expected Result:**
- Legs 1-2 rolled back (already executed)
- Legs 3-5 never execute
- Complete system consistency

### 6.12 Test Case 12: Edge Cases

**Test ID:** S0-005-T12  
**Name:** Edge Case Coverage

**Scenarios:**
1. **Zero-amount transfer**: Should fail validation, rollback properly
2. **Expired transfer**: Cancellation path works correctly
3. **All VMs fail**: Complete rollback of empty state changes
4. **Single-leg bundle**: Rollback works for trivial case
5. **Maximum legs bundle**: Rollback works at scale limit

---

## 7. IMPLEMENTATION CHECKLIST

### Phase 3: Implementation (Next Phase)
- [ ] Add `with_storage_layer` wrappers to all identified functions
- [ ] Implement `revert_vm_leg` in CrossVmRouter trait
- [ ] Implement `apply_vm_state_diff` in CrossVmRouter trait
- [ ] Implement `verify_vm_state_consistency` checks
- [ ] Add new error variants to Error enum
- [ ] Implement account locking with deterministic ordering
- [ ] Implement force cleanup for stale bundles
- [ ] Add consistency verification functions

### Phase 4: Testing (After Implementation)
- [ ] Write test S0-005-T01: EVM success + SVM failure → rollback
- [ ] Write test S0-005-T02: SVM success + EVM failure → rollback
- [ ] Write test S0-005-T03: Three-VM rollback
- [ ] Write test S0-005-T04: Partial state detection
- [ ] Write test S0-005-T05: Cross-VM consistency
- [ ] Write test S0-005-T06: Concurrent operations (no deadlock)
- [ ] Write test S0-005-T07: Timeout rollback
- [ ] Write test S0-005-T08: Invalid transition rejection
- [ ] Write test S0-005-T09: Replay protection with rollback
- [ ] Write test S0-005-T10: Fee handling on rollback
- [ ] Write test S0-005-T11: Multi-leg rollback
- [ ] Write test S0-005-T12: Edge cases
- [ ] Verify all 12 tests pass with 100% success rate

### Phase 5: Documentation (After Testing)
- [ ] Write comprehensive S0_BLOCKER_5_ATOMIC_ROLLBACK_FIXED.md (600+ lines)
- [ ] Document all design decisions and tradeoffs
- [ ] Include execution flow diagrams
- [ ] Document test results and coverage
- [ ] Create integration notes for other subsystems

### Phase 6: Validation (Final)
- [ ] Build verification: `cargo build -p x3-cross-vm-router`
- [ ] Build verification: `cargo build -p x3-atomic-kernel`
- [ ] Test verification: `cargo test -p x3-cross-vm-router`
- [ ] Test verification: `cargo test -p x3-atomic-kernel`
- [ ] Update SECURITY_BLOCKER_PROGRESS.md (mark S0-005 as RESOLVED)
- [ ] Update THREE_TRACK_VERIFICATION_MASTER_SUMMARY.md (5/9 resolved)
- [ ] Git commit with message: "feat: S0-005 atomic_rollback_missing RESOLVED - Two-phase commit with storage transactions"

---

## 8. DESIGN VALIDATION

### 8.1 Security Properties Achieved

✅ **Atomicity:** All cross-VM operations wrapped in `with_storage_layer`  
✅ **Isolation:** Storage changes buffered until commit  
✅ **Consistency:** Comprehensive invariant checks before commit  
✅ **Durability:** Once committed, changes permanent (unless explicitly rolled back)

### 8.2 Vulnerability Remediation

✅ **Root Cause Addressed:** Storage transaction boundaries added everywhere  
✅ **False Claim Fixed:** Line 28-31 claim now accurate after implementation  
✅ **Partial State Prevented:** Impossible due to transaction atomicity  
✅ **Fund Loss Eliminated:** EVM success + SVM failure = complete rollback

### 8.3 Attack Vectors Closed

✅ **Exploitation Scenario 1:** EVM transfer succeeds, SVM fails → NO LONGER POSSIBLE (full rollback)  
✅ **Exploitation Scenario 2:** Partial finalization → NO LONGER POSSIBLE (all-or-nothing commit)  
✅ **Exploitation Scenario 3:** State divergence → PREVENTED by consistency checks

### 8.4 Compliance with S0-004 Quality Standard

✅ **Research Phase:** Complete (Phase 1)  
✅ **Design Phase:** Complete (Phase 2 - this document)  
🟡 **Implementation Phase:** Pending (Phase 3)  
🟡 **Testing Phase:** Pending (Phase 4 - 12 tests planned)  
🟡 **Documentation Phase:** Pending (Phase 5 - 600+ lines planned)  
🟡 **Validation Phase:** Pending (Phase 6 - build + trackers + commit)

---

## 9. NEXT STEPS

**Immediate Next Action:**
Proceed to **Phase 3: Implement Two-Phase Commit**

**Implementation Order:**
1. Add `with_storage_layer` wrappers (highest priority)
2. Implement new trait methods (`revert_vm_leg`, `apply_vm_state_diff`)
3. Add consistency verification functions
4. Implement account locking logic
5. Add new error variants
6. Update function signatures as needed

**Success Criteria:**
- All code compiles cleanly
- No new warnings introduced
- Ready for comprehensive testing in Phase 4

---

**Document Status:** ✅ COMPLETE  
**Phase:** 2/6 - Design Rollback Mechanism  
**Next Phase:** 3/6 - Implement Two-Phase Commit  
**Approval Required:** User approval to proceed to implementation

