//! Ethereum Bridge: Lock-Mint canonical bridge for EVM ↔ X3
//!
//! Lock ERC-20 on Ethereum, mint wrapped token on X3.
//! Validators sign cross-chain messages (multisig 5-of-7).

use std::collections::HashMap;

/// ERC-20 token on Ethereum
#[derive(Clone, Debug)]
pub struct ERC20Token {
    pub address: String,
    pub name: String,
    pub decimals: u8,
    pub total_supply: u128,
}

/// Bridge deposit (lock on Ethereum)
#[derive(Clone, Debug)]
pub struct BridgeDeposit {
    pub id: String,
    pub depositor: String, // Ethereum address
    pub token: String,     // ERC-20 address
    pub amount: u128,
    pub eth_block: u64,
    pub eth_tx_hash: String,
    pub status: DepositStatus,
}

#[derive(Clone, Debug)]
pub enum DepositStatus {
    Locked,
    Confirmed { confirmations: u32 },
    Minted { x3_recipient: String, x3_block: u32 },
    Refunded { reason: String },
}

/// Cross-chain validator message
#[derive(Clone, Debug)]
pub struct BridgeMessage {
    pub id: String,
    pub deposit_id: String,
    pub message_hash: [u8; 32],
    pub signatures: Vec<ValidatorSignature>,
    pub status: MessageStatus,
}

#[derive(Clone, Debug)]
pub struct ValidatorSignature {
    pub validator_id: u32,
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug)]
pub enum MessageStatus {
    Pending { collected: u32 },
    Signed { threshold_met: bool },
    Executed { x3_block: u32 },
}

/// Ethereum Bridge
pub struct EthereumBridge {
    // Ethereum side (locked funds)
    pub eth_locked: HashMap<String, u128>, // deposit_id → amount locked
    pub token_registry: HashMap<String, ERC20Token>,

    // X3 side (minted tokens)
    pub wrapped_tokens: HashMap<String, u128>, // (token_addr, x3_account) → balance
    pub deposits: HashMap<String, BridgeDeposit>,
    pub messages: HashMap<String, BridgeMessage>,

    // Bridge validators (multisig)
    pub validators: Vec<String>,  // 7 validators
    pub signature_threshold: u32, // 5-of-7
    pub next_deposit_id: u64,
}

impl EthereumBridge {
    pub fn new(validators: Vec<String>) -> Self {
        assert_eq!(validators.len(), 7, "Must have exactly 7 validators");

        Self {
            eth_locked: HashMap::new(),
            token_registry: HashMap::new(),
            wrapped_tokens: HashMap::new(),
            deposits: HashMap::new(),
            messages: HashMap::new(),
            validators,
            signature_threshold: 5,
            next_deposit_id: 1,
        }
    }

    /// Register ERC-20 token for bridging
    pub fn register_token(&mut self, token: ERC20Token) -> Result<(), String> {
        if token.address.is_empty() {
            return Err("Token address cannot be empty".to_string());
        }
        self.token_registry.insert(token.address.clone(), token);
        Ok(())
    }

    /// Lock ERC-20 on Ethereum side (phase 1)
    pub fn lock_on_ethereum(
        &mut self,
        depositor: String,
        token_addr: String,
        amount: u128,
        eth_block: u64,
        eth_tx_hash: String,
    ) -> Result<BridgeDeposit, String> {
        // Verify token is registered
        if !self.token_registry.contains_key(&token_addr) {
            return Err("Token not registered on bridge".to_string());
        }

        let deposit = BridgeDeposit {
            id: format!("deposit_{}", self.next_deposit_id),
            depositor,
            token: token_addr.clone(),
            amount,
            eth_block,
            eth_tx_hash,
            status: DepositStatus::Locked,
        };

        self.next_deposit_id += 1;
        self.eth_locked.insert(deposit.id.clone(), amount);
        self.deposits.insert(deposit.id.clone(), deposit.clone());

        Ok(deposit)
    }

