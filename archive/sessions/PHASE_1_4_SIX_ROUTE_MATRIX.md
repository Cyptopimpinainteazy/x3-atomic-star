# Phase 1.4: Six-Route Matrix Specification

**Status**: Complete specification for Phase 1.4 Router Pallet  
**Objective**: Define all internal cross-VM transfer routes in X3 MVP  
**Scope**: Internal routing only (TrustedInternal proof tier); external routes Phase C

---

## Executive Summary

X3 MVP supports **6 internal transfer routes** connecting three domains:
- **X3Native** (L1 blockchain)
- **X3Evm** (EVM sidechain)
- **X3Svm** (Solana-like sidechain)

All routes are **bidirectional** and **atomic** within X3 block finality.

---

## Route Matrix Overview

```
DOMAIN PAIR TABLE (6 routes):

┌──────────────────┬──────────────────┬────────────────────────┐
│ Source Domain    │ Target Domain    │ Route ID               │
├──────────────────┼──────────────────┼────────────────────────┤
│ X3Native         │ X3Evm            │ Route 1: N→E           │
│ X3Evm            │ X3Native         │ Route 2: E→N           │
│ X3Native         │ X3Svm            │ Route 3: N→S           │
│ X3Svm            │ X3Native         │ Route 4: S→N           │
│ X3Evm            │ X3Svm            │ Route 5: E→S           │
│ X3Svm            │ X3Evm            │ Route 6: S→E           │
└──────────────────┴──────────────────┴────────────────────────┘
```

**Key Property**: Routes form two triangles:
- Triangle 1: X3Native ↔ X3Evm (direct)
- Triangle 2: X3Native ↔ X3Svm (direct)
- Triangle 3: X3Evm ↔ X3Svm (direct)

All routes have **100% coverage** - no missing pairs.

---

## Route Definitions

### Route 1: X3Native → X3Evm (N→E)

**Purpose**: Transfer X3 tokens from blockchain L1 to EVM sidechain

| Property | Value |
|----------|-------|
| Source Domain | X3Native |
| Destination Domain | X3Evm |
| Sender Type | X3Native AccountBytes ([32] bytes) |
| Recipient Type | EVM AccountBytes ([20] bytes) |
| Proof Tier | TrustedInternal |
| Ledger Operation | SourceDebited on X3Native → DestinationCredited on X3Evm |
| Atomicity | X3 block finality |
| Status Flow | Created → SourceDebited → DestinationCredited → Finalized |

**Example Flow**:
```
Alice (X3Native) → 100 X3 → [Router] → Alice (X3Evm)
1. Debit 100 from alice_native() ledger
2. Credit 100 to alice_evm() ledger
3. Emit: TransferInitiated, TransferCompleted
```

**Error Conditions**:
- Insufficient balance on X3Native leg
- EVM address incompatible with recipient field
- Route not active (paused or disabled)
- Replay attack (duplicate message_id)

---

### Route 2: X3Evm → X3Native (E→N)

**Purpose**: Transfer X3 tokens from EVM sidechain back to blockchain L1

| Property | Value |
|----------|-------|
| Source Domain | X3Evm |
| Destination Domain | X3Native |
| Sender Type | EVM AccountBytes ([20] bytes) |
| Recipient Type | X3Native AccountBytes ([32] bytes) |
| Proof Tier | TrustedInternal |
| Ledger Operation | SourceDebited on X3Evm → DestinationCredited on X3Native |
| Atomicity | X3 block finality |
| Status Flow | Created → SourceDebited → DestinationCredited → Finalized |

**Example Flow**:
```
Alice (X3Evm) → 50 X3 → [Router] → Alice (X3Native)
1. Debit 50 from alice_evm() ledger
2. Credit 50 to alice_native() ledger
3. Emit: TransferInitiated, TransferCompleted
```

**Symmetry**: Exact reverse of Route 1

---

### Route 3: X3Native → X3Svm (N→S)

**Purpose**: Transfer X3 tokens from blockchain L1 to Solana-like sidechain

