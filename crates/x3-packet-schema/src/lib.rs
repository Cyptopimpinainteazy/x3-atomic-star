#![cfg_attr(not(feature = "std"), no_std)]

use scale_info::prelude::vec::Vec;

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

    #[test]
    fn test_packet_domain_masks() {
        let evm_pkt = Packet::Evm(EvmPacket::Call {
            contract: [0; 20],
            function_selector: [0; 4],
            args: vec![],
            value: U256::zero(),
        });
        assert_eq!(evm_pkt.domain_mask(), 0b0001);

        let svm_pkt = Packet::Svm(SvmPacket::Invoke {
            program_id: [0; 32],
            accounts: vec![],
            data: vec![],
        });
        assert_eq!(svm_pkt.domain_mask(), 0b0010);

        let x3vm_pkt = Packet::X3Vm(X3VmPacket::Transfer {
            from_domain: 0,
            to_domain: 1,
            asset_id: 0,
            amount: 0,
            recipient: vec![],
        });
        assert_eq!(x3vm_pkt.domain_mask(), 0b0100);
    }
}

pub mod prelude {
    pub use crate::{Packet, PacketHeader, EvmPacket, SvmPacket, X3VmPacket};
}
