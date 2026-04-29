//! X3 Chain node RPC module wiring.
//!
//! Assembles the full JSON-RPC module used by the service.
//! Merges substrate system RPCs, transaction-payment RPCs, and the
//! Frontier-compatible ETH/SVM RPC provided by `rpc_frontier`.

use flash_finality::FlashFinalityGadget;
use jsonrpsee::{core::Error as JsonRpseeError, RpcModule};
use sc_client_api::BlockBackend;
use sc_rpc_api::DenyUnsafe;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use std::sync::{Arc, Mutex};
use x3_atomic_trade::{AMMPool, SwapRPCServer};
use x3_chain_runtime::{opaque::Block, AccountId, AssetId, Balance};
use x3_rpc::{SwapRequest, WalletDexApi, WalletDexRpc};

use crate::rpc_middleware::RateLimiter;
use crate::service::FullClient;

type RpcError = Box<dyn std::error::Error + Send + Sync>;

/// Helper to create custom JSON-RPC errors.
fn custom_error(message: impl Into<String>) -> JsonRpseeError {
    JsonRpseeError::Custom(message.into())
}

/// Decode hex string with "0x" prefix to 32-byte array.
fn decode_hex_32(value: &str, label: &str) -> Result<[u8; 32], JsonRpseeError> {
    let stripped = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(stripped)
        .map_err(|e| custom_error(format!("{label} decode failed: {e}")))?;
    if bytes.len() != 32 {
        return Err(custom_error(format!(
            "{label} must be 32 bytes, got {}",
            bytes.len()
        )));
    }
    let mut array = [0u8; 32];
    array.copy_from_slice(&bytes);
    Ok(array)
}

/// Parse u128 value from JSON.
fn parse_u128_value(
    value: Option<&serde_json::Value>,
    label: &str,
) -> Result<u128, JsonRpseeError> {
    let val = value.ok_or_else(|| custom_error(format!("Missing {label}")))?;
    if let Some(s) = val.as_str() {
        s.parse::<u128>()
            .map_err(|e| custom_error(format!("{label} parse failed: {e}")))
    } else if let Some(n) = val.as_u64() {
        Ok(n as u128)
    } else {
        Err(custom_error(format!("{label} must be string or number")))
    }
}

/// Full RPC extension creation.
///
/// Called by the service to build the RPC module for each connection.
pub fn create_full<P>(
    client: Arc<FullClient>,
    pool: Arc<P>,
    deny_unsafe: DenyUnsafe,
    _gadget: Option<Arc<FlashFinalityGadget>>,
    _limiter: Arc<RateLimiter>,
) -> Result<RpcModule<()>, RpcError>
where
    P: TransactionPool + Sync + Send + 'static,
    FullClient: ProvideRuntimeApi<Block>,
    FullClient: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
    FullClient: BlockBackend<Block>,
    <FullClient as ProvideRuntimeApi<Block>>::Api: BlockBuilder<Block>,
    <FullClient as ProvideRuntimeApi<Block>>::Api:
        substrate_frame_rpc_system::AccountNonceApi<Block, x3_chain_runtime::AccountId, u32>,
    <FullClient as ProvideRuntimeApi<Block>>::Api:
        pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<
            Block,
            x3_chain_runtime::Balance,
        >,
    <FullClient as ProvideRuntimeApi<Block>>::Api:
        pallet_x3_kernel::AtlasKernelRuntimeApi<Block, AccountId, Balance, AssetId>,
{
    let mut module = RpcModule::new(());

    let system_rpc = substrate_frame_rpc_system::System::new(client.clone(), pool, deny_unsafe);
    module.merge(substrate_frame_rpc_system::SystemApiServer::into_rpc(
        system_rpc,
    ))?;

    let tx_payment_rpc = pallet_transaction_payment_rpc::TransactionPayment::new(client.clone());
    module.merge(
        pallet_transaction_payment_rpc::TransactionPaymentApiServer::into_rpc(tx_payment_rpc),
    )?;

    // Merge Frontier ETH-compatible JSON-RPC endpoints.
    let frontier_module = crate::rpc_frontier::create_frontier_rpc(client.clone())?;
    module.merge(frontier_module)?;

    // Merge SVM-compatible JSON-RPC endpoints.
    let svm_module = crate::rpc_frontier::create_svm_rpc(client.clone())?;
    module.merge(svm_module)?;

    // Initialize DEX RPC integration.
    let wallet_dex = Arc::new(WalletDexRpc::<Block, FullClient>::new(client.clone()));
    let swap_rpc = Arc::new(Mutex::new(SwapRPCServer::new()));

    // Register default AMM pool (X3/USDC).
    {
        let mut engine = swap_rpc
            .lock()
            .map_err(|_| custom_error("Swap engine lock poisoned"))?;

        let _ = engine.register_pool(AMMPool {
            id: "default_x3_usdc".to_string(),
            token_a: "X3".to_string(),
            token_b: "USDC".to_string(),
            reserve_a: 10_000_000_000_000,
            reserve_b: 10_000_000_000_000,
            fee_bps: 30,
            tvl_usd: 20_000_000.0,
        });
    }

    // Register walletDex_estimateSwap RPC method.
    let wallet_dex_estimate = wallet_dex.clone();
    module.register_method("walletDex_estimateSwap", move |params, _| {
        let req: serde_json::Value = params.one()?;
        let request = SwapRequest {
            token_in: decode_hex_32(
                req.get("token_in")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| custom_error("Missing token_in"))?,
                "token_in",
            )?,
            token_out: decode_hex_32(
                req.get("token_out")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| custom_error("Missing token_out"))?,
                "token_out",
            )?,
            amount_in: parse_u128_value(req.get("amount_in"), "amount_in")?,
            min_amount_out: parse_u128_value(req.get("min_amount_out"), "min_amount_out")?,
            wallet_id: decode_hex_32(
                req.get("wallet_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| custom_error("Missing wallet_id"))?,
                "wallet_id",
            )?,
            require_approval: req
                .get("require_approval")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            approval_threshold: parse_u128_value(
                req.get("approval_threshold"),
                "approval_threshold",
            )?,
        };

        wallet_dex_estimate
            .estimate_swap(request)
            .map_err(|e| custom_error(format!("walletDex_estimateSwap failed: {e}")))
    })?;

    // Register walletDex_executeSwap RPC method.
    let wallet_dex_execute = wallet_dex.clone();
    module.register_method("walletDex_executeSwap", move |params, _| {
        let req: serde_json::Value = params.one()?;
        let request = SwapRequest {
            token_in: decode_hex_32(
                req.get("token_in")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| custom_error("Missing token_in"))?,
                "token_in",
            )?,
            token_out: decode_hex_32(
                req.get("token_out")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| custom_error("Missing token_out"))?,
                "token_out",
            )?,
            amount_in: parse_u128_value(req.get("amount_in"), "amount_in")?,
            min_amount_out: parse_u128_value(req.get("min_amount_out"), "min_amount_out")?,
            wallet_id: decode_hex_32(
                req.get("wallet_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| custom_error("Missing wallet_id"))?,
                "wallet_id",
            )?,
            require_approval: req
                .get("require_approval")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            approval_threshold: parse_u128_value(
                req.get("approval_threshold"),
                "approval_threshold",
            )?,
        };

        wallet_dex_execute
            .execute_swap(request)
            .map_err(|e| custom_error(format!("walletDex_executeSwap failed: {e}")))
    })?;

    Ok(module)
}
