# Phase 1.3: Adapter Integration Layer - Packet Deserialization & Routing

**Phase:** 1.3 (Adapter Integration)  
**Status:** 🟢 STARTING  
**Duration:** 5-6 hours  
**Target Tests:** 8+ integration tests  

---

## Overview

Phase 1.3 bridges the packet schema (Phase 1.2) with kernel execution. It adds deserialization, validation, and domain routing logic at the kernel's adapter boundary.

**Key Components:**
1. Packet deserialization module (deserialize raw Vec<u8> → typed Packets)
2. Domain router (route packets to correct VM based on domain_mask)
3. Payload validator (CRC32, size, expiry checks)
4. Integration with x3-kernel pallet (inject deserializer into submit_comit flow)

---

## Architecture

```
submit_comit(evm_payload, svm_payload)
         ↓
[Raw Vec<u8>]
         ↓
Deserializer (packet_adapters.rs)
         ↓
[Typed Packet enum]
         ↓
Router (domain_mask matching)
         ↓
├─→ EVM packets  → EvmExecutorAdapter
├─→ SVM packets  → SvmExecutorAdapter
└─→ X3VM packets → X3ExecutorAdapter
         ↓
[ExecutionReceipt]
```

---

## Implementation Plan

### Step 1: Add x3-packet-schema dependency to x3-kernel

**File:** `pallets/x3-kernel/Cargo.toml`

```toml
[dependencies]
x3-packet-schema = { path = "../../crates/x3-packet-schema" }
```

### Step 2: Create packet_adapters.rs module

**File:** `pallets/x3-kernel/src/packet_adapters.rs` (NEW)

**Responsibilities:**
- Deserialize raw payload bytes into typed Packet enums
- Validate packet structure (header, size, checksum)
- Route packets by domain_mask
- Handle partial/failed deserialization gracefully

**Key Functions:**
```rust
pub fn deserialize_packet(payload: &[u8]) -> Result<Packet, DispatchError>
pub fn validate_packet(packet: &Packet) -> Result<(), DispatchError>
pub fn route_to_domain(packet: &Packet) -> DomainRoute
```

### Step 3: Integrate with kernel's submit_comit

**File:** `pallets/x3-kernel/src/lib.rs`

Inject deserialization into the comit submission flow:
1. After payload received, deserialize to typed Packet
2. Validate packet structure
3. Route to appropriate executor based on domain_mask
4. Proceed with existing kernel logic

### Step 4: Add integration tests

**File:** `pallets/x3-kernel/src/tests.rs` (additions)

Test coverage:
- Deserialize EVM Call → executes on EVM adapter
- Deserialize SVM Invoke → executes on SVM adapter
- Deserialize X3VM AtomicCross → routes to both
- Invalid payload handling
- Checksum validation failures
- Size limit enforcement

---

## File Structure After Phase 1.3

```
pallets/x3-kernel/
├── src/
│   ├── lib.rs (modified: integrate deserializer)
│   ├── adapters.rs (existing: executor traits)
│   ├── packet_adapters.rs (NEW: deserialization & routing)
│   ├── tests.rs (modified: add 8+ new tests)
│   └── ...
└── Cargo.toml (modified: add x3-packet-schema dep)
```

---

## Success Criteria

- ✅ x3-packet-schema imported in x3-kernel
- ✅ Deserializer functions compile without errors
- ✅ Router correctly maps domain_mask → executor
- ✅ Validator checks CRC32 checksums
- ✅ All 8+ integration tests pass
- ✅ submit_comit flow enhanced with typed deserialization
- ✅ git commit created with clean history

---

## Testing Strategy

**Unit Tests** (in packet_adapters.rs):
- Deserialize valid EVM packet
- Deserialize valid SVM packet
- Deserialize valid X3VM packet
- Reject malformed packets
- Validate CRC32 checksums
- Enforce max size limits

**Integration Tests** (in tests.rs):
- submit_comit with typed EVM packet
- submit_comit with typed SVM packet
- Verify route logic (domain_mask matching)

---

## Timeline

- Step 1: Cargo.toml update (5 min)
- Step 2: packet_adapters.rs creation (2-3 hours)
- Step 3: lib.rs integration (1-2 hours)
- Step 4: Tests & validation (1-2 hours)
- Total: 5-6 hours

---

## Next Phase

**Phase 1.4 - Router Pallet:** Implements the pallet-x3-cross-vm-router with:
- Pending queue for cross-VM operations
- Batch execution orchestration
- Settlement logic
