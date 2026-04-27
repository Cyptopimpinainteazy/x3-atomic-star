use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use scale_info::prelude::{boxed::Box, vec::Vec};

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

    #[test]
    fn test_x3vm_atomic_cross_packet_round_trip() {
        let packet = X3VmPacket::AtomicCross {
            evm: Some(vec![1, 2, 3]),
            svm: Some(vec![4, 5, 6]),
            atomic: true,
        };

        let encoded = packet.encode();
        let decoded: X3VmPacket = Decode::decode(&mut &encoded[..]).unwrap();
        assert_eq!(packet, decoded);
    }

    #[test]
    fn test_x3vm_conditional_packet_round_trip() {
        let packet = X3VmPacket::Conditional {
            condition: X3Condition::BlockHeightAbove { min_height: 1000 },
            if_true: Box::new(X3VmPacket::Transfer {
                from_domain: 0,
                to_domain: 1,
                asset_id: 0,
                amount: 100,
                recipient: vec![],
            }),
            if_false: None,
        };

        let encoded = packet.encode();
        let decoded: X3VmPacket = Decode::decode(&mut &encoded[..]).unwrap();
        assert_eq!(packet, decoded);
    }
}