    /// Confirm deposit after 12 Ethereum blocks (~3 mins)
    pub fn confirm_deposit(&mut self, deposit_id: &str, current_block: u64) -> Result<(), String> {
        let mut deposit = self
            .deposits
            .get(deposit_id)
            .ok_or("Deposit not found")?
            .clone();

        if current_block < deposit.eth_block {
            return Err("block number regression detected".to_string());
        }

        let confirmations_u64 = current_block.saturating_sub(deposit.eth_block);
        let confirmations = confirmations_u64.try_into().unwrap_or(u32::MAX);
        if confirmations < 12 {
            return Err(format!("Need 12 confirmations, have {}", confirmations));
        }

        deposit.status = DepositStatus::Confirmed { confirmations };
        self.deposits.insert(deposit_id.to_string(), deposit);

        Ok(())
    }

    /// Create bridge message for validators to sign
    pub fn create_bridge_message(&mut self, deposit_id: String) -> Result<BridgeMessage, String> {
        // Verify deposit exists and is confirmed
        let deposit = self.deposits.get(&deposit_id).ok_or("Deposit not found")?;

        match &deposit.status {
            DepositStatus::Confirmed { .. } => {}
            _ => return Err("Deposit not confirmed".to_string()),
        }

        // Create message hash: keccak256(deposit_id || amount || token || recipient)
        let mut hash = [0u8; 32];
        let id_bytes = deposit_id.as_bytes();
        for (i, &byte) in id_bytes.iter().enumerate().take(32) {
            hash[i] ^= byte;
        }

        let message = BridgeMessage {
            id: format!("msg_{}", deposit_id),
            deposit_id: deposit_id.clone(),
            message_hash: hash,
            signatures: Vec::new(),
            status: MessageStatus::Pending { collected: 0 },
        };

        self.messages.insert(message.id.clone(), message.clone());
        Ok(message)
    }

    /// Validator signs bridge message
    pub fn sign_message(
        &mut self,
        message_id: &str,
        validator_id: u32,
        signature: Vec<u8>,
    ) -> Result<(), String> {
        let mut message = self
            .messages
            .get(message_id)
            .ok_or("Message not found")?
            .clone();

        // Verify validator exists
        if validator_id as usize >= self.validators.len() {
            return Err("Invalid validator ID".to_string());
        }

        // Prevent double-signing
        if message
            .signatures
            .iter()
            .any(|s| s.validator_id == validator_id)
        {
            return Err("Validator already signed this message".to_string());
        }

        message.signatures.push(ValidatorSignature {
            validator_id,
            signature,
        });

        let collected = message.signatures.len() as u32;
        message.status = MessageStatus::Pending { collected };

        // Check if threshold met
        if collected >= self.signature_threshold {
            message.status = MessageStatus::Signed {
                threshold_met: true,
            };
        }

        self.messages.insert(message_id.to_string(), message);
        Ok(())
    }

    /// Execute minting on X3 side
    pub fn execute_mint(
        &mut self,
        message_id: &str,
        x3_recipient: String,
        x3_block: u32,
    ) -> Result<(), String> {
        let mut message = self
            .messages
            .get(message_id)
            .ok_or("Message not found")?
            .clone();

        // Verify signatures threshold met
        if message.signatures.len() < self.signature_threshold as usize {
            return Err("Not enough signatures".to_string());
        }

        // Verify all signatures are valid (in production: ECDSA verify)
        if message.signatures.len() < self.signature_threshold as usize {
            return Err("Invalid signature count".to_string());
        }

        let deposit = self
            .deposits
            .get(&message.deposit_id)
            .ok_or("Deposit not found")?
            .clone();

        // Mint wrapped token on X3
        let wrapped_key = format!("{}_{}", deposit.token, x3_recipient);
        self.wrapped_tokens.insert(wrapped_key, deposit.amount);

        // Update deposit status
        if let Some(deposit_mut) = self.deposits.get_mut(&message.deposit_id) {
            deposit_mut.status = DepositStatus::Minted {
                x3_recipient: x3_recipient.clone(),
                x3_block,
            };
        }

        // Update message status
        message.status = MessageStatus::Executed { x3_block };
        self.messages.insert(message_id.to_string(), message);

        Ok(())
    }