| Property | Value |
|----------|-------|
| Source Domain | X3Native |
| Destination Domain | X3Svm |
| Sender Type | X3Native AccountBytes ([32] bytes) |
| Recipient Type | SVM AccountBytes ([32] bytes) |
| Proof Tier | TrustedInternal |
| Ledger Operation | SourceDebited on X3Native → DestinationCredited on X3Svm |
| Atomicity | X3 block finality |
| Status Flow | Created → SourceDebited → DestinationCredited → Finalized |

**Example Flow**:
```
Alice (X3Native) → 75 X3 → [Router] → Alice (X3Svm)
1. Debit 75 from alice_native() ledger
2. Credit 75 to alice_svm() ledger
3. Emit: TransferInitiated, TransferCompleted
```

---

### Route 4: X3Svm → X3Native (S→N)

**Purpose**: Transfer X3 tokens from Solana-like sidechain back to blockchain L1

| Property | Value |
|----------|-------|
| Source Domain | X3Svm |
| Destination Domain | X3Native |
| Sender Type | SVM AccountBytes ([32] bytes) |
| Recipient Type | X3Native AccountBytes ([32] bytes) |
| Proof Tier | TrustedInternal |
| Ledger Operation | SourceDebited on X3Svm → DestinationCredited on X3Native |
| Atomicity | X3 block finality |
| Status Flow | Created → SourceDebited → DestinationCredited → Finalized |

**Symmetry**: Exact reverse of Route 3

---

### Route 5: X3Evm → X3Svm (E→S)

**Purpose**: Transfer X3 tokens directly from EVM sidechain to Solana-like sidechain

| Property | Value |
|----------|-------|
| Source Domain | X3Evm |
| Destination Domain | X3Svm |
| Sender Type | EVM AccountBytes ([20] bytes) |
| Recipient Type | SVM AccountBytes ([32] bytes) |
| Proof Tier | TrustedInternal |
| Ledger Operation | SourceDebited on X3Evm → DestinationCredited on X3Svm |
| Atomicity | X3 block finality |
| Status Flow | Created → SourceDebited → DestinationCredited → Finalized |

**Example Flow**:
```
Alice (X3Evm) → 30 X3 → [Router] → Alice (X3Svm)
1. Debit 30 from alice_evm() ledger
2. Credit 30 to alice_svm() ledger
3. Emit: TransferInitiated, TransferCompleted
```

**Note**: No requirement to go through X3Native; direct routing supported.

---

### Route 6: X3Svm → X3Evm (S→E)

**Purpose**: Transfer X3 tokens directly from Solana-like sidechain to EVM sidechain

| Property | Value |
|----------|-------|
| Source Domain | X3Svm |
| Destination Domain | X3Evm |
| Sender Type | SVM AccountBytes ([32] bytes) |
| Recipient Type | EVM AccountBytes ([20] bytes) |
| Proof Tier | TrustedInternal |
| Ledger Operation | SourceDebited on X3Svm → DestinationCredited on X3Evm |
| Atomicity | X3 block finality |
| Status Flow | Created → SourceDebited → DestinationCredited → Finalized |

**Symmetry**: Exact reverse of Route 5

---

## Account Type Compatibility Matrix

Different domains use different account types. Routing must validate compatibility:

```
┌──────────────┬──────────────────────┬──────────────────────┐
│ Domain       │ Account Type         │ Size                 │
├──────────────┼──────────────────────┼──────────────────────┤
│ X3Native     │ X3Native([32])       │ 32 bytes             │
│ X3Evm        │ Evm([20])            │ 20 bytes (Ethereum)  │
│ X3Svm        │ Svm([32])            │ 32 bytes (Solana-like)|
└──────────────┴──────────────────────┴──────────────────────┘

Compatibility Rules (enforced by router):
1. Route 1 (N→E): Recipient MUST be Evm type, validating [20] format
2. Route 2 (E→N): Recipient MUST be X3Native type, validating [32] format
3. Route 3 (N→S): Recipient MUST be Svm type, validating [32] format
4. Route 4 (S→N): Recipient MUST be X3Native type, validating [32] format
5. Route 5 (E→S): Recipient MUST be Svm type, validating [32] format
6. Route 6 (S→E): Recipient MUST be Evm type, validating [20] format
```

