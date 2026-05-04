//! RPC wiring for X3 Chain.

use std::sync::Arc;
use std::sync::Mutex;

use flash_finality::FlashFinalityGadget;
use jsonrpsee::core::Error as JsonRpseeError;
use jsonrpsee::RpcModule;
use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
use pallet_x3_kernel::AtlasKernelRuntimeApi;
use parity_scale_codec::{Decode, Encode};
use sc_client_api::BlockBackend;
pub use sc_rpc_api::DenyUnsafe;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use substrate_frame_rpc_system::{System, SystemApiServer};
use x3_atomic_trade::{AMMPool, SwapRPCServer, TokenPair};
use x3_chain_runtime::{opaque::Block, AccountId, AssetId, Balance, Nonce};
use x3_rpc::{SwapRequest, WalletDexApi, WalletDexRpc};

use crate::rpc_middleware::RateLimiter;

fn custom_error(message: impl Into<String>) -> JsonRpseeError {
    JsonRpseeError::Custom(message.into())
}

fn decode_hex_param(value: &str, label: &str) -> Result<Vec<u8>, JsonRpseeError> {
    let stripped = value.strip_prefix("0x").unwrap_or(value);
    hex::decode(stripped).map_err(|e| custom_error(format!("Invalid {label}: {e}")))
}

fn decode_account_id(value: &str) -> Result<AccountId, JsonRpseeError> {
    let bytes = decode_hex_param(value, "account")?;

    if bytes.len() == 32 {
        return AccountId::decode(&mut &bytes[..])
            .or_else(|_| AccountId::decode(&mut &bytes.encode()[..]))
            .map_err(|e| custom_error(format!("Invalid account bytes: {e}")));
    }

    AccountId::decode(&mut &bytes[..])
        .map_err(|e| custom_error(format!("Invalid SCALE-encoded account: {e}")))
}

