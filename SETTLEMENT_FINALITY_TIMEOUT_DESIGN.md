# Settlement Finality Timeout Implementation Guide

## Issue #5: Settlement Finality Timeout Missing

**Priority:** 🟡 MEDIUM  
**Category:** Settlement Correctness  
**Severity:** Medium — Attestation stalls could lock settlement proofs indefinitely  

---

## Problem Statement

The X3 settlement engine integrates validator attestations for settlement finality confirmation. Currently:

1. ❌ **No timeout mechanism** — If validator attestations stall, settlement proof waits forever
2. ❌ **No fallback logic** — No dispute resolution if consensus cannot be reached
3. ❌ **No monitoring** — Stalled proofs are not detected or reported
4. ❌ **No recovery** — Once stuck, proof requires manual governance intervention

This creates potential for:
- Bridge transaction deadlock
- Validator inactivity (sybil attack by withholding signatures)
- Cascading settlement failures (one stalled proof blocks others)

---

## Solution Architecture

### 1. Configuration Layer

```rust
// File: pallets/x3-settlement-engine/src/lib.rs

parameter_types! {
    /// Settlement finality timeout (blocks)
    /// At 200ms/block, 300 blocks = 60 seconds
    /// Allows sufficient time for consensus but prevents indefinite hangs
    pub const SettlementFinalityTimeoutBlocks: u32 = 300;
    
    /// Attestation quorum required (as numerator)
    /// e.g., 2/3: quorum_numerator=2, quorum_denominator=3
    pub const QuorumNumerator: u32 = 2;
    pub const QuorumDenominator: u32 = 3;
    
    /// Action on timeout
    /// false = initiate dispute / true = auto-reject and return funds
    pub const AutoRejectOnTimeout: bool = false;
}

pub trait Config: frame_system::Config {
    type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    
    /// Timeout configuration
    type SettlementFinalityTimeoutBlocks: Get<u32>;
    type QuorumNumerator: Get<u32>;
    type QuorumDenominator: Get<u32>;
    type AutoRejectOnTimeout: Get<bool>;
}
```

### 2. Storage Data Structures

```rust
/// Pending settlement proof with attestation tracking
#[derive(Debug, Clone, Encode, Decode, Default)]
pub struct PendingProof<AccountId> {
    pub proof_hash: H256,
    pub proof_data: Vec<u8>,
    pub submitted_block: BlockNumber,
    pub attestations: BTreeMap<AccountId, bool>,  // validator => attested
    pub status: ProofStatus,
}

#[derive(Debug, Clone, Copy, Encode, Decode, PartialEq, Eq)]
pub enum ProofStatus {
    Pending,              // Waiting for attestations
    Confirmed,            // Reached quorum
    Disputed,             // Under dispute review
    Timeout,              // Timeout reached
    Rejected,             // Governance rejected
    Finalized,            // Confirmed and finalized
}

#[pallet::storage]
pub type PendingProofs<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    H256,
    PendingProof<T::AccountId>,
    ValueQuery,
>;

#[pallet::storage]
pub type ProofAttestation<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    H256,           // proof_hash
    Blake2_128Concat,
    T::AccountId,   // validator
    bool,           // attested
    ValueQuery,
>;

#[pallet::storage]
pub type ProofTimeoutLog<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    H256,
    BlockNumber,    // block when timeout was detected
    OptionQuery,
>;
```

### 3. Timeout Check in `on_idle()`

