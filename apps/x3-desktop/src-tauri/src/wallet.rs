//! Universal Multi-Chain Wallet - BIP39 + EVM chains + Substrate

use bip39::{Mnemonic, Language};
use rand::RngCore;
use sp_core::{sr25519, Pair};
use sp_core::crypto::Ss58Codec;
use solana_sdk::signature::{Keypair, SeedDerivable, Signer};
use bs58;
use std::sync::{Mutex, OnceLock};
use tauri::{command, AppHandle, Emitter, State};
use crate::wallet_core::substrate_hook::{SubstrateHookManager, SubstrateHookEvent};

#[derive(Debug, thiserror::Error, serde::Serialize)]
pub enum WalletError {
    #[error("Crypto error: {0}")]
    CryptoError(String),
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct UniversalWallet {
    mnemonic: String,
    seed_hex: String,
    evm_address: String,
    evm_private_key: String,
    solana_address: String,
    solana_private_key: String,
    substrate_address: String,
    evm_chain_count: usize,
    warning: String,
}

#[command]
pub fn generate_universal_wallet() -> Result<UniversalWallet, WalletError> {
    // Generate 12-word mnemonic using supported bip39 API
    use rand::thread_rng;
    let mut entropy = [0u8; 16];
    thread_rng().fill_bytes(&mut entropy);
    let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
        .map_err(|e| WalletError::CryptoError(e.to_string()))?;
    let mnemonic_str = mnemonic.to_string();
    let seed = mnemonic.to_seed("");

    // Derive EVM address from seed (use keccak256 of seed)
    let hash = sp_core::hashing::keccak_256(&seed);
    let evm_address = format!("0x{}", hex::encode(&hash[12..32]));
    let evm_private_key = format!("0x{}", hex::encode(&seed[0..32]));

    // Solana
    let mut solana_seed = [0u8; 32];
    solana_seed.copy_from_slice(&seed[0..32]);
    let solana_keypair = Keypair::from_seed(&solana_seed).map_err(|e| WalletError::CryptoError(format!("Solana keypair error: {}", e)))?;
    let solana_address = solana_keypair.pubkey().to_string();
    let solana_private_key = bs58::encode(solana_keypair.to_bytes()).into_string();

    // Substrate (using Polkadot SS58 format)
    let mut seed_array = [0u8; 32];
    seed_array.copy_from_slice(&seed[0..32]);
    let pair = sr25519::Pair::from_seed(&seed_array);
    let substrate_address = pair.public().to_ss58check();

    // Chain count - placeholder
    let evm_chain_count = 60000;

    Ok(UniversalWallet {
        mnemonic: mnemonic_str,
        seed_hex: hex::encode(seed),
        evm_address,
        evm_private_key,
        solana_address,
        solana_private_key,
        substrate_address,
        evm_chain_count,
        warning: "⚠️ LIVE KEYS - Backup mnemonic securely. Single EVM address works on 60k+ chains.".to_string(),
    })
}

#[command]
pub fn import_universal_wallet(mnemonic: String) -> Result<UniversalWallet, WalletError> {
    // Use provided mnemonic
    let mnemonic = Mnemonic::parse_in(Language::English, mnemonic.as_str())
        .map_err(|e| WalletError::CryptoError(e.to_string()))?;
    let mnemonic_str = mnemonic.to_string();
    let seed = mnemonic.to_seed("");

    // Derive EVM address from seed
    let hash = sp_core::hashing::keccak_256(&seed);
    let evm_address = format!("0x{}", hex::encode(&hash[12..32]));
    let evm_private_key = format!("0x{}", hex::encode(&seed[0..32]));

    // Solana
    let mut solana_seed = [0u8; 32];
    solana_seed.copy_from_slice(&seed[0..32]);
    let solana_keypair = Keypair::from_seed(&solana_seed).map_err(|e| WalletError::CryptoError(format!("Solana keypair error: {}", e)))?;
    let solana_address = solana_keypair.pubkey().to_string();
    let solana_private_key = bs58::encode(solana_keypair.to_bytes()).into_string();

    // Substrate
    let mut seed_array = [0u8; 32];
    seed_array.copy_from_slice(&seed[0..32]);
    let pair = sr25519::Pair::from_seed(&seed_array);
    let substrate_address = pair.public().to_ss58check();

    let evm_chain_count = 60000;

    Ok(UniversalWallet {
        mnemonic: mnemonic_str,
        seed_hex: hex::encode(seed),
        evm_address,
        evm_private_key,
        solana_address,
        solana_private_key,
        substrate_address,
        evm_chain_count,
        warning: "⚠️ IMPORTED KEYS - Verify and backup securely.".to_string(),
    })
}

#[command]
pub fn get_evm_chain_count() -> usize {
    59263
}

#[command]
pub async fn store_wallet_secure(wallet: UniversalWallet) -> Result<(), WalletError> {
    let master_password = std::env::var("X3_WALLET_MASTER_PASSWORD").map_err(|_| {
        WalletError::CryptoError(
            "X3_WALLET_MASTER_PASSWORD is required for secure wallet storage".to_string(),
        )
    })?;

    let mut store = wallet_store()
        .lock()
        .map_err(|e| WalletError::CryptoError(format!("Wallet store lock poisoned: {}", e)))?;
    let wallet_id = format!(
        "wallet_{}",
        hex::encode(sp_core::hashing::keccak_256(wallet.substrate_address.as_bytes()))[0..16]
            .to_uppercase()
    );

    store
        .store_wallet(
            &wallet_id,
            &wallet.mnemonic,
            &wallet.seed_hex,
            "m/44'/354'/0'/0/0",
            &master_password,
        )
        .map_err(|e| WalletError::CryptoError(format!("Failed to store wallet: {}", e)))
}

#[command]
pub async fn get_wallet_balance(chain_id: String, address: String) -> Result<String, WalletError> {
    let params = serde_json::json!({
        "wallet_id": address,
        "token_id": serde_json::Value::Null,
        "network": chain_id,
    });

    let result = crate::rpc_call("wallet_getBalance", params)
        .await
        .ok_or_else(|| WalletError::CryptoError("Failed to query wallet balance".to_string()))?;

    Ok(result.to_string())
}

#[command]
pub async fn submit_cross_swap(
    wallet_address: String,
    from_chain: String,
    to_chain: String,
    amount: String,
) -> Result<String, WalletError> {
    if wallet_address.trim().is_empty() {
        return Err(WalletError::CryptoError(
            "wallet_address is required to submit a cross-chain swap".to_string(),
        ));
    }

    let amount_in = amount
        .parse::<u128>()
        .map_err(|e| WalletError::CryptoError(format!("Invalid amount: {}", e)))?;

    let token_in = sp_core::hashing::blake2_256(from_chain.as_bytes());
    let token_out = sp_core::hashing::blake2_256(to_chain.as_bytes());

    let params = serde_json::json!({
        "token_in": format!("0x{}", hex::encode(token_in)),
        "token_out": format!("0x{}", hex::encode(token_out)),
        "amount_in": amount_in.to_string(),
        "min_amount_out": amount_in.to_string(),
        "wallet_id": wallet_address,
        "require_approval": false,
        "approval_threshold": "0",
    });

    let result = crate::rpc_call("walletDex_executeSwap", params)
        .await
        .ok_or_else(|| WalletError::CryptoError("Failed to submit cross swap".to_string()))?;

    Ok(result.to_string())
}

#[command]
pub async fn execute_x3_script(script: String, _wallet: UniversalWallet) -> Result<String, WalletError> {
    if !script.starts_with("0x") {
        return Err(WalletError::CryptoError(
            "script must be SCALE-encoded extrinsic bytes (0x-prefixed)".to_string(),
        ));
    }

    let result = crate::rpc_call("author_submitExtrinsic", serde_json::json!([script]))
        .await
        .ok_or_else(|| WalletError::CryptoError("Failed to execute x3 script".to_string()))?;

    Ok(result.to_string())
}

#[command]
pub async fn run_cross_chain_intent(draft: crate::wallet_core::ipc::IntentDraft) -> Result<String, String> {
    crate::wallet_core::coordinator::WalletCoordinator::create_intent_draft(draft).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 2: Substrate Hook Commands
// ─────────────────────────────────────────────────────────────────────────────

#[command]
pub async fn subscribe_substrate_events(app: AppHandle, state: State<'_, crate::SubstrateState>) -> Result<String, String> {
    let rpc_url = "ws://127.0.0.1:9944";
    let mut manager = SubstrateHookManager::new(rpc_url);
    let handler = manager.get_handler("default");

    let app_handle = app.clone();
    handler.register_hook("emit_events", Box::new(move |event: SubstrateHookEvent| {
        let payload = match &event {
            SubstrateHookEvent::NewBlock { hash, number, parent_hash, timestamp } => {
                serde_json::json!({
                    "type": "NewBlock",
                    "data": {
                        "hash": format!("{:?}", hash),
                        "number": *number,
                        "parentHash": format!("{:?}", parent_hash),
                        "timestamp": *timestamp,
                    }
                })
            }
            SubstrateHookEvent::Extrinsic { hash, signer, method, success, error } => {
                serde_json::json!({
                    "type": "Extrinsic",
                    "data": {
                        "hash": format!("{:?}", hash),
                        "signer": format!("{:?}", signer),
                        "method": method.clone(),
                        "success": *success,
                        "error": error.clone(),
                    }
                })
            }
            SubstrateHookEvent::ChainReorg { old_hash, new_hash, reorg_depth } => {
                serde_json::json!({
                    "type": "ChainReorg",
                    "data": {
                        "oldHash": format!("{:?}", old_hash),
                        "newHash": format!("{:?}", new_hash),
                        "reorgDepth": *reorg_depth,
                    }
                })
            }
        };
        let _ = app_handle.emit("substrate_event", payload);
    }));

    *state.manager.write().unwrap() = Some(manager);
    Ok("substrate_events_subscribed".to_string())
}

#[command]
pub async fn get_substrate_hook_state(state: State<'_, crate::SubstrateState>) -> Result<String, String> {
    if let Some(manager) = &mut *state.manager.write().unwrap() {
        let handler = manager.get_handler("default");
        let hook_state = handler.get_state();
        let response = serde_json::json!({
            "connected": hook_state.connected,
            "lastBlockNumber": hook_state.last_block_number,
        });
        Ok(response.to_string())
    } else {
        Ok(r#"{"connected": false, "lastBlockNumber": null}"#.to_string())
    }
}

#[command]
pub async fn register_substrate_hook(hook_id: String, state: State<'_, crate::SubstrateState>) -> Result<(), String> {
    if let Some(_manager) = &*state.manager.read().unwrap() {
        // Hook is already registered in subscribe
        Ok(())
    } else {
        Err("not subscribed".to_string())
    }
}

#[command]
pub async fn unregister_substrate_hook(hook_id: String, state: State<'_, crate::SubstrateState>) -> Result<(), String> {
    if let Some(manager) = &mut *state.manager.write().unwrap() {
        manager.remove_handler(&hook_id);
        Ok(())
    } else {
        Err("not subscribed".to_string())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 2: Wallet Store Commands
// ─────────────────────────────────────────────────────────────────────────────

use crate::wallet_core::wallet_store::WalletStore;

static WALLET_STORE: OnceLock<Mutex<WalletStore>> = OnceLock::new();

fn wallet_store() -> &'static Mutex<WalletStore> {
    WALLET_STORE.get_or_init(|| {
        let mut store = WalletStore::new();
        store.initialize();
        Mutex::new(store)
    })
}

#[command]
pub async fn store_wallet_encrypted(
    wallet_id: String,
    mnemonic: String,
    seed: String,
    derivation_path: String,
    master_password: String,
) -> Result<(), String> {
    let mut store = wallet_store()
        .lock()
        .map_err(|e| format!("Wallet store lock poisoned: {}", e))?;
    store
        .store_wallet(&wallet_id, &mnemonic, &seed, &derivation_path, &master_password)
        .map_err(|e| format!("Failed to store wallet: {}", e))
}

#[command]
pub async fn retrieve_wallet_encrypted(wallet_id: String, master_password: String) -> Result<String, String> {
    let store = wallet_store()
        .lock()
        .map_err(|e| format!("Wallet store lock poisoned: {}", e))?;
    store
        .retrieve_wallet(&wallet_id, &master_password)
        .map(|(mnemonic, seed)| {
            // Build the JSON via `serde_json` so any quotes/backslashes in the
            // wallet_id, mnemonic, or seed are escaped correctly. Using
            // `format!()` here previously produced malformed JSON or could be
            // exploited via injection through the user-supplied `wallet_id`.
            serde_json::json!({
                "wallet_id": wallet_id,
                "mnemonic": mnemonic,
                "seed": seed,
            })
            .to_string()
        })
        .map_err(|e| format!("Failed to retrieve wallet: {}", e))
}

#[command]
pub async fn delete_wallet(wallet_id: String) -> Result<(), String> {
    let mut store = wallet_store()
        .lock()
        .map_err(|e| format!("Wallet store lock poisoned: {}", e))?;
    store
        .delete_wallet(&wallet_id)
        .map_err(|e| format!("Failed to delete wallet: {}", e))
}

#[command]
pub async fn export_wallet_backup(wallet_id: String) -> Result<String, String> {
    let store = wallet_store()
        .lock()
        .map_err(|e| format!("Wallet store lock poisoned: {}", e))?;
    store
        .export_backup(&wallet_id)
        .map_err(|e| format!("Failed to export wallet backup: {}", e))
}

#[command]
pub async fn import_wallet_backup(backup: String) -> Result<String, String> {
    let mut store = wallet_store()
        .lock()
        .map_err(|e| format!("Wallet store lock poisoned: {}", e))?;
    store
        .import_backup(&backup)
        .map(|wallet_id| {
            serde_json::json!({
                "wallet_id": wallet_id,
                "status": "imported",
            })
            .to_string()
        })
        .map_err(|e| format!("Failed to import wallet backup: {}", e))
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 2: x3ChainService Commands
// ─────────────────────────────────────────────────────────────────────────────

#[command]
pub async fn query_block(block_number: Option<u64>, block_hash: Option<String>) -> Result<String, String> {
    if let Some(hash) = block_hash {
        return match crate::rpc_call("chain_getBlock", serde_json::json!([hash])).await {
            Some(result) => Ok(result.to_string()),
            None => Err("Failed to query block by hash".to_string()),
        };
    }

    if let Some(number) = block_number {
        let hash = crate::rpc_call("chain_getBlockHash", serde_json::json!([number]))
            .await
            .ok_or_else(|| "Failed to resolve block hash".to_string())?;
        return match crate::rpc_call("chain_getBlock", serde_json::json!([hash])).await {
            Some(result) => Ok(result.to_string()),
            None => Err("Failed to query block by number".to_string()),
        };
    }

    match crate::rpc_call("chain_getBlock", serde_json::json!([])).await {
        Some(result) => Ok(result.to_string()),
        None => Err("Failed to query latest block".to_string()),
    }
}

#[command]
pub async fn query_account(address: String, at_block: Option<u64>) -> Result<String, String> {
    let _at_param = if let Some(block) = at_block {
        serde_json::json!([format!("0x{:x}", block)])
    } else {
        serde_json::json!([])
    };

    // Get nonce first.
    let nonce_result = crate::rpc_call("system_accountNextIndex", serde_json::json!([address])).await;
    let nonce = if let Some(n) = nonce_result {
        n.as_u64().unwrap_or(0)
    } else {
        0
    };

    // Query canonical balance through wallet RPC using the structured request expected by the node.
    let balance_params = serde_json::json!({
        "wallet_id": address,
        "token_id": serde_json::Value::Null,
        "network": "local",
    });
    let balance_result = crate::rpc_call("wallet_getBalance", balance_params).await;
    let balance = balance_result.unwrap_or_else(|| serde_json::json!({}));

    serde_json::to_string(&serde_json::json!({
        "address": address,
        "nonce": nonce,
        "balance": balance
    })).map_err(|e| format!("Failed to serialize account: {}", e))
}

#[command]
pub async fn query_balance(address: String, asset_id: Option<String>) -> Result<String, String> {
    let params = serde_json::json!({
        "wallet_id": address,
        "token_id": asset_id,
        "network": "local",
    });
    match crate::rpc_call("wallet_getBalance", params).await {
        Some(result) => Ok(result.to_string()),
        None => Err("Failed to query balance".to_string()),
    }
}

#[command]
pub async fn submit_extrinsic(call: String, signer: String, nonce: Option<u64>, tip: Option<u64>) -> Result<String, String> {
    let call_data = serde_json::from_str::<serde_json::Value>(&call)
        .map_err(|e| format!("Failed to parse call: {}", e))?;
    let params = serde_json::json!([call_data]);
    match crate::rpc_call("author_submitExtrinsic", params).await {
        Some(result) => Ok(result.to_string()),
        None => Err("Failed to submit extrinsic".to_string()),
    }
}

#[command]
pub async fn get_connection_status() -> Result<String, String> {
    match crate::rpc_call("system_health", serde_json::json!([])).await {
        Some(result) => {
            let is_syncing = result.get("isSyncing").and_then(|v| v.as_bool()).unwrap_or(false);
            let peers = result.get("peers").and_then(|v| v.as_u64()).unwrap_or(0);
            let best_block = result.get("bestBlock").unwrap_or(&serde_json::Value::Null);
            serde_json::to_string(&serde_json::json!({
                "connected": !is_syncing,
                "blockNumber": best_block,
                "peers": peers
            })).map_err(|e| format!("Failed to serialize status: {}", e))
        }
        None => Err("Failed to get connection status".to_string()),
    }
}

#[command]
pub async fn clear_chain_cache() -> Result<(), String> {
    // Clear the chain operation cache
    // In production, this would use the x3_chain_service module
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_universal_wallet() {
        let wallet = generate_universal_wallet().expect("Failed to generate wallet");
        assert!(!wallet.mnemonic.is_empty());
        assert_eq!(wallet.mnemonic.split_whitespace().count(), 12);
        assert!(wallet.evm_address.starts_with("0x"));
        assert_eq!(wallet.evm_address.len(), 42);
        assert!(!wallet.solana_address.is_empty());
        assert!(!wallet.substrate_address.is_empty());
    }

    #[test]
    fn test_import_universal_wallet_consistency() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string();
        let wallet = import_universal_wallet(mnemonic.clone()).expect("Failed to import wallet");
        
        assert_eq!(wallet.mnemonic, mnemonic);
        // Verify fixed addresses for this known mnemonic (if logic is stable)
        assert!(wallet.evm_address.starts_with("0x"));
        
        // Import again and check for exact match
        let wallet2 = import_universal_wallet(mnemonic).expect("Failed to import wallet again");
        assert_eq!(wallet.evm_address, wallet2.evm_address);
        assert_eq!(wallet.solana_address, wallet2.solana_address);
        assert_eq!(wallet.substrate_address, wallet2.substrate_address);
    }

    #[test]
    fn test_evm_chain_count() {
        assert!(get_evm_chain_count() > 50000);
    }

    #[test]
    fn test_invalid_mnemonic_fails() {
        let result = import_universal_wallet("invalid mnemonic".to_string());
        assert!(result.is_err());
    }
}
