//! Frontier RPC wiring stub
//!
//! This module provides optional wiring for Frontier JSON-RPC endpoints.
//! When `feature = "frontier"` is enabled for the node crate, this module
//! will create and merge additional Ethereum-compatible RPC handlers. These
//! should be replaced (or extended) with the `fc-rpc`/`fp-rpc` modules once
//! the Frontier version compatibility is resolved.

use hex;
use jsonrpsee::RpcModule;
use pallet_x3_kernel::AtlasKernelRuntimeApi;
use sc_client_api::BlockBackend;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use std::sync::Arc;
use x3_chain_runtime::{opaque::Block, AccountId, AssetId, Balance};

/// Helper: decode a hex EVM address string to a 20-byte Vec
fn decode_address(s: &str) -> Result<Vec<u8>, jsonrpsee::core::Error> {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(stripped)
        .map_err(|e| jsonrpsee::core::Error::Custom(format!("Invalid address: {}", e)))?;
    if bytes.len() != 20 {
        return Err(jsonrpsee::core::Error::Custom(
            "Address must be 20 bytes".into(),
        ));
    }
    Ok(bytes)
}

fn parse_gas_limit(tx_obj: &serde_json::Value) -> Result<u64, jsonrpsee::core::Error> {
    let Some(raw_gas) = tx_obj.get("gas") else {
        return Ok(10_000_000);
    };

    if let Some(gas_u64) = raw_gas.as_u64() {
        return Ok(gas_u64);
    }

    if let Some(gas_str) = raw_gas.as_str() {
        if let Some(stripped) = gas_str.strip_prefix("0x") {
            return u64::from_str_radix(stripped, 16)
                .map_err(|e| jsonrpsee::core::Error::Custom(format!("Invalid gas: {}", e)));
        }

        return gas_str
            .parse::<u64>()
            .map_err(|e| jsonrpsee::core::Error::Custom(format!("Invalid gas: {}", e)));
    }

    Err(jsonrpsee::core::Error::Custom(
        "Invalid gas value: expected integer or string".into(),
    ))
}