```rust
pub fn check_proof_timeouts(now: BlockNumber) -> Weight {
    let timeout_blocks = T::SettlementFinalityTimeoutBlocks::get();
    let mut weight = Weight::zero();
    
    // Iterate over all pending proofs
    for (proof_hash, mut proof) in PendingProofs::<T>::iter() {
        if proof.status != ProofStatus::Pending {
            continue;  // Skip already finalized/disputed proofs
        }
        
        let age = now.saturating_sub(proof.submitted_block);
        
        // Check if proof has timed out
        if age > timeout_blocks {
            // Timeout reached
            log::warn!(
                "🔴 Settlement proof {:?} timed out after {} blocks (limit: {})",
                proof_hash,
                age,
                timeout_blocks
            );
            
            // Record timeout
            ProofTimeoutLog::<T>::insert(&proof_hash, now);
            weight.saturating_accrue(T::DbWeight::get().write);
            
            // Check current attestation count
            let attestation_count: u32 = ProofAttestation::<T>::iter_prefix(&proof_hash)
                .filter(|(_, attested)| *attested)
                .count() as u32;
            
            let quorum_needed = Self::calculate_quorum_count();
            
            // Emit event with timeout details
            Self::deposit_event(Event::ProofTimeout {
                proof_hash,
                submitted_block: proof.submitted_block,
                timeout_block: now,
                attestations_received: attestation_count,
                attestations_required: quorum_needed,
            });
            weight.saturating_accrue(T::DbWeight::get().read);
            
            // Handle timeout action
            if T::AutoRejectOnTimeout::get() {
                // Auto-reject: return funds to original sender
                proof.status = ProofStatus::Rejected;
                log::warn!(
                    \"⚠️  Auto-rejecting settlement proof {:?} due to timeout\",\n                    proof_hash\n                );
                Self::deposit_event(Event::ProofAutoRejected { proof_hash, block: now });
            } else {
                // Dispute resolution: move to governance review
                proof.status = ProofStatus::Disputed;
                log::info!(
                    \"📋 Moving settlement proof {:?} to dispute resolution\",
                    proof_hash
                );
                Self::deposit_event(Event::ProofDisputeInitiated { proof_hash, block: now });
            }
            
            // Update proof status
            PendingProofs::<T>::insert(&proof_hash, proof);
            weight.saturating_accrue(T::DbWeight::get().write);
        }
    }
    
    weight
}

/// Calculate minimum attestations required for quorum
pub fn calculate_quorum_count() -> u32 {
    let total_validators = pallet_session::Validators::<T>::get().len() as u32;
    let numerator = T::QuorumNumerator::get();
    let denominator = T::QuorumDenominator::get();
    
    // Calculate: total_validators * numerator / denominator (rounding up)
    ((total_validators * numerator) + (denominator - 1)) / denominator
}
```

### 4. Settlement Submission with Timeout Tracking

```rust
/// Submit a settlement proof with timeout tracking
#[pallet::call]
impl<T: Config> Pallet<T> {
    #[pallet::weight(T::DbWeight::get().read_writes(3, 3))]
    pub fn submit_settlement_proof(
        origin: OriginFor<T>,
        proof_hash: H256,
        proof_data: Vec<u8>,
    ) -> DispatchResult {
        let submitter = ensure_signed(origin)?;
        
        // Verify proof is not duplicate
        ensure!(!PendingProofs::<T>::contains_key(&proof_hash), Error::<T>::ProofAlreadyExists);
        
        // Create pending proof record
        let pending_proof = PendingProof {
            proof_hash,
            proof_data,
            submitted_block: frame_system::Pallet::<T>::block_number(),
            attestations: BTreeMap::new(),
            status: ProofStatus::Pending,
        };
        
        PendingProofs::<T>::insert(&proof_hash, pending_proof);
        
        Self::deposit_event(Event::SettlementProofSubmitted {
            proof_hash,
            submitted_by: submitter,
            block: frame_system::Pallet::<T>::block_number(),
        });
        
        Ok(())
    }
    
    /// Validator attests to a settlement proof
    #[pallet::weight(T::DbWeight::get().read_writes(2, 2))]
    pub fn attest_proof(
        origin: OriginFor<T>,
        proof_hash: H256,
    ) -> DispatchResult {
        let validator = ensure_signed(origin)?;
        
        // Verify validator is in active set
        ensure!(
            pallet_session::Validators::<T>::get().contains(&validator),
            Error::<T>::NotValidator
        );
        
        // Verify proof exists and is pending
        let mut proof = PendingProofs::<T>::get(&proof_hash);
        ensure!(proof.status == ProofStatus::Pending, Error::<T>::ProofNotPending);
        
        // Record attestation
        ProofAttestation::<T>::insert(&proof_hash, &validator, true);
        proof.attestations.insert(validator.clone(), true);
        
        // Check if quorum reached
        let attestation_count = proof.attestations.values().filter(|a| **a).count() as u32;
        let quorum_needed = Self::calculate_quorum_count();
        
        Self::deposit_event(Event::AttestationRecorded {
            proof_hash,
            validator,
            attestation_count,
            attestation_required: quorum_needed,
        });
        
        if attestation_count >= quorum_needed {
            // Quorum reached!
            proof.status = ProofStatus::Confirmed;
            PendingProofs::<T>::insert(&proof_hash, proof);
            
            Self::deposit_event(Event::ProofConfirmed {
                proof_hash,
                block: frame_system::Pallet::<T>::block_number(),
            });
        } else {
            PendingProofs::<T>::insert(&proof_hash, proof);
        }
        
        Ok(())
    }
}
```

### 5. Event Logging