fn decode_hex_32(value: &str, label: &str) -> Result<[u8; 32], JsonRpseeError> {
    let bytes = decode_hex_param(value, label)?;
    if bytes.len() != 32 {
        return Err(custom_error(format!("{label} must be 32 bytes")));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn parse_u128_value(
    value: Option<&serde_json::Value>,
    label: &str,
) -> Result<u128, JsonRpseeError> {
    let raw = value.ok_or_else(|| custom_error(format!("Missing {label}")))?;
    if let Some(as_str) = raw.as_str() {
        return as_str
            .parse::<u128>()
            .map_err(|e| custom_error(format!("Invalid {label}: {e}")));
    }
    if let Some(as_u64) = raw.as_u64() {
        return Ok(as_u64 as u128);
    }
    Err(custom_error(format!(
        "Invalid {label}: expected string or integer"
    )))
}

/// Build the full RPC module exposed by the node.
pub fn create_full<C, P>(
    client: Arc<C>,
    pool: Arc<P>,
    deny_unsafe: DenyUnsafe,
    _flash_finality_gadget: Option<Arc<FlashFinalityGadget>>,
    rate_limiter: Arc<RateLimiter>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
    C: ProvideRuntimeApi<Block>
        + HeaderBackend<Block>
        + HeaderMetadata<Block, Error = BlockChainError>
        + BlockBackend<Block>
        + Send
        + Sync
        + 'static,
    C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>
        + pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>
        + BlockBuilder<Block>
        + pallet_x3_kernel::AtlasKernelRuntimeApi<Block, AccountId, Balance, AssetId>,
    P: TransactionPool + Sync + Send + 'static,
{
    let mut module = RpcModule::new(());

    module.merge(System::new(client.clone(), pool, deny_unsafe).into_rpc())?;
    module.merge(TransactionPayment::new(client.clone()).into_rpc())?;
    module.merge(crate::rpc_frontier::create_frontier_stub(client.clone())?)?;
    module.merge(crate::rpc_frontier::create_svm_stub(client.clone())?)?;

    let check_rate_limit = {
        let limiter = rate_limiter.clone();
        move |method: &str| -> Result<(), JsonRpseeError> {
            limiter
                .check_request(0, method)
                .map_err(|_| custom_error("Rate limit exceeded"))
        }
    };

    let wallet_dex = Arc::new(WalletDexRpc::<Block, C>::new(client.clone()));
    let swap_rpc = Arc::new(Mutex::new(SwapRPCServer::new()));

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

    let c = client.clone();
    let check = check_rate_limit.clone();
    module.register_method("x3_getAssetMetadata", move |params, _| {
        check("x3_getAssetMetadata")?;
        let asset_id: u32 = params.one()?;
        let api = c.runtime_api();
        let at = c.info().best_hash;
        let metadata: Option<(Vec<u8>, u8)> = api
            .get_asset_metadata(at, asset_id)
            .map_err(|e| custom_error(format!("Runtime error: {e:?}")))?;

        Ok(metadata
            .map(|(symbol, decimals)| (String::from_utf8_lossy(&symbol).to_string(), decimals)))
    })?;

    let c = client.clone();
    let check = check_rate_limit.clone();
    module.register_method("x3_isAuthorized", move |params, _| {
        check("x3_isAuthorized")?;
        let account: String = params.one()?;
        let account_id = decode_account_id(&account)?;
        let api = c.runtime_api();
        let at = c.info().best_hash;
        api.is_authorized(at, account_id)
            .map_err(|e| custom_error(format!("Runtime error: {e:?}")))
    })?;

    let c = client.clone();
    let check = check_rate_limit.clone();
    module.register_method("x3_getAuthorizedAccounts", move |_params, _| {
        check("x3_getAuthorizedAccounts")?;
        let api = c.runtime_api();
        let at = c.info().best_hash;
        let accounts = api
            .get_authorized_accounts(at)
            .map_err(|e| custom_error(format!("Runtime error: {e:?}")))?;

        Ok(accounts
            .into_iter()
            .map(|account| format!("0x{}", hex::encode(account.encode())))
            .collect::<Vec<_>>())
    })?;

    let c = client.clone();
    let check = check_rate_limit.clone();
    module.register_method("x3_getAuthorities", move |_params, _| {
        check("x3_getAuthorities")?;
        let api = c.runtime_api();
        let at = c.info().best_hash;
        let authorities = api
            .get_authorities(at)
            .map_err(|e| custom_error(format!("Runtime error: {e:?}")))?;

        Ok(authorities
            .into_iter()
            .map(|account| format!("0x{}", hex::encode(account.encode())))
            .collect::<Vec<_>>())
    })?;
    let c = client.clone();
    let check = check_rate_limit.clone();
    module.register_method("x3_getCanonicalBalance", move |params, _| {
        check("x3_getCanonicalBalance")?;
        let (account, asset_id): (String, u32) = params.parse()?;
        let account_id = decode_account_id(&account)?;
        let api = c.runtime_api();
        let at = c.info().best_hash;
        let bal = api
            .get_canonical_balance(at, account_id, asset_id)
            .map_err(|e| custom_error(format!("Runtime error: {e:?}")))?;
        Ok(format!("0x{:x}", bal))
    })?;

    let c = client.clone();
    let check = check_rate_limit.clone();
    module.register_method("x3_submitCrossVmTransaction", move |params, _| {
        check("x3_submitCrossVmTransaction")?;
        let body: serde_json::Value = params.one()?;
        let evm_payload_hex = body
            .get("evm_payload")
            .and_then(|v| v.as_str())
            .ok_or_else(|| custom_error("Missing evm_payload"))?;
        let svm_payload_hex = body.get("svm_payload").and_then(|v| v.as_str()).unwrap_or("0x");
        let atomic = body.get("atomic").and_then(|v| v.as_bool()).unwrap_or(true);

        if atomic || svm_payload_hex != "0x" {
            return Err(custom_error(
                "x3_submitCrossVmTransaction cross-VM mode is not implemented yet; svm_payload is currently unsupported",
            ));
        }

        let evm_payload = decode_hex_param(evm_payload_hex, "evm_payload")?;

        let api = c.runtime_api();
        let at = c.info().best_hash;
        let tx_hash = api
            .submit_evm_transaction(at, evm_payload)
            .map_err(|e| custom_error(format!("Runtime error: {e:?}")))?
            .map_err(|e| custom_error(format!("Execution failed: {}", String::from_utf8_lossy(&e))))?;

        Ok(format!("0x{}", hex::encode(tx_hash)))
    })?;

    let check = check_rate_limit.clone();
    let wallet_dex_estimate = wallet_dex.clone();
    module.register_method("walletDex_estimateSwap", move |params, _| {
        check("walletDex_estimateSwap")?;
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

    let check = check_rate_limit.clone();
    let wallet_dex_execute = wallet_dex.clone();
    module.register_method("walletDex_executeSwap", move |params, _| {
        check("walletDex_executeSwap")?;
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

        let signatures: Vec<Vec<u8>> = req
            .get("signatures")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| decode_hex_param(s, "signature"))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();

        wallet_dex_execute
            .execute_swap(request, signatures)
            .map_err(|e| custom_error(format!("walletDex_executeSwap failed: {e}")))
    })?;

    let swap_create = swap_rpc.clone();
    let check = check_rate_limit.clone();
    module.register_method("atomicTrade_createSwap", move |params, _| {
        check("atomicTrade_createSwap")?;
        let req: serde_json::Value = params.one()?;
        let token_in = req
            .get("token_in")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let token_out = req
            .get("token_out")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let amount_in = parse_u128_value(req.get("amount_in"), "amount_in")?;
        let min_amount_out = match req.get("min_amount_out") {
            Some(v) => parse_u128_value(Some(v), "min_amount_out")?,
            None => 0,
        };
        let deadline = req.get("deadline").and_then(|v| v.as_u64()).unwrap_or(0);
        let from = req
            .get("from")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let mut engine = swap_create
            .lock()
            .map_err(|_| custom_error("Swap engine lock poisoned"))?;

        let order = engine
            .create_swap(
                from,
                TokenPair {
                    token_in,
                    token_out,
                    amount_in,
                },
                min_amount_out,
                deadline,
            )
            .map_err(custom_error)?;

        Ok(serde_json::json!({"id": order.id, "status": "Pending"}))
    })?;

    let swap_execute = swap_rpc.clone();
    let check = check_rate_limit.clone();
    module.register_method("atomicTrade_executeSwap", move |params, _| {
        check("atomicTrade_executeSwap")?;
        let req: serde_json::Value = params.one()?;
        let order_id = req
            .get("order_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| custom_error("Missing order_id"))?;
        let block_height = req
            .get("block_height")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let mut engine = swap_execute
            .lock()
            .map_err(|_| custom_error("Swap engine lock poisoned"))?;
        let order = engine
            .execute_swap(order_id, block_height)
            .map_err(custom_error)?;

        let (status, amount_out, block) = match order.status {
            x3_atomic_trade::SwapStatus::Executed { amount_out, block } => {
                ("Executed", amount_out, block)
            }
            x3_atomic_trade::SwapStatus::Pending => ("Pending", 0, 0),
            x3_atomic_trade::SwapStatus::Expired => ("Expired", 0, 0),
            x3_atomic_trade::SwapStatus::Failed { .. } => ("Failed", 0, 0),
        };

        Ok(serde_json::json!({
            "id": order.id,
            "status": status,
            "amount_out": amount_out.to_string(),
            "block": block,
        }))
    })?;

    let swap_quote = swap_rpc.clone();
    let check = check_rate_limit.clone();
    module.register_method("atomicTrade_getSwapQuote", move |params, _| {
        check("atomicTrade_getSwapQuote")?;
        let req: serde_json::Value = params.one()?;
        let token_in = req
            .get("token_in")
            .and_then(|v| v.as_str())
            .ok_or_else(|| custom_error("Missing token_in"))?
            .to_string();
        let token_out = req
            .get("token_out")
            .and_then(|v| v.as_str())
            .ok_or_else(|| custom_error("Missing token_out"))?
            .to_string();
        let amount_in = parse_u128_value(req.get("amount_in"), "amount_in")?;

        let engine = swap_quote
            .lock()
            .map_err(|_| custom_error("Swap engine lock poisoned"))?;
        let quote = engine
            .get_swap_quote(TokenPair {
                token_in,
                token_out,
                amount_in,
            })
            .map_err(custom_error)?;

        Ok(serde_json::json!({
            "amount_out": quote.amount_out.to_string(),
            "slippage_pct": quote.slippage_pct,
            "price": quote.price,
            "execution_time_ms": quote.execution_time_ms,
            "route": quote.route,
        }))
    })?;

    let swap_slippage = swap_rpc.clone();
    let check = check_rate_limit.clone();
    module.register_method("atomicTrade_estimateSlippage", move |params, _| {
        check("atomicTrade_estimateSlippage")?;
        let req: serde_json::Value = params.one()?;
        let token_in = req
            .get("token_in")
            .and_then(|v| v.as_str())
            .ok_or_else(|| custom_error("Missing token_in"))?;
        let token_out = req
            .get("token_out")
            .and_then(|v| v.as_str())
            .ok_or_else(|| custom_error("Missing token_out"))?;
        let amount_in = parse_u128_value(req.get("amount_in"), "amount_in")?;

        let engine = swap_slippage
            .lock()
            .map_err(|_| custom_error("Swap engine lock poisoned"))?;
        engine
            .estimate_slippage(token_in, token_out, amount_in)
            .map_err(custom_error)
    })?;

    let swap_status = swap_rpc.clone();
    let check = check_rate_limit.clone();
    module.register_method("atomicTrade_getSwapStatus", move |params, _| {
        check("atomicTrade_getSwapStatus")?;
        let order_id: String = params.one()?;
        let engine = swap_status
            .lock()
            .map_err(|_| custom_error("Swap engine lock poisoned"))?;
        let order = engine
            .get_order(&order_id)
            .ok_or_else(|| custom_error("Swap not found"))?;

        Ok(serde_json::json!({"id": order.id, "status": format!("{:?}", order.status)}))
    })?;

    let check = check_rate_limit.clone();
    module.register_method("x3_newCore", move |_params, _| {
        check("x3_newCore")?;
        Err::<String, _>(custom_error(
            "x3_newCore is not available on this node build",
        ))
    })?;

    Ok(module)
}