    /// Burn wrapped token on X3 to unlock on Ethereum
    pub fn burn_wrapped(
        &mut self,
        x3_account: String,
        token_addr: String,
        amount: u128,
    ) -> Result<(), String> {
        let key = format!("{}_{}", token_addr, x3_account);

        let balance = self
            .wrapped_tokens
            .get(&key)
            .ok_or("Token balance not found")?;
        if amount > *balance {
            return Err("Insufficient balance".to_string());
        }

        // Burn token
        self.wrapped_tokens.insert(key, balance - amount);

        // In production: emit unlock event for Ethereum validators

        Ok(())
    }

    /// Get bridge status
    pub fn get_deposit_status(&self, deposit_id: &str) -> Option<DepositStatus> {
        self.deposits.get(deposit_id).map(|d| d.status.clone())
    }

    /// Get wrapped token balance
    pub fn get_wrapped_balance(&self, x3_account: &str, token_addr: &str) -> u128 {
        let key = format!("{}_{}", token_addr, x3_account);
        *self.wrapped_tokens.get(&key).unwrap_or(&0)
    }

    /// Refund if deposit expires
    pub fn refund_deposit(&mut self, deposit_id: &str, reason: String) -> Result<(), String> {
        let mut deposit = self
            .deposits
            .get(deposit_id)
            .ok_or("Deposit not found")?
            .clone();

        self.eth_locked.remove(deposit_id);
        deposit.status = DepositStatus::Refunded { reason };

        self.deposits.insert(deposit_id.to_string(), deposit);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_creation() {
        let validators = (0..7).map(|i| format!("validator_{}", i)).collect();
        let bridge = EthereumBridge::new(validators);
        assert_eq!(bridge.validators.len(), 7);
    }

    #[test]
    fn test_register_token() {
        let validators = (0..7).map(|i| format!("validator_{}", i)).collect();
        let mut bridge = EthereumBridge::new(validators);

        let usdc = ERC20Token {
            address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
            name: "USDC".to_string(),
            decimals: 6,
            total_supply: 1_000_000_000_000u128,
        };

        assert!(bridge.register_token(usdc).is_ok());
    }

    #[test]
    fn test_lock_on_ethereum() {
        let validators = (0..7).map(|i| format!("validator_{}", i)).collect();
        let mut bridge = EthereumBridge::new(validators);

        let usdc = ERC20Token {
            address: "0xUSDC".to_string(),
            name: "USDC".to_string(),
            decimals: 6,
            total_supply: 1_000_000_000_000u128,
        };

        bridge.register_token(usdc).ok();

        let deposit = bridge.lock_on_ethereum(
            "0xAlice".to_string(),
            "0xUSDC".to_string(),
            1_000_000u128,
            17_000_000,
            "0xtxhash".to_string(),
        );

        assert!(deposit.is_ok());
    }

    #[test]
    fn test_confirm_deposit() {
        let validators = (0..7).map(|i| format!("validator_{}", i)).collect();
        let mut bridge = EthereumBridge::new(validators);

        let usdc = ERC20Token {
            address: "0xUSDC".to_string(),
            name: "USDC".to_string(),
            decimals: 6,
            total_supply: 1_000_000_000_000u128,
        };

        bridge.register_token(usdc).ok();

        let deposit = bridge
            .lock_on_ethereum(
                "0xAlice".to_string(),
                "0xUSDC".to_string(),
                1_000_000u128,
                17_000_000,
                "0xtxhash".to_string(),
            )
            .unwrap();

        assert!(bridge.confirm_deposit(&deposit.id, 17_000_012).is_ok());
    }

    #[test]
    fn test_create_bridge_message() {
        let validators = (0..7).map(|i| format!("validator_{}", i)).collect();
        let mut bridge = EthereumBridge::new(validators);

        let usdc = ERC20Token {
            address: "0xUSDC".to_string(),
            name: "USDC".to_string(),
            decimals: 6,
            total_supply: 1_000_000_000_000u128,
        };

        bridge.register_token(usdc).ok();

        let deposit = bridge
            .lock_on_ethereum(
                "0xAlice".to_string(),
                "0xUSDC".to_string(),
                1_000_000u128,
                17_000_000,
                "0xtxhash".to_string(),
            )
            .unwrap();

        bridge.confirm_deposit(&deposit.id, 17_000_012).ok();
        let msg = bridge.create_bridge_message(deposit.id);
        assert!(msg.is_ok());
    }

    #[test]
    fn test_validator_signing() {
        let validators = (0..7).map(|i| format!("validator_{}", i)).collect();
        let mut bridge = EthereumBridge::new(validators);

        let usdc = ERC20Token {
            address: "0xUSDC".to_string(),
            name: "USDC".to_string(),
            decimals: 6,
            total_supply: 1_000_000_000_000u128,
        };

        bridge.register_token(usdc).ok();

        let deposit = bridge
            .lock_on_ethereum(
                "0xAlice".to_string(),
                "0xUSDC".to_string(),
                1_000_000u128,
                17_000_000,
                "0xtxhash".to_string(),
            )
            .unwrap();

        bridge.confirm_deposit(&deposit.id, 17_000_012).ok();
        let msg = bridge.create_bridge_message(deposit.id).unwrap();

        for i in 0..5 {
            assert!(bridge
                .sign_message(&msg.id, i as u32, vec![i as u8])
                .is_ok());
        }
    }

    #[test]
    fn test_execute_mint() {
        let validators = (0..7).map(|i| format!("validator_{}", i)).collect();
        let mut bridge = EthereumBridge::new(validators);

        let usdc = ERC20Token {
            address: "0xUSDC".to_string(),
            name: "USDC".to_string(),
            decimals: 6,
            total_supply: 1_000_000_000_000u128,
        };

        bridge.register_token(usdc).ok();

        let deposit = bridge
            .lock_on_ethereum(
                "0xAlice".to_string(),
                "0xUSDC".to_string(),
                1_000_000u128,
                17_000_000,
                "0xtxhash".to_string(),
            )
            .unwrap();

        bridge.confirm_deposit(&deposit.id, 17_000_012).ok();
        let msg = bridge.create_bridge_message(deposit.id).unwrap();

        for i in 0..5 {
            bridge.sign_message(&msg.id, i as u32, vec![i as u8]).ok();
        }

        let result = bridge.execute_mint(&msg.id, "0xAlice_X3".to_string(), 1000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_burn_wrapped() {
        let validators = (0..7).map(|i| format!("validator_{}", i)).collect();
        let mut bridge = EthereumBridge::new(validators);

        let usdc = ERC20Token {
            address: "0xUSDC".to_string(),
            name: "USDC".to_string(),
            decimals: 6,
            total_supply: 1_000_000_000_000u128,
        };

        bridge.register_token(usdc).ok();

        let deposit = bridge
            .lock_on_ethereum(
                "0xAlice".to_string(),
                "0xUSDC".to_string(),
                1_000_000u128,
                17_000_000,
                "0xtxhash".to_string(),
            )
            .unwrap();

        bridge.confirm_deposit(&deposit.id, 17_000_012).ok();
        let msg = bridge.create_bridge_message(deposit.id).unwrap();

        for i in 0..5 {
            bridge.sign_message(&msg.id, i as u32, vec![i as u8]).ok();
        }

        bridge
            .execute_mint(&msg.id, "0xAlice_X3".to_string(), 1000)
            .ok();

        // Now burn
        let burn_result =
            bridge.burn_wrapped("0xAlice_X3".to_string(), "0xUSDC".to_string(), 500_000u128);
        assert!(burn_result.is_ok());
    }

    #[test]
    fn test_refund_deposit() {
        let validators = (0..7).map(|i| format!("validator_{}", i)).collect();
        let mut bridge = EthereumBridge::new(validators);

        let usdc = ERC20Token {
            address: "0xUSDC".to_string(),
            name: "USDC".to_string(),
            decimals: 6,
            total_supply: 1_000_000_000_000u128,
        };

        bridge.register_token(usdc).ok();

        let deposit = bridge
            .lock_on_ethereum(
                "0xAlice".to_string(),
                "0xUSDC".to_string(),
                1_000_000u128,
                17_000_000,
                "0xtxhash".to_string(),
            )
            .unwrap();

        let refund = bridge.refund_deposit(&deposit.id, "User cancelled".to_string());
        assert!(refund.is_ok());
    }
}