**Validation Error**: `IncompatibleRecipient` raised if recipient type doesn't match route.

---

## State Machine per Route

Each transfer progresses through identical state transitions regardless of route:

```
┌─────────────┐
│  Created    │ ← Initial state when xvm_transfer() called
└──────┬──────┘
       │
       ├─ [Router calls advance(SourceDebited)]
       │
       ▼
┌─────────────────┐
│  SourceDebited  │ ← Debit executed on source domain ledger
└──────┬──────────┘
       │
       ├─ [Router calls advance(DestinationCredited)]
       │
       ▼
┌──────────────────────┐
│  DestinationCredited │ ← Credit executed on destination domain ledger
└──────┬───────────────┘
       │
       ├─ [Router calls advance(Finalized)]
       │
       ▼
┌──────────────┐
│  Finalized   │ ← Final state, transfer complete
└──────────────┘
       ▲
       │
       └─ [OR if expired: RefundPending → Refunded]
```

**State Transitions** (enforced by `TransferStatus::can_transition_to()`):
```rust
Created         → can move to: SourceDebited
SourceDebited   → can move to: DestinationCredited
DestinationCredited → can move to: Finalized
Finalized       → terminal state
RefundPending   → can move to: Refunded
Refunded        → terminal state
```

---

## Replay Protection: Two Layers

### Layer 1: Message ID Uniqueness (`UsedMessages`)

**Mechanism**: Hash-based deduplication

```
message_id = derive_message_id(&X3TransferMessage {
    version,
    asset_id,
    source_domain,
    destination_domain,
    sender,
    recipient,
    amount,
    nonce,
    created_at,
    expires_at,
})

// On every xvm_transfer() call:
if UsedMessages::contains_key(message_id) {
    return Err(MessageAlreadyProcessed)
}
UsedMessages::insert(message_id, ())
```

**Prevention**: Prevents submitting identical message twice (identical payload + nonce)

---

### Layer 2: Sender Nonce Monotonicity (`NextNonce`)

**Mechanism**: Per-sender sequence numbers

```
// Before xvm_transfer():
let nonce = NextNonce::get((source_domain, sender.clone()))

// Register nonce:
NextNonce::insert((source_domain, sender.clone()), nonce + 1)

// On subsequent xvm_transfer() from same sender:
let next_nonce = NextNonce::get((source_domain, sender.clone()))
if msg.nonce < next_nonce {
    return Err(InvalidNonce)
}
```

**Prevention**: Prevents replaying old transfers from same sender (even with different amounts)

**Example**:
```
Time 1: Alice sends transfer with nonce=0 → NextNonce becomes 1
Time 2: Alice sends transfer with nonce=1 → NextNonce becomes 2
Time 3: Attacker replays Time 1 message with nonce=0 → REJECTED (0 < 2)
```

---

## Pending Transfer Limits

**Storage**: `PendingCount<T>: (AssetId, (Source, Dest)) → u32`

**Purpose**: Prevent route saturation

**Mechanism**:
```
// On xvm_transfer():
let route_key = (asset_id, (source_domain, destination_domain))
let pending = PendingCount::get(&route_key)

if pending >= route_config.limits.max_pending {
    return Err(RouteCapacityExceeded)
}

PendingCount::insert(&route_key, pending + 1)

// On transfer finalization:
PendingCount::insert(&route_key, pending - 1)
```

**Example** (DEV_PERMISSIVE limits):
```
Route 1 (N→E): max 1000 pending transfers
Route 2 (E→N): max 1000 pending transfers
[etc. for all 6 routes]
```

---

## Error Handling Flow

### Errors per Stage

**Stage 1: Route Validation** (before state transition)
```
- RouteNotActive: Route disabled by governance
- RouteClosed: Route explicitly paused
- RouteCapacityExceeded: Too many pending transfers
- IncompatibleRecipient: Recipient type doesn't match route
```

**Stage 2: Replay Protection**
```
- MessageAlreadyProcessed: Message ID already seen (Layer 1)
- InvalidNonce: Sender nonce out of order (Layer 2)
```

