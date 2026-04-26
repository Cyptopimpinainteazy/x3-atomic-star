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

    #[test]
    fn test_svm_initialize_state_packet_round_trip() {
        let packet = SvmPacket::InitializeState {
            account: [0x22; 32],
            state: vec![42, 43, 44],
        };

        let encoded = packet.encode();
        let decoded: SvmPacket = Decode::decode(&mut &encoded[..]).unwrap();
        assert_eq!(packet, decoded);
    }
}
