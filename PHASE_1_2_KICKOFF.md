# Phase 1.2: Packet Schema Implementation Kickoff

**Phase:** 1.2 (Packet Schema & SCALE Codec Integration)  
**Status:** 🟢 READY TO START  
**Estimated Duration:** 4-5 hours  
**Target Tests:** 15+ unit tests  

---

## Quick Summary

Phase 1.2 creates the foundational packet schema crate with type-safe Rust structs for EVM, SVM, and X3VM payloads. Using SCALE codec (Substrate's standard serialization), we'll enable deterministic packet encoding/decoding.

**Deliverables:**
- ✅ `crates/x3-packet-schema/` crate
- ✅ `PacketHeader` struct (32-byte fixed)
- ✅ `EvmPacket`, `SvmPacket`, `X3VmPacket` enums
- ✅ Serialization/deserialization with validation
- ✅ 15+ round-trip tests passing

---

## File Structure to Create

```
crates/x3-packet-schema/
├── Cargo.toml (NEW)
├── src/
│   ├── lib.rs (NEW)
│   ├── header.rs (NEW - PacketHeader)
│   ├── evm.rs (NEW - EvmPacket, EvmCall, U256)
│   ├── svm.rs (NEW - SvmPacket, SvmAccount, SvmDeployMetadata)
│   ├── x3vm.rs (NEW - X3VmPacket, X3Condition, Transfer)
│   ├── validation.rs (NEW - checksum, size validation)
│   ├── tests.rs (NEW - 15+ tests)
│   └── prelude.rs (NEW - re-exports for convenience)
```

---

## Step-by-Step Implementation

### Step 1: Create Cargo.toml

```toml
[package]
name = "x3-packet-schema"
version = "0.1.0"
edition = "2021"
license = "Apache-2.0"

[dependencies]
parity-scale-codec = { version = "3.14", default-features = false, features = ["derive", "max-encoded-len"] }
scale-info = { version = "2.11", default-features = false, features = ["derive"] }
frame-support = { version = "30", default-features = false }
sp-core = { version = "30", default-features = false }
sp-std = { version = "14", default-features = false }
serde = { version = "1.0", default-features = false, features = ["derive"], optional = true }

[features]
default = ["std"]
std = [
    "parity-scale-codec/std",
    "scale-info/std",
    "frame-support/std",
    "sp-core/std",
    "sp-std/std",
    "serde/std",
]

[dev-dependencies]
quickcheck = "1.0"
quickcheck_macros = "1.0"
```

### Step 2: Create src/header.rs

```rust
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

/// Fixed 32-byte packet header for all packet types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct PacketHeader {
    /// Format version (currently 1)
    pub version: u8,

    /// Destination VM(s) bitmask
    /// Bit 0: EVM, Bit 1: SVM, Bit 2: X3VM
    pub domain_mask: u8,

    /// Packet type
    pub packet_type: u8,

    /// Reserved for future use
    pub reserved: u8,

    /// Payload size in bytes (max 65535)
    pub payload_size: u16,

    /// Checksum of payload (first 8 bytes of blake2_256)
    pub checksum: u64,

    /// Packet sequence number (per sender, per block)
    pub sequence: u16,

    /// Expiry block height (0 = no expiry)
    pub expires_at: u32,

    /// Domain-specific routing hint
    pub routing_hint: u32,

    /// Padding to 32-byte boundary
    pub padding: [u8; 2],
}

impl PacketHeader {
    /// Create header with defaults
    pub fn new(version: u8, domain_mask: u8, payload_size: u16) -> Self {
        Self {
            version,
            domain_mask,
            packet_type: 0,
            reserved: 0,
            payload_size,
            checksum: 0,
            sequence: 0,
            expires_at: 0,
            routing_hint: 0,
            padding: [0; 2],
        }
    }

    /// Check if packet targets EVM
    pub fn targets_evm(&self) -> bool {
        self.domain_mask & 0b0001 != 0
    }

    /// Check if packet targets SVM
    pub fn targets_svm(&self) -> bool {
        self.domain_mask & 0b0010 != 0
    }

    /// Check if packet targets X3VM
    pub fn targets_x3vm(&self) -> bool {
        self.domain_mask & 0b0100 != 0
    }

    /// Check if packet is expired at given block height
    pub fn is_expired(&self, current_block: u32) -> bool {
        self.expires_at > 0 && current_block >= self.expires_at
    }

    /// Validate header fields
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.version != 1 {
            return Err("Invalid packet version");
        }
        if self.payload_size > 65000 {
            return Err("Payload size exceeds kernel limit");
        }
        if self.domain_mask == 0 {
            return Err("Must target at least one domain");
        }
        Ok(())
    }
}

impl Default for PacketHeader {
    fn default() -> Self {
        Self {
            version: 1,
            domain_mask: 0b0111,  // All domains by default
            packet_type: 0,
            reserved: 0,
            payload_size: 0,
            checksum: 0,
            sequence: 0,
            expires_at: 0,
            routing_hint: 0,
            padding: [0; 2],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_is_32_bytes() {
        let header = PacketHeader::default();
        let encoded = header.encode();
        assert_eq!(encoded.len(), 32);
    }

    #[test]
    fn test_header_domain_mask_evm_only() {
        let mut header = PacketHeader::default();
        header.domain_mask = 0b0001;
        assert!(header.targets_evm());
        assert!(!header.targets_svm());
        assert!(!header.targets_x3vm());
    }

    #[test]
    fn test_header_expiry_validation() {
        let mut header = PacketHeader::default();
        header.expires_at = 1000;

        assert!(!header.is_expired(999));
        assert!(header.is_expired(1000));
        assert!(header.is_expired(1001));
    }

    #[test]
    fn test_header_validation_rejects_invalid_version() {
        let mut header = PacketHeader::default();
        header.version = 99;

        assert!(header.validate().is_err());
    }

    #[test]
    fn test_header_validation_rejects_oversized_payload() {
        let mut header = PacketHeader::default();
        header.payload_size = 70000;

        assert!(header.validate().is_err());
    }
}
```

### Step 3: Create src/evm.rs

```rust
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

/// U256 type for Ethereum
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[repr(transparent)]
pub struct U256(pub [u8; 32]);

impl U256 {
    pub fn from(value: u64) -> Self {
        let mut bytes = [0u8; 32];
        bytes[24..32].copy_from_slice(&value.to_be_bytes());
        U256(bytes)
    }

    pub fn zero() -> Self {
        U256([0u8; 32])
    }
}

impl Default for U256 {
    fn default() -> Self {
        Self::zero()
    }
}

/// EVM contract call target
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct EvmCall {
    /// Contract address (20 bytes)
    pub contract: [u8; 20],

    /// Function selector (4 bytes)
    pub function_selector: [u8; 4],

    /// ABI-encoded arguments
    pub args: Vec<u8>,
}

/// EVM packet variants
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub enum EvmPacket {
    /// Call smart contract
    Call {
        contract: [u8; 20],
        function_selector: [u8; 4],
        args: Vec<u8>,
        value: U256,
    },

    /// Deploy contract
    Deploy {
        bytecode: Vec<u8>,
        args: Vec<u8>,
        value: U256,
    },

    /// Batch multiple calls
    Batch {
        calls: Vec<(EvmCall, Option<U256>)>,
        continue_on_revert: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u256_from_u64() {
        let val = U256::from(1000);
        assert_eq!(val.0[31], 232);  // 1000 in last byte of big-endian
    }

    #[test]
    fn test_evm_call_packet_round_trip() {
        let packet = EvmPacket::Call {
            contract: [0x42; 20],
            function_selector: [0xaa, 0xbb, 0xcc, 0xdd],
            args: vec![1, 2, 3],
            value: U256::from(1000),
        };

        let encoded = packet.encode();
        let decoded: EvmPacket = Decode::decode(&mut &encoded[..]).unwrap();
        assert_eq!(packet, decoded);
    }

    #[test]
    fn test_evm_batch_packet_round_trip() {
        let packet = EvmPacket::Batch {
            calls: vec![
                (EvmCall {
                    contract: [0x11; 20],
                    function_selector: [0x12, 0x34, 0x56, 0x78],
                    args: vec![],
                }, None),
            ],
            continue_on_revert: true,
        };

        let encoded = packet.encode();
        let decoded: EvmPacket = Decode::decode(&mut &encoded[..]).unwrap();
        assert_eq!(packet, decoded);
    }
}
```

### Step 4: Create src/svm.rs

```rust
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

/// SVM account metadata
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct SvmAccount {
    pub pubkey: [u8; 32],
    pub is_writable: bool,
    pub is_signer: bool,
    pub is_executable: bool,
    pub lamports: u64,
    pub owner: [u8; 32],
}

/// SVM deployment metadata
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct SvmDeployMetadata {
    pub name: Vec<u8>,
    pub version: Vec<u8>,
    pub upgrade_authority: Option<[u8; 32]>,
}

/// SVM packet variants
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub enum SvmPacket {
    /// Invoke program
    Invoke {
        program_id: [u8; 32],
        accounts: Vec<SvmAccount>,
        data: Vec<u8>,
    },

    /// Deploy program
    Deploy {
        bytecode: Vec<u8>,
        metadata: SvmDeployMetadata,
    },

    /// Initialize state account
    InitializeState {
        account: [u8; 32],
        state: Vec<u8>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_svm_invoke_packet_round_trip() {
        let packet = SvmPacket::Invoke {
            program_id: [0x11; 32],
            accounts: vec![],
            data: vec![1, 2, 3],
        };

        let encoded = packet.encode();
        let decoded: SvmPacket = Decode::decode(&mut &encoded[..]).unwrap();
        assert_eq!(packet, decoded);
    }

    #[test]
    fn test_svm_deploy_packet_round_trip() {
        let packet = SvmPacket::Deploy {
            bytecode: vec![0xBF; 100],
            metadata: SvmDeployMetadata {
                name: b"test".to_vec(),
                version: b"1.0".to_vec(),
                upgrade_authority: Some([0xAA; 32]),
            },
        };

        let encoded = packet.encode();
        let decoded: SvmPacket = Decode::decode(&mut &encoded[..]).unwrap();
        assert_eq!(packet, decoded);
    }
}
```

### Step 5: Create src/x3vm.rs

```rust
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

/// X3VM Condition for conditional execution
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub enum X3Condition {
    BlockHeightAbove { min_height: u32 },
    Always,
}

/// X3VM packet variants
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub enum X3VmPacket {
    /// Atomic cross-VM transaction
    AtomicCross {
        evm: Option<Vec<u8>>,  // Serialized EvmPacket
        svm: Option<Vec<u8>>,  // Serialized SvmPacket
        atomic: bool,
    },

    /// Conditional execution
    Conditional {
        condition: X3Condition,
        if_true: Box<X3VmPacket>,
        if_false: Option<Box<X3VmPacket>>,
    },

    /// Value transfer across domains
    Transfer {
        from_domain: u8,
        to_domain: u8,
        asset_id: u32,
        amount: u128,
        recipient: Vec<u8>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_x3vm_transfer_packet_round_trip() {
        let packet = X3VmPacket::Transfer {
            from_domain: 0,
            to_domain: 1,
            asset_id: 0,
            amount: 1000000,
            recipient: vec![0xAA; 32],
        };

        let encoded = packet.encode();
        let decoded: X3VmPacket = Decode::decode(&mut &encoded[..]).unwrap();
        assert_eq!(packet, decoded);
    }
}
```

### Step 6: Create src/lib.rs

```rust
#![cfg_attr(not(feature = "std"), no_std)]

mod header;
mod evm;
mod svm;
mod x3vm;

pub use header::PacketHeader;
pub use evm::{EvmPacket, EvmCall, U256};
pub use svm::{SvmPacket, SvmAccount, SvmDeployMetadata};
pub use x3vm::{X3VmPacket, X3Condition};

use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

/// Top-level packet wrapper for router dispatch
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub enum Packet {
    Evm(EvmPacket),
    Svm(SvmPacket),
    X3Vm(X3VmPacket),
}

impl Packet {
    /// Get the domain mask for routing
    pub fn domain_mask(&self) -> u8 {
        match self {
            Packet::Evm(_) => 0b0001,
            Packet::Svm(_) => 0b0010,
            Packet::X3Vm(_) => 0b0100,
        }
    }

    /// Serialize to Vec<u8>
    pub fn to_bytes(&self) -> Vec<u8> {
        self.encode()
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, parity_scale_codec::Error> {
        Self::decode(&mut &bytes[..])
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_packet_wrapper_round_trip() {
        let evm_packet = EvmPacket::Call {
            contract: [0x42; 20],
            function_selector: [0xaa, 0xbb, 0xcc, 0xdd],
            args: vec![],
            value: U256::from(0),
        };

        let packet = Packet::Evm(evm_packet.clone());
        let bytes = packet.to_bytes();
        let decoded = Packet::from_bytes(&bytes).unwrap();

        assert_eq!(packet, decoded);
    }
}

pub mod prelude {
    pub use crate::{Packet, PacketHeader, EvmPacket, SvmPacket, X3VmPacket};
}
```

---

## Testing Checklist

```bash
# Test compilation
✅ cargo build -p x3-packet-schema

# Run all tests
✅ cargo test -p x3-packet-schema --lib

# Test header (32 bytes)
✅ test_header_is_32_bytes

# Test EVM serialization (5+ tests)
✅ test_evm_call_packet_round_trip
✅ test_evm_deploy_packet_round_trip
✅ test_evm_batch_packet_round_trip
✅ test_evm_call_to_max_size_contract
✅ test_packet_empty_payload_handling

# Test SVM serialization (3+ tests)
✅ test_svm_invoke_packet_round_trip
✅ test_svm_deploy_packet_round_trip
✅ test_svm_initialize_state_packet_round_trip

# Test X3VM serialization (3+ tests)
✅ test_x3vm_atomic_cross_packet_round_trip
✅ test_x3vm_transfer_packet_round_trip
✅ test_x3vm_conditional_packet_round_trip

# Test wrapper integration (1+ tests)
✅ test_packet_wrapper_round_trip

# Total: 15+ tests minimum
```

---

## Integration with Workspace

1. **Update root Cargo.toml:**
   ```toml
   [workspace.members]
   # ... existing members ...
   "crates/x3-packet-schema"
   ```

2. **Add to root workspace dependencies:**
   ```toml
   x3-packet-schema = { path = "crates/x3-packet-schema" }
   ```

3. **Commit structure:**
   ```bash
   git add crates/x3-packet-schema/
   git commit -m "feat(phase-1.2): add packet schema crate with SCALE codec integration"
   ```

---

## Success Criteria

✅ All 15+ tests passing  
✅ Crate compiles cleanly with no warnings  
✅ 32-byte header verified  
✅ All packet types serialize/deserialize deterministically  
✅ Zero panics on edge cases  
✅ Ready for Phase 1.3 (adapter integration)  

---

**Estimated Time:** 4-5 hours  
**Next Phase:** Phase 1.3 - Adapter Integration Layer  

*Ready to start Phase 1.2? Begin with creating the Cargo.toml and directory structure.*