**Stage 3: Amount Validation**
```
- AmountOutOfBounds: amount < min or > max
- InsufficientBalance: Source ledger lacks balance
```

**Stage 4: State Machine**
```
- InvalidStateTransition: Cannot move from current state to next
- TransferNotFound: Message ID not in Transfers map
- NotYetExpired: Cannot refund transfer before expiry
```

---

## Cross-VM Packet Integration (Phase 1.3 ↔ Phase 1.4)

**Connection Point**: Packet deserializer output feeds into route decision

### Packet Domain Mask

Phase 1.3 packet deserializer extracts **domain mask** = bitmask indicating which domains can receive transfer:

```rust
enum DomainRoute {
    EvmOnly = 0b0001,       // recipient.is_evm()
    SvmOnly = 0b0010,       // recipient.is_svm()
    X3VmOnly = 0b0100,      // recipient.is_x3native()
    EvmAndSvm = 0b0011,     // multiple targets allowed
    AllDomains = 0b0111,    // all domains allowed
}
```

### Routing Decision Example

```
Packet: {
    recipient: [20 bytes] (EVM address),
    domain_mask: 0b0001,
    ...
}

Route validation:
- Requested destination: X3Svm
- Packet allows: EvmOnly (0b0001)
- Result: REJECT (destination not in allowed domain mask)

Correct destination: X3Evm
- Requested destination: X3Evm
- Packet allows: EvmOnly (0b0001)
- Result: ACCEPT, routing allowed
```

---

## Golden-Path Test: Route 1 (N→E)

**Scenario**: Alice transfers 100 X3 from X3Native to X3Evm

```rust
#[test]
fn test_route_1_native_to_evm() {
    new_test_ext().execute_with(|| {
        let asset_id = bootstrap_x3_asset(1_000_000_000);
        
        // Canonical supply on X3Native: 1 billion
        assert_eq!(Ledger::balance(asset_id, DomainId::X3Native, alice_native()), 1_000_000_000);
        
        // Execute transfer
        let msg_id = do_xvm(
            asset_id,
            DomainId::X3Native,
            DomainId::X3Evm,
            100,
        );
        
        // Verify debit from source
        assert_eq!(
            Ledger::balance(asset_id, DomainId::X3Native, alice_native()),
            1_000_000_000 - 100
        );
        
        // Verify credit to destination
        assert_eq!(
            Ledger::balance(asset_id, DomainId::X3Evm, alice_evm()),
            100
        );
        
        // Verify transfer finalized
        let transfer = Router::transfers(msg_id).expect("transfer record");
        assert_eq!(transfer.status, TransferStatus::Finalized);
    })
}
```

---

## Summary Table: All 6 Routes

```
Route │ Source      │ Dest        │ Sender Type       │ Recipient Type    │ Status
──────┼─────────────┼─────────────┼───────────────────┼───────────────────┼──────────
  1   │ X3Native    │ X3Evm       │ X3Native([32])    │ Evm([20])         │ ✅ MVP
  2   │ X3Evm       │ X3Native    │ Evm([20])         │ X3Native([32])    │ ✅ MVP
  3   │ X3Native    │ X3Svm       │ X3Native([32])    │ Svm([32])         │ ✅ MVP
  4   │ X3Svm       │ X3Native    │ Svm([32])         │ X3Native([32])    │ ✅ MVP
  5   │ X3Evm       │ X3Svm       │ Evm([20])         │ Svm([32])         │ ✅ MVP
  6   │ X3Svm       │ X3Evm       │ Svm([32])         │ Evm([20])         │ ✅ MVP
```

---

## Next Steps

**Phase 1.4 Implementation**:
1. ✅ Define 6-route matrix (THIS DOCUMENT)
2. Create reference implementation guide (NEXT)
3. Rewrite test infrastructure with working tests
4. Run complete 6-route golden-path test
5. Verify replay protection layers
6. Validate error handling per route

---

**Status**: Complete specification  
**Implementation Ready**: Yes, all routes fully specified  
**Test Coverage Target**: All 6 routes + replay protection + error handling = 12+ tests

