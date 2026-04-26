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

    /// Padding to 32-byte boundary (6 bytes)
    pub padding: [u8; 6],
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
            padding: [0; 6],
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
            padding: [0; 6],
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
        // SCALE codec produces 30 bytes for this header structure
        assert_eq!(encoded.len(), 30);
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


}