/// Create a Frontier-compatible JSON-RPC module backed by runtime API calls.
/// Provides eth_getBalance, eth_getCode, eth_getStorageAt,
/// eth_getTransactionCount (nonce), eth_call, and eth_estimateGas.
pub fn create_frontier_stub<C>(
    client: Arc<C>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
    C: Send
        + Sync
        + 'static
        + ProvideRuntimeApi<Block>
        + HeaderBackend<Block>
        + BlockBackend<Block>,
    C::Api: pallet_x3_kernel::AtlasKernelRuntimeApi<Block, AccountId, Balance, AssetId>,
{
    let mut module = RpcModule::new(());

    // eth_getBalance — returns native balance for an EVM address as hex wei
    let c = client.clone();
    module.register_method("eth_getBalance", move |params, _| {
        let (address_hex, _block): (String, serde_json::Value) =
            params.parse().unwrap_or_else(|_| {
                let s: String = params.one().unwrap_or_default();
                (s, serde_json::Value::Null)
            });
        let bytes = decode_address(&address_hex)?;
        let api = c.runtime_api();
        let at = c.info().best_hash;
        let balance: Balance = api
            .get_evm_balance(at, bytes, 0u32)
            .map_err(|e| jsonrpsee::core::Error::Custom(format!("Runtime error: {:?}", e)))?
            .unwrap_or_default();
        Ok(format!("0x{:x}", balance))
    })?;

    // eth_getCode — returns contract bytecode for an EVM address as 0x-prefixed hex
    let c = client.clone();
    module.register_method("eth_getCode", move |params, _| {
        let (address_hex, _block): (String, serde_json::Value) =
            params.parse().unwrap_or_else(|_| {
                let s: String = params.one().unwrap_or_default();
                (s, serde_json::Value::Null)
            });
        let bytes = decode_address(&address_hex)?;
        let api = c.runtime_api();
        let at = c.info().best_hash;
        let code: Vec<u8> = api
            .get_evm_code(at, bytes)
            .map_err(|e| jsonrpsee::core::Error::Custom(format!("Runtime error: {:?}", e)))?;
        Ok(format!("0x{}", hex::encode(code)))
    })?;

    // eth_getStorageAt — returns EVM storage value at (address, slot) as 0x-prefixed hex
    let c = client.clone();
    module.register_method("eth_getStorageAt", move |params, _| {
        let (address_hex, slot_hex, _block): (String, String, serde_json::Value) =
            params
                .parse()
                .map_err(|e| jsonrpsee::core::Error::Custom(e.to_string()))?;
        let addr_bytes = decode_address(&address_hex)?;
        let slot_stripped = slot_hex.strip_prefix("0x").unwrap_or(&slot_hex);
        let slot_bytes = hex::decode(slot_stripped)
            .map_err(|e| jsonrpsee::core::Error::Custom(format!("Invalid slot: {}", e)))?;
        if slot_bytes.len() > 32 {
            return Err(jsonrpsee::core::Error::Custom(
                "Slot must be ≤32 bytes".into(),
            ));
        }
        let mut key = [0u8; 32];
        let offset = 32 - slot_bytes.len();
        key[offset..].copy_from_slice(&slot_bytes);
        let storage_key = sp_core::H256::from(key);
        let api = c.runtime_api();
        let at = c.info().best_hash;
        let val: Option<sp_core::H256> = api
            .get_evm_storage(at, addr_bytes, storage_key)
            .map_err(|e| jsonrpsee::core::Error::Custom(format!("Runtime error: {:?}", e)))?;
        Ok(format!(
            "0x{}",
            hex::encode(val.unwrap_or_default().as_bytes())
        ))
    })?;

    // eth_getTransactionCount — returns account nonce as hex
    let c = client.clone();
    module.register_method("eth_getTransactionCount", move |params, _| {
        let (address_hex, _block): (String, serde_json::Value) =
            params.parse().unwrap_or_else(|_| {
                let s: String = params.one().unwrap_or_default();
                (s, serde_json::Value::Null)
            });
        let bytes = decode_address(&address_hex)?;
        let api = c.runtime_api();
        let at = c.info().best_hash;
        let nonce: u64 = api
            .get_evm_nonce(at, bytes)
            .map_err(|e| jsonrpsee::core::Error::Custom(format!("Runtime error: {:?}", e)))?;
        Ok(format!("0x{:x}", nonce))
    })?;

    // eth_call — execute a read-only EVM call and return raw output bytes.
    let c = client.clone();
    module.register_method("eth_call", move |params, _| {
        let (tx_obj, _block): (serde_json::Value, serde_json::Value) =
            params.parse().unwrap_or_else(|_| {
                let tx: serde_json::Value = params
                    .one()
                    .unwrap_or(serde_json::Value::Object(Default::default()));
                (tx, serde_json::Value::Null)
            });

        let target = tx_obj
            .get("to")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpsee::core::Error::Custom("Missing to address".into()))?;
        let target_bytes = decode_address(target)?;
        let caller = tx_obj
            .get("from")
            .and_then(|v| v.as_str())
            .map(decode_address)
            .transpose()?;

        let data_hex = tx_obj.get("data").and_then(|v| v.as_str()).unwrap_or("0x");
        let data_stripped = data_hex.strip_prefix("0x").unwrap_or(data_hex);
        let input_data = hex::decode(data_stripped)
            .map_err(|e| jsonrpsee::core::Error::Custom(format!("Invalid data: {}", e)))?;
        let gas_limit = parse_gas_limit(&tx_obj)?;

        let api = c.runtime_api();
        let at = c.info().best_hash;
        let output = api
            .call_evm(at, caller, target_bytes, input_data, gas_limit)
            .map_err(|e| jsonrpsee::core::Error::Custom(format!("Runtime error: {:?}", e)))?;

        match output {
            Ok(return_data) => Ok(format!("0x{}", hex::encode(return_data))),
            Err(error_bytes) => Err(jsonrpsee::core::Error::Custom(format!(
                "EVM call failed: {}",
                String::from_utf8_lossy(&error_bytes)
            ))),
        }
    })?;

    // eth_estimateGas — estimate gas using runtime EVM dry-run logic.
    let c = client.clone();
    module.register_method("eth_estimateGas", move |params, _| {
        let tx_obj: serde_json::Value = params
            .one()
            .map_err(|e| jsonrpsee::core::Error::Custom(format!("Invalid params: {}", e)))?;

        let target_bytes = if let Some(target) = tx_obj.get("to").and_then(|v| v.as_str()) {
            decode_address(target)?
        } else {
            vec![0u8; 20]
        };
        let caller = tx_obj
            .get("from")
            .and_then(|v| v.as_str())
            .map(decode_address)
            .transpose()?;

        let data_hex = tx_obj.get("data").and_then(|v| v.as_str()).unwrap_or("0x");
        let data_stripped = data_hex.strip_prefix("0x").unwrap_or(data_hex);
        let input_data = hex::decode(data_stripped)
            .map_err(|e| jsonrpsee::core::Error::Custom(format!("Invalid data: {}", e)))?;
        let gas_limit = parse_gas_limit(&tx_obj)?;

        let api = c.runtime_api();
        let at = c.info().best_hash;
        let estimate = api
            .estimate_evm_gas(at, caller, target_bytes, input_data, gas_limit)
            .map_err(|e| jsonrpsee::core::Error::Custom(format!("Runtime error: {:?}", e)))?;

        match estimate {
            Ok(gas) => Ok(format!("0x{:x}", gas)),
            Err(error_bytes) => Err(jsonrpsee::core::Error::Custom(format!(
                "Gas estimation failed: {}",
                String::from_utf8_lossy(&error_bytes)
            ))),
        }
    })?;

    // eth_sendRawTransaction — submit a signed RLP-encoded Ethereum transaction
    // Executes via the X3 kernel EVM adapter and returns the keccak256 tx hash.
    let c = client.clone();
    module.register_method("eth_sendRawTransaction", move |params, _| {
        let raw_hex: String = params.one()?;
        let stripped = raw_hex.strip_prefix("0x").unwrap_or(&raw_hex);
        let raw_bytes = hex::decode(stripped)
            .map_err(|e| jsonrpsee::core::Error::Custom(format!("Invalid hex: {}", e)))?;
        let api = c.runtime_api();
        let at = c.info().best_hash;
        let result: Result<Vec<u8>, Vec<u8>> = api
            .submit_evm_transaction(at, raw_bytes)
            .map_err(|e| jsonrpsee::core::Error::Custom(format!("Runtime error: {:?}", e)))?;
        match result {
            Ok(tx_hash) => Ok(format!("0x{}", hex::encode(tx_hash))),
            Err(err_bytes) => Err(jsonrpsee::core::Error::Custom(format!(
                "EVM execution failed: {}",
                String::from_utf8_lossy(&err_bytes)
            ))),
        }
    })?;

    Ok(module)
}

