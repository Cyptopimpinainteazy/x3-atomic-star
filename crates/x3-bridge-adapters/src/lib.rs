//! X3 Bridge Adapters
//!
//! This crate provides implementations of bridge adapters for external chains
//! (Ethereum, Solana, Bitcoin) that integrate with the X3 cross-chain gateway.

pub mod ethereum;
pub mod solana;
pub mod bitcoin;

pub use ethereum::EthereumBridgeAdapter;
pub use solana::SolanaBridgeAdapter;
pub use bitcoin::BitcoinBridgeAdapter;

/// Bridge adapter trait for external chain integration
pub trait BridgeAdapter {
    /// Get the chain name
    fn chain_name(&self) -> &str;
    
    /// Get the chain ID
    fn chain_id(&self) -> u64;
    
    /// Validate a block header
    fn validate_header(&self, header: &[u8]) -> Result<(), BridgeError>;
    
    /// Generate a proof for a block
    fn generate_proof(&self, block_number: u64) -> Result<Vec<u8>, BridgeError>;
    
    /// Get the latest block number
    fn get_latest_block_number(&self) -> Result<u64, BridgeError>;
}

/// Bridge adapter error
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Timeout error")]
    Timeout,
}