```rust
#[pallet::event]
pub enum Event<T: Config> {
    /// Settlement proof submitted
    SettlementProofSubmitted {
        proof_hash: H256,
        submitted_by: T::AccountId,
        block: BlockNumber,
    },
    
    /// Validator attested to proof
    AttestationRecorded {
        proof_hash: H256,
        validator: T::AccountId,
        attestation_count: u32,
        attestation_required: u32,
    },
    
    /// Proof reached quorum and is confirmed
    ProofConfirmed {
        proof_hash: H256,
        block: BlockNumber,
    },
    
    /// Settlement proof timed out (attestations stalled)
    ProofTimeout {
        proof_hash: H256,
        submitted_block: BlockNumber,
        timeout_block: BlockNumber,
        attestations_received: u32,
        attestations_required: u32,
    },
    
    /// Proof moved to dispute resolution (timeout, not auto-reject)
    ProofDisputeInitiated {
        proof_hash: H256,
        block: BlockNumber,
    },
    
    /// Proof auto-rejected due to timeout
    ProofAutoRejected {
        proof_hash: H256,
        block: BlockNumber,
    },
}

#[pallet::error]
pub enum Error<T> {
    ProofAlreadyExists,
    ProofNotPending,
    NotValidator,
}
```

### 6. Integration with Finality Oracle

```rust
/// Notify finality oracle when proof is confirmed
pub fn notify_finality_oracle(proof_hash: H256) {
    // Integration with x3_finality_oracle
    // This informs the finality oracle that a proof has reached consensus
    
    if let Ok(oracle) = <T as pallet_x3_settlement_engine::Config>::FinalityOracle::get() {
        let _ = oracle.mark_proof_final(proof_hash);
    }
}

/// Check if proof is finalized for settlement execution
pub fn is_proof_finalized(proof_hash: H256) -> bool {
    if let Some(proof) = PendingProofs::<T>::get(&proof_hash) {
        proof.status == ProofStatus::Finalized || proof.status == ProofStatus::Confirmed
    } else {
        false
    }
}
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_quorum_calculation() {
        // 10 validators, 2/3 quorum = 7 needed
        let quorum = calculate_quorum_count();
        assert_eq!(quorum, 7);
    }
    
    #[test]
    fn test_proof_timeout_detection() {
        // Submit proof at block 100
        // Check timeout at block 400 (300 block timeout)
        // Expect: proof marked as Timeout
    }
    
    #[test]
    fn test_attestation_quorum_reached() {
        // Submit proof
        // Have 7/10 validators attest
        // Expect: proof marked as Confirmed
    }
    
    #[test]
    fn test_attestation_insufficient_after_timeout() {
        // Submit proof
        // Wait for timeout
        // Have only 5/10 validators attested
        // Expect: proof moved to Disputed, not auto-rejected
    }
    
    #[test]
    fn test_auto_reject_on_timeout() {
        // Set AutoRejectOnTimeout = true
        // Submit proof, wait for timeout
        // Expect: proof marked as Rejected, funds returned
    }
}
```

### Integration Tests

```bash
# Test settlement timeout with 3 validators
cargo test -p pallet-x3-settlement-engine --test settlement_timeout -- --nocapture

# Test under high load
cargo test -p x3-chain-runtime --test settlement_under_load -- --nocapture

# Benchmark finality time
cargo bench --bench settlement_finality -- --nocapture
```

---

## Deployment Configuration

### Mainnet Defaults
```rust
SettlementFinalityTimeoutBlocks = 300,   // 60 seconds
QuorumNumerator = 2,                     // 2/3
QuorumDenominator = 3,
AutoRejectOnTimeout = false,             // Dispute resolution
```

### Testnet Configuration
```rust
SettlementFinalityTimeoutBlocks = 50,    // 10 seconds (faster testing)
QuorumNumerator = 1,                     // 1/3 (faster consensus)
QuorumDenominator = 3,
AutoRejectOnTimeout = true,              // Auto-reject for testing
```

---

## Monitoring & Alerting

### Metrics to Expose
- `settlement_proofs_pending_total` — Number of pending proofs
- `settlement_proofs_timeout_total` — Total timeouts (counter)
- `settlement_proof_age_seconds` — Age of pending proofs (gauge)
- `settlement_attestation_success_rate` — % of proofs reaching quorum

### Alerts
- 🚨 **Critical:** Pending proofs > 100 (settlement backlog)
- ⚠️ **Warning:** Attestation success rate < 70% (validator inactivity)
- ℹ️ **Info:** Proof reached timeout (investigation needed)

---

## Related Issues & Dependencies

- ✅ Issue #1: GPU Sidecar (independent)
- ✅ Issue #2: CrossChainStateRootApi (independent)
- ✅ Issue #3: Pallet Ordering (independent)
- ✅ Issue #4: EVM Precompiles (independent)
- ❌ Issue #5: **This file** (Settlement Timeout)
- ⏳ Issue #6: AgentMemory Indexing (independent)
- ✅ Issue #7: TX Pool Sizing (independent)

---

**Next Action:** Implement in `pallets/x3-settlement-engine/src/lib.rs` following this architecture.

**Estimated Effort:** 4-6 hours (implementation + testing)

**Target Deadline:** Before testnet launch