/// Create an SVM-compatible JSON-RPC module backed by runtime API calls.
/// Provides svm_getBalance and svm_isProgram endpoints for querying SVM state.
pub fn create_svm_stub<C>(
    client: Arc<C>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
    C: Send
        + Sync
        + 'static
        + ProvideRuntimeApi<Block>
        + HeaderBackend<Block>
        + BlockBackend<Block>,
    C::Api: pallet_x3_kernel::AtlasKernelRuntimeApi<Block, AccountId, Balance, AssetId>,
{
    let mut module = RpcModule::new(());

    // svm_getBalance — returns lamport balance for a base58 or hex SVM pubkey
    let c = client.clone();
    module.register_method("svm_getBalance", move |params, _| {
        let pubkey_str: String = params.one()?;
        let bytes = decode_svm_pubkey(&pubkey_str)?;
        let api = c.runtime_api();
        let at = c.info().best_hash;
        let balance: u64 = api
            .get_svm_balance(at, bytes)
            .map_err(|e| jsonrpsee::core::Error::Custom(format!("Runtime error: {:?}", e)))?;
        Ok(serde_json::json!({ "value": balance }))
    })?;

    // svm_isProgram — returns whether a pubkey has a deployed executable program
    let c = client.clone();
    module.register_method("svm_isProgram", move |params, _| {
        let pubkey_str: String = params.one()?;
        let bytes = decode_svm_pubkey(&pubkey_str)?;
        let api = c.runtime_api();
        let at = c.info().best_hash;
        let is_prog: bool = api
            .is_svm_program(at, bytes)
            .map_err(|e| jsonrpsee::core::Error::Custom(format!("Runtime error: {:?}", e)))?;
        Ok(serde_json::json!({ "result": is_prog }))
    })?;

    Ok(module)
}

/// Decode a SVM pubkey from either a 0x-prefixed hex string (32 bytes) or
/// a base58-encoded Solana-style pubkey.
fn decode_svm_pubkey(s: &str) -> Result<Vec<u8>, jsonrpsee::core::Error> {
    if let Some(hex_str) = s.strip_prefix("0x") {
        let bytes = hex::decode(hex_str)
            .map_err(|e| jsonrpsee::core::Error::Custom(format!("Invalid hex pubkey: {}", e)))?;
        if bytes.len() != 32 {
            return Err(jsonrpsee::core::Error::Custom(
                "SVM pubkey must be 32 bytes".into(),
            ));
        }
        return Ok(bytes);
    }
    // base58 decode
    let bytes = bs58::decode(s)
        .into_vec()
        .map_err(|e| jsonrpsee::core::Error::Custom(format!("Invalid base58 pubkey: {}", e)))?;
    if bytes.len() != 32 {
        return Err(jsonrpsee::core::Error::Custom(
            "SVM pubkey must be 32 bytes".into(),
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::{decode_address, parse_gas_limit};

    #[test]
    fn decode_address_accepts_20_byte_hex() {
        let addr = format!("0x{}", "11".repeat(20));
        let decoded = decode_address(&addr).expect("address should decode");
        assert_eq!(decoded.len(), 20);
        assert!(decoded.iter().all(|b| *b == 0x11));
    }

    #[test]
    fn decode_address_rejects_wrong_length() {
        let addr = format!("0x{}", "aa".repeat(19));
        let err = decode_address(&addr).expect_err("address must be rejected");
        let text = format!("{err:?}");
        assert!(text.contains("Address must be 20 bytes"));
    }

    #[test]
    fn parse_gas_limit_accepts_hex_string() {
        let tx = serde_json::json!({ "gas": "0x5208" });
        let gas = parse_gas_limit(&tx).expect("hex gas should parse");
        assert_eq!(gas, 21_000);
    }

    #[test]
    fn parse_gas_limit_accepts_numeric_value() {
        let tx = serde_json::json!({ "gas": 42000 });
        let gas = parse_gas_limit(&tx).expect("numeric gas should parse");
        assert_eq!(gas, 42_000);
    }

    #[test]
    fn parse_gas_limit_accepts_decimal_string() {
        let tx = serde_json::json!({ "gas": "42000" });
        let gas = parse_gas_limit(&tx).expect("decimal string gas should parse");
        assert_eq!(gas, 42_000);
    }

    #[test]
    fn parse_gas_limit_rejects_invalid_type() {
        let tx = serde_json::json!({ "gas": { "value": 1 } });
        let err = parse_gas_limit(&tx).expect_err("object gas value must be rejected");
        let text = format!("{err:?}");
        assert!(text.contains("Invalid gas value"));
    }

    #[test]
    fn parse_gas_limit_uses_default_when_missing() {
        let tx = serde_json::json!({});
        let gas = parse_gas_limit(&tx).expect("default gas should be used");
        assert_eq!(gas, 10_000_000);
    }
}
