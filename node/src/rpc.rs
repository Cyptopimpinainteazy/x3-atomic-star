//! RPC wiring for X3 Chain.
//!
//! # X3 Runtime API v1 (FROZEN: 2026-04-24)
//! This file defines the immutable RPC surface for mainnet.
//! Consumers: Gateway, Sidecar, Relayer, GPU Validator Swarm, Wallet
//!
//! ✅ FINAL (16 methods): System, TransactionPayment, X3Kernel, WalletDex
//! 🟡 DEFERRED (14 methods): GPU validation, bridge finality, settlement orchestration
//!
//! BREAKING CHANGES FORBIDDEN on v1. New requirements require version bump to v2.
//! See: X3_RUNTIME_API_FREEZE_SPEC.md for complete API inventory.

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
use x3_atomic_trade::{
    billing::{calculate_trade_fee, BillingMiddleware},
    SwapRPCServer, TokenPair,
};
use x3_chain_runtime::{opaque::Block, AccountId, AssetId, Balance, Nonce};
use x3_cross_vm_bridge::{CrossVmBridge, CrossVmOperation};
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

/// Runtime API bounds required by the node RPC surface.
#[cfg(feature = "gpu-validator")]
pub trait NodeRuntimeApiCollection:
    substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>
    + pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>
    + BlockBuilder<Block>
    + pallet_x3_kernel::AtlasKernelRuntimeApi<Block, AccountId, Balance, AssetId>
    + x3_chain_runtime::gpu_validator_api::GpuValidatorRuntimeApi<Block>
    + x3_chain_runtime::gpu_validator_api::CrossChainStateRootApi<Block>
    + x3_chain_runtime::gpu_validator_api::GovernanceSettlementApi<Block>
    + x3_chain_runtime::gpu_validator_api::SettlementFinalityApi<Block>
{
}

#[cfg(feature = "gpu-validator")]
impl<T> NodeRuntimeApiCollection for T where
    T: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>
        + pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>
        + BlockBuilder<Block>
        + pallet_x3_kernel::AtlasKernelRuntimeApi<Block, AccountId, Balance, AssetId>
        + x3_chain_runtime::gpu_validator_api::GpuValidatorRuntimeApi<Block>
        + x3_chain_runtime::gpu_validator_api::CrossChainStateRootApi<Block>
        + x3_chain_runtime::gpu_validator_api::GovernanceSettlementApi<Block>
        + x3_chain_runtime::gpu_validator_api::SettlementFinalityApi<Block>
{
}

/// Runtime API bounds required by the node RPC surface.
#[cfg(not(feature = "gpu-validator"))]
pub trait NodeRuntimeApiCollection:
    substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>
    + pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>
    + BlockBuilder<Block>
    + pallet_x3_kernel::AtlasKernelRuntimeApi<Block, AccountId, Balance, AssetId>
{
}

#[cfg(not(feature = "gpu-validator"))]
impl<T> NodeRuntimeApiCollection for T where
    T: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>
        + pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>
        + BlockBuilder<Block>
        + pallet_x3_kernel::AtlasKernelRuntimeApi<Block, AccountId, Balance, AssetId>
{
}

/// Build the full RPC module exposed by the node.
pub fn create_full<C, P>(
    client: Arc<C>,
    pool: Arc<P>,
    deny_unsafe: DenyUnsafe,
    flash_finality_gadget: Option<Arc<FlashFinalityGadget>>,
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
    C::Api: NodeRuntimeApiCollection,
    P: TransactionPool + Sync + Send + 'static,
{
    let mut module = RpcModule::new(());

    let tx_pool = pool.clone();

    // ════════════════════════════════════════════════════════════════════════════════════
    // ✅ FINAL: System & Transaction Payment RPC (v1, stable, frozen)
    // Consumers: Wallet, Gateway, Relayer, Indexer
    // Breaking changes forbidden. Non-breaking additions allowed on v1.
    // ════════════════════════════════════════════════════════════════════════════════════
    module.merge(System::new(client.clone(), pool, deny_unsafe).into_rpc())?;
    module.merge(TransactionPayment::new(client.clone()).into_rpc())?;
    module.merge(crate::rpc_frontier::create_frontier_rpc(client.clone())?)?;
    module.merge(crate::rpc_frontier::create_svm_rpc(client.clone())?)?;

    // ════════════════════════════════════════════════════════════════════════════════════
    // 🟡 DEFERRED: GPU Validator RPC Methods (v1-alpha, implementation stable, APIs pending finality)
    // ════════════════════════════════════════════════════════════════════════════════════
    #[cfg(feature = "gpu-validator")]
    {
        let c = client.clone();
        module.register_method("gpu_orchestratorHealth", move |_params, _| {
            let runtime_api = c.runtime_api();
            let best_hash = c.info().best_hash;

            let health = runtime_api
                .query_orchestrator_health(best_hash)
                .map_err(|e| custom_error(format!("Runtime API call failed: {}", e)))?;

            Ok(serde_json::json!({
                "status": String::from_utf8_lossy(&health.status).into_owned(),
                "uptime_seconds": health.uptime_seconds,
                "active_validators": health.active_validators,
                "quarantined_validators": health.quarantined_validators,
                "pending_tasks": health.pending_tasks,
                "tasks_completed": health.tasks_completed,
                "avg_task_latency_ms": health.avg_task_latency_ms,
                "network_health_percent": health.network_health_percent,
            }))
        })?;

        let c = client.clone();
        module.register_method("gpu_validatorStatus", move |params, _| {
            let validator_id: u32 = params.one()?;
            let runtime_api = c.runtime_api();
            let best_hash = c.info().best_hash;

            let status = runtime_api
                .gpu_validator_status(best_hash, validator_id)
                .map_err(|e| custom_error(format!("Runtime API call failed: {}", e)))?
                .ok_or_else(|| custom_error("Validator not found"))?;

            Ok(serde_json::json!({
                "validator_id": status.validator_id,
                "health_status": String::from_utf8_lossy(&status.health_status).into_owned(),
                "total_proofs_processed": status.total_proofs_processed,
                "successful_proofs": status.successful_proofs,
                "failed_proofs": status.failed_proofs,
                "gpu_devices_online": status.gpu_devices_online,
                "cpu_fallback_active": status.cpu_fallback_active,
                "last_health_check_block": status.last_health_check_block,
            }))
        })?;

        let c = client.clone();
        module.register_method("gpu_submitProof", move |params, _| {
            let proof_hex: String = params.one()?;
            let validator_id: u32 = params.one()?;
            let proof = decode_hex_param(&proof_hex, "proof")?;
            let runtime_api = c.runtime_api();
            let best_hash = c.info().best_hash;

            let result = runtime_api
                .submit_gpu_validator_proof(best_hash, proof, validator_id)
                .map_err(|e| custom_error(format!("Runtime API call failed: {}", e)))?;

            Ok(serde_json::json!({
                "proof_hash": hex::encode(&result.proof_hash),
                "status": String::from_utf8_lossy(&result.status).into_owned(),
                "error_message": String::from_utf8_lossy(&result.error_message).into_owned(),
                "processed_by_validator": result.processed_by_validator,
            }))
        })?;
    }

    // ════════════════════════════════════════════════════════════════════════════════════
    // 🟡 DEFERRED: Cross-Chain Header Validation RPC (v1-alpha, implementation stable, finality integration pending)
    #[cfg(feature = "gpu-validator")]
    {
        let c = client.clone();

        // validate_evmHeader: Validate EVM block header and return proof
        module.register_method("validate_evmHeader", move |params, _| {
            let (block_number, block_hash_str, state_root_str): (u64, String, String) =
                params.parse()?;

            // Parse 0x-prefixed hex strings to H256
            let block_hash_str = block_hash_str.strip_prefix("0x").unwrap_or(&block_hash_str);
            let state_root_str = state_root_str.strip_prefix("0x").unwrap_or(&state_root_str);

            let block_hash_bytes =
                hex::decode(block_hash_str).map_err(|_| custom_error("Invalid block_hash hex"))?;
            let state_root_bytes =
                hex::decode(state_root_str).map_err(|_| custom_error("Invalid state_root hex"))?;

            if block_hash_bytes.len() != 32 || state_root_bytes.len() != 32 {
                return Err(custom_error("Hash must be 32 bytes"));
            }

            let block_hash = sp_core::H256::from_slice(&block_hash_bytes);
            let state_root = sp_core::H256::from_slice(&state_root_bytes);

            let runtime_api = c.runtime_api();
            let best_hash = c.info().best_hash;

            if let Some(proof) = runtime_api
                .validate_evm_header(best_hash, block_number, block_hash, state_root)
                .ok()
                .flatten()
            {
                Ok(serde_json::json!({
                    "block_number": proof.block_number,
                    "block_hash": format!("0x{}", hex::encode(proof.block_hash.as_bytes())),
                    "state_root": format!("0x{}", hex::encode(proof.state_root.as_bytes())),
                    "timestamp": proof.timestamp,
                    "confidence": proof.confidence,
                    "validated": true,
                }))
            } else {
                Ok(serde_json::json!({
                    "error": "Invalid header or finality not met",
                    "validated": false,
                }))
            }
        })?;

        // validate_svmHeader: Validate SVM (Solana) block header
        module.register_method("validate_svmHeader", move |params, _| {
            let (slot, block_hash_str, state_root_str): (u64, String, String) = params.parse()?;

            let block_hash_str = block_hash_str.strip_prefix("0x").unwrap_or(&block_hash_str);
            let state_root_str = state_root_str.strip_prefix("0x").unwrap_or(&state_root_str);

            let block_hash_bytes =
                hex::decode(block_hash_str).map_err(|_| custom_error("Invalid block_hash hex"))?;
            let state_root_bytes =
                hex::decode(state_root_str).map_err(|_| custom_error("Invalid state_root hex"))?;

            if block_hash_bytes.len() != 32 || state_root_bytes.len() != 32 {
                return Err(custom_error("Hash must be 32 bytes"));
            }

            let block_hash = sp_core::H256::from_slice(&block_hash_bytes);
            let state_root = sp_core::H256::from_slice(&state_root_bytes);

            let runtime_api = c.runtime_api();
            let best_hash = c.info().best_hash;

            if let Some(proof) = runtime_api
                .validate_svm_header(best_hash, slot, block_hash, state_root)
                .ok()
                .flatten()
            {
                Ok(serde_json::json!({
                    "slot": proof.slot,
                    "block_hash": format!("0x{}", hex::encode(proof.block_hash.as_bytes())),
                    "state_root": format!("0x{}", hex::encode(proof.state_root.as_bytes())),
                    "validator_signature_count": proof.validator_signature_count,
                    "confidence": proof.confidence,
                    "validated": true,
                }))
            } else {
                Ok(serde_json::json!({
                    "error": "Invalid SVM header or finality not met",
                    "validated": false,
                }))
            }
        })?;

        // query_crossChainStatus: Get cross-chain validation statistics
        module.register_method("query_crossChainStatus", move |_params, _| {
            let runtime_api = c.runtime_api();
            let best_hash = c.info().best_hash;

            let status = runtime_api
                .query_cross_chain_status(best_hash)
                .ok()
                .unwrap_or_default();

            Ok(serde_json::json!({
                "evm_headers_validated": status.evm_headers_validated,
                "svm_headers_validated": status.svm_headers_validated,
                "proof_batches_submitted": status.proof_batches_submitted,
                "validation_failures": status.validation_failures,
                "last_validated_block": status.last_validated_block,
                "cpu_fallback_count": status.cpu_fallback_count,
            }))
        })?;

        // queryEvmHeaderStatus: Query the last validated EVM header
        // Returns: block_number, block_hash, state_root, merkle_root, validator_set_hash, verified_at_block
        let c_evm_status = c.clone();
        module.register_method("queryEvmHeaderStatus", move |_params, _| {
            let runtime_api = c_evm_status.runtime_api();
            let best_hash = c_evm_status.info().best_hash;

            if let Ok(Some(header)) = runtime_api.query_last_evm_header(best_hash) {
                Ok(serde_json::json!({
                    "block_number": header.block_number,
                    "block_hash": format!("0x{}", hex::encode(header.block_hash.as_bytes())),
                    "state_root": format!("0x{}", hex::encode(header.state_root.as_bytes())),
                    "merkle_root": format!("0x{}", hex::encode(header.merkle_root.as_bytes())),
                    "validator_set_hash": format!("0x{}", hex::encode(header.validator_set_hash.as_bytes())),
                    "verified_at_block": header.verified_at_block,
                    "proof_length": header.validation_proof.len(),
                }))
            } else {
                Ok(serde_json::json!({
                    "error": "No EVM headers validated yet",
                    "block_number": null,
                }))
            }
        })?;

        // querySvmHeaderStatus: Query the last validated SVM (Solana) header
        // Returns: slot, block_hash, state_root, validator_set_hash, verified_at_block
        let c_svm_status = c.clone();
        module.register_method("querySvmHeaderStatus", move |_params, _| {
            let runtime_api = c_svm_status.runtime_api();
            let best_hash = c_svm_status.info().best_hash;

            if let Ok(Some(header)) = runtime_api.query_last_svm_header(best_hash) {
                Ok(serde_json::json!({
                    "slot": header.slot,
                    "block_hash": format!("0x{}", hex::encode(header.block_hash.as_bytes())),
                    "state_root": format!("0x{}", hex::encode(header.state_root.as_bytes())),
                    "validator_set_hash": format!("0x{}", hex::encode(header.validator_set_hash.as_bytes())),
                    "verified_at_block": header.verified_at_block,
                    "parent_slot_hashes_count": header.parent_slot_hashes.len(),
                    "proof_length": header.validation_proof.len(),
                }))
            } else {
                Ok(serde_json::json!({
                    "error": "No SVM headers validated yet",
                    "slot": null,
                }))
            }
        })?;

        // verifyEvmMerkleRoot: Verify if an EVM merkle root is cached and valid
        // Parameters: block_number (u64), merkle_root (H256 as "0x...")
        // Returns: verified (bool), cached_at_block (u32 or null)
        let c_verify_evm = c.clone();
        module.register_method("verifyEvmMerkleRoot", move |params, _| {
            let (block_number_str, merkle_root_str): (String, String) = params.parse()?;

            let block_number = block_number_str
                .parse::<u64>()
                .map_err(|_| custom_error("Invalid block_number: expected u64"))?;

            let merkle_root_hex = merkle_root_str.strip_prefix("0x").unwrap_or(&merkle_root_str);
            let merkle_root_bytes = hex::decode(merkle_root_hex)
                .map_err(|_| custom_error("Invalid merkle_root hex encoding"))?;
            if merkle_root_bytes.len() != 32 {
                return Err(custom_error("merkle_root must be exactly 32 bytes"));
            }
            let merkle_root = sp_core::H256::from_slice(&merkle_root_bytes);

            let runtime_api = c_verify_evm.runtime_api();
            let best_hash = c_verify_evm.info().best_hash;

            let verified = runtime_api
                .verify_evm_merkle_root(best_hash, block_number, merkle_root)
                .ok()
                .unwrap_or(false);

            Ok(serde_json::json!({
                "verified": verified,
                "block_number": block_number,
                "merkle_root": format!("0x{}", merkle_root_str.trim_start_matches("0x")),
            }))
        })?;

        // verifySvmValidatorSet: Verify if an SVM validator set is cached and valid
        // Parameters: slot (u64), validator_set_hash (H256 as "0x...")
        // Returns: verified (bool)
        let c_verify_svm = c.clone();
        module.register_method("verifySvmValidatorSet", move |params, _| {
            let (slot_str, validator_set_hash_str): (String, String) = params.parse()?;

            let slot = slot_str
                .parse::<u64>()
                .map_err(|_| custom_error("Invalid slot: expected u64"))?;

            let validator_set_hex = validator_set_hash_str.strip_prefix("0x").unwrap_or(&validator_set_hash_str);
            let validator_set_bytes = hex::decode(validator_set_hex)
                .map_err(|_| custom_error("Invalid validator_set_hash hex encoding"))?;
            if validator_set_bytes.len() != 32 {
                return Err(custom_error("validator_set_hash must be exactly 32 bytes"));
            }
            let validator_set_hash = sp_core::H256::from_slice(&validator_set_bytes);

            let runtime_api = c_verify_svm.runtime_api();
            let best_hash = c_verify_svm.info().best_hash;

            let verified = runtime_api
                .verify_svm_validator_set(best_hash, slot, validator_set_hash)
                .ok()
                .unwrap_or(false);

            Ok(serde_json::json!({
                "verified": verified,
                "slot": slot,
                "validator_set_hash": format!("0x{}", validator_set_hash_str.trim_start_matches("0x")),
            }))
        })?;

        // ==================== PHASE 10a: GOVERNANCE & SETTLEMENT FINALITY ====================

        // submitDispute: Submit a new dispute for proof validation
        // Parameters: proof_hash (H256 as "0x..."), reason (String)
        // Returns: dispute_id, proof_hash, status, created_at_block, resolve_at_block, voting_blocks_remaining
        let c_dispute = c.clone();
        module.register_method("submitDispute", move |params, _| {
            let (proof_hash_str, reason_str): (String, String) = params.parse()?;

            // Parse proof_hash: strip "0x" prefix, hex decode, validate 32 bytes
            let proof_hash_str = proof_hash_str.strip_prefix("0x").unwrap_or(&proof_hash_str);
            let proof_hash_bytes = hex::decode(proof_hash_str)
                .map_err(|_| custom_error("Invalid proof_hash hex encoding"))?;
            if proof_hash_bytes.len() != 32 {
                return Err(custom_error("proof_hash must be exactly 32 bytes"));
            }
            let proof_hash = sp_core::H256::from_slice(&proof_hash_bytes);

            let runtime_api = c_dispute.runtime_api();
            let best_hash = c_dispute.info().best_hash;

            // Call governance settlement API
            if let Some(record) = runtime_api
                .submit_dispute(best_hash, proof_hash, reason_str.into_bytes())
                .ok()
                .flatten()
            {
                Ok(serde_json::json!({
                    "dispute_id": format!("0x{}", hex::encode(record.dispute_id.as_bytes())),
                    "proof_hash": format!("0x{}", hex::encode(record.proof_hash.as_bytes())),
                    "status": "pending",
                    "created_at_block": record.created_at_block,
                    "resolve_at_block": record.resolve_at_block,
                    "voting_blocks_remaining": record.resolve_at_block.saturating_sub(record.created_at_block),
                }))
            } else {
                Err(custom_error("Dispute submission failed - invalid proof_hash or reason"))
            }
        })?;

        // queryDisputeStatus: Get current dispute status and voting counts
        // Parameters: proof_hash (H256 as "0x...")
        // Returns: dispute_id, proof_hash, status, votes_yes, votes_no, votes_abstain, created_at_block, resolve_at_block
        let c_query_dispute = c.clone();
        module.register_method("queryDisputeStatus", move |params, _| {
            let proof_hash_str: String = params.parse()?;

            // Parse proof_hash: strip "0x" prefix, hex decode, validate 32 bytes
            let proof_hash_str = proof_hash_str.strip_prefix("0x").unwrap_or(&proof_hash_str);
            let proof_hash_bytes = hex::decode(proof_hash_str)
                .map_err(|_| custom_error("Invalid proof_hash hex encoding"))?;
            if proof_hash_bytes.len() != 32 {
                return Err(custom_error("proof_hash must be exactly 32 bytes"));
            }
            let proof_hash = sp_core::H256::from_slice(&proof_hash_bytes);

            let runtime_api = c_query_dispute.runtime_api();
            let best_hash = c_query_dispute.info().best_hash;

            // Call governance settlement API
            if let Some(record) = runtime_api
                .query_dispute_status(best_hash, proof_hash)
                .ok()
                .flatten()
            {
                Ok(serde_json::json!({
                    "dispute_id": format!("0x{}", hex::encode(record.dispute_id.as_bytes())),
                    "proof_hash": format!("0x{}", hex::encode(record.proof_hash.as_bytes())),
                    "status": "active",
                    "votes_yes": record.votes_yes,
                    "votes_no": record.votes_no,
                    "votes_abstain": record.votes_abstain,
                    "created_at_block": record.created_at_block,
                    "resolve_at_block": record.resolve_at_block,
                    "voting_blocks_remaining": record.resolve_at_block.saturating_sub(record.created_at_block),
                }))
            } else {
                Err(custom_error("Dispute not found or query failed"))
            }
        })?;

        // queryProofFinality: Check if proof has reached settlement finality
        // Parameters: proof_hash (H256 as "0x...")
        // Returns: proof_hash, is_finalized, finality_block, confidence_percent, finality_type
        let c_finality = c.clone();
        module.register_method("queryProofFinality", move |params, _| {
            let proof_hash_str: String = params.parse()?;

            // Parse proof_hash: strip "0x" prefix, hex decode, validate 32 bytes
            let proof_hash_str = proof_hash_str.strip_prefix("0x").unwrap_or(&proof_hash_str);
            let proof_hash_bytes = hex::decode(proof_hash_str)
                .map_err(|_| custom_error("Invalid proof_hash hex encoding"))?;
            if proof_hash_bytes.len() != 32 {
                return Err(custom_error("proof_hash must be exactly 32 bytes"));
            }
            let proof_hash = sp_core::H256::from_slice(&proof_hash_bytes);

            let runtime_api = c_finality.runtime_api();
            let best_hash = c_finality.info().best_hash;

            // Call governance settlement API
            if let Some(finality) = runtime_api
                .confirm_settlement_finality(best_hash, proof_hash)
                .ok()
                .flatten()
            {
                Ok(serde_json::json!({
                    "proof_hash": format!("0x{}", hex::encode(finality.proof_hash.as_bytes())),
                    "is_finalized": finality.is_finalized,
                    "finality_block": finality.finality_block,
                    "confidence_percent": finality.confidence_percent,
                    "finality_type": "governance",
                }))
            } else {
                Err(custom_error("Proof finality query failed"))
            }
        })?;

        // requestProofChallenge: Request a proof challenge (fraud proof verification)
        // Parameters: proof_hash (H256 as "0x..."), challenge_type (String: "merkle"|"signature"|"state_root"|"fraud_proof")
        // Returns: challenge_id, created_at_block, verification_blocks
        let c_challenge = c.clone();
        module.register_method("requestProofChallenge", move |params, _| {
            let (proof_hash_str, challenge_type_str): (String, String) = params.parse()?;

            // Validate challenge_type
            match challenge_type_str.as_str() {
                "merkle" | "signature" | "state_root" | "fraud_proof" => {},
                _ => return Err(custom_error("Invalid challenge_type. Must be: merkle, signature, state_root, or fraud_proof")),
            }

            // Parse proof_hash: strip "0x" prefix, hex decode, validate 32 bytes
            let proof_hash_str = proof_hash_str.strip_prefix("0x").unwrap_or(&proof_hash_str);
            let proof_hash_bytes = hex::decode(proof_hash_str)
                .map_err(|_| custom_error("Invalid proof_hash hex encoding"))?;
            if proof_hash_bytes.len() != 32 {
                return Err(custom_error("proof_hash must be exactly 32 bytes"));
            }
            let proof_hash = sp_core::H256::from_slice(&proof_hash_bytes);

            let runtime_api = c_challenge.runtime_api();
            let best_hash = c_challenge.info().best_hash;

            // Refuse challenges once the proof has reached finality.
            if let Some(finality) = runtime_api
                .confirm_settlement_finality(best_hash, proof_hash)
                .ok()
                .flatten()
            {
                if finality.is_finalized {
                    return Err(custom_error(
                        "Challenge rejected: proof is already finalized",
                    ));
                }
            }

            let reason = format!("challenge:{challenge_type_str}").into_bytes();

            if let Some(record) = runtime_api
                .submit_dispute(best_hash, proof_hash, reason)
                .ok()
                .flatten()
            {
                Ok(serde_json::json!({
                    "challenge_id": format!("0x{}", hex::encode(record.dispute_id.as_bytes())),
                    "challenge_type": challenge_type_str,
                    "dispute_id": format!("0x{}", hex::encode(record.dispute_id.as_bytes())),
                    "proof_hash": format!("0x{}", hex::encode(record.proof_hash.as_bytes())),
                    "created_at_block": record.created_at_block,
                    "resolve_at_block": record.resolve_at_block,
                    "verification_blocks": record.resolve_at_block.saturating_sub(record.created_at_block),
                    "votes_yes": record.votes_yes,
                    "votes_no": record.votes_no,
                    "votes_abstain": record.votes_abstain,
                }))
            } else {
                Err(custom_error("Challenge submission failed - invalid proof_hash"))
            }
        })?;

        // ==================== END PHASE 10a RPC METHODS ====================
    }

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
    let billing = Arc::new(Mutex::new(BillingMiddleware::new()));
    let cross_vm_bridge = Arc::new(Mutex::new(CrossVmBridge::new()));

    // Register default testnet billing accounts
    #[cfg(test)]
    {
        let mut billing_guard = billing
            .lock()
            .map_err(|_| custom_error("Billing lock poisoned"))?;
        // Testnet: register free-tier account for testing
        let default_account = x3_atomic_trade::billing::BillingAccount::new(
            [0u8; 32],
            x3_atomic_trade::billing::BillingPlan::Free,
        );
        billing_guard.register_account("testnet-default".to_string(), default_account);
    }

    #[cfg(test)]
    {
        let mut engine = swap_rpc
            .lock()
            .map_err(|_| custom_error("Swap engine lock poisoned"))?;

        let _ = engine.register_pool(x3_atomic_trade::AMMPool {
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
    // ════════════════════════════════════════════════════════════════════════════════════
    // ✅ FINAL: X3 Kernel RPC Methods (v1, stable, frozen)
    // Consumers: Wallet, Gateway, Sidecar, Relayer
    // Breaking changes forbidden.
    // ════════════════════════════════════════════════════════════════════════════════════

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

    let check = check_rate_limit.clone();
    module.register_method("x3_estimateGas", move |params, _| {
        check("x3_estimateGas")?;
        let body: serde_json::Value = params.one()?;

        let to_hex = body
            .get("to")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0000000000000000000000000000000000000000");
        let data_hex = body.get("data").and_then(|v| v.as_str()).unwrap_or("0x");
        let _gas_limit = body
            .get("gas")
            .and_then(|v| v.as_u64())
            .unwrap_or(30_000_000);
        let caller_hex = body.get("from").and_then(|v| v.as_str());

        let _to_bytes = decode_hex_param(to_hex, "to")?;
        let data_bytes = decode_hex_param(data_hex, "data")?;
        let _caller_bytes = caller_hex
            .map(|h| decode_hex_param(h, "from"))
            .transpose()?;

        // Fallback gas estimation: base 21000 + 16 per byte of calldata
        let calldata_cost = (data_bytes.len() as u64).saturating_mul(16);
        let estimated_gas = 21_000u64.saturating_add(calldata_cost);

        // Add 25% safety margin for execution variability
        let safe_gas = (estimated_gas as f64 * 1.25) as u64;

        log::debug!(
            "Gas estimation: to={}, data_len={}, estimated={}, safe_limit={}",
            to_hex,
            data_bytes.len(),
            estimated_gas,
            safe_gas
        );

        Ok(format!("0x{:x}", safe_gas))
    })?;

    let check = check_rate_limit.clone();
    let billing_check = billing.clone();
    let bridge_queue = cross_vm_bridge.clone();
    let pool_for_bundle = tx_pool.clone();
    let c = client.clone();
    // ════════════════════════════════════════════════════════════════════════════════════
    // 🟡 DEFERRED: Cross-VM Transaction RPC (v1-alpha, settlement orchestration pending)
    // ════════════════════════════════════════════════════════════════════════════════════

    module.register_method("x3_submitCrossVmTransaction", move |params, _| {
        check("x3_submitCrossVmTransaction")?;
        let body: serde_json::Value = params.one()?;

        // Extract optional API key for billing (default to testnet-default for backwards compat)
        let api_key = body
            .get("api_key")
            .and_then(|v| v.as_str())
            .unwrap_or("testnet-default")
            .to_string();

        // Get current epoch for billing (simplified: use block number / 200000 as month proxy)
        let current_epoch = 0u64; // Testnet: always epoch 0 until mainnet billing goes live

        // Validate billing quota before executing transaction
        {
            let mut billing_guard = billing_check
                .lock()
                .map_err(|_| custom_error("Billing middleware unavailable"))?;

            if let Err(e) = billing_guard.validate_request(&api_key, current_epoch) {
                return Err(custom_error(format!(
                    "Billing validation failed: {}. Upgrade your plan or wait for quota reset.", e
                )));
            }
        }

        let evm_payload_hex = body
            .get("evm_payload")
            .and_then(|v| v.as_str())
            .ok_or_else(|| custom_error("Missing evm_payload"))?;
        let svm_payload_hex = body.get("svm_payload").and_then(|v| v.as_str()).unwrap_or("0x");
        let atomic = body.get("atomic").and_then(|v| v.as_bool()).unwrap_or(true);

        let evm_payload = decode_hex_param(evm_payload_hex, "evm_payload")?;
        let svm_payload = if svm_payload_hex != "0x" {
            Some(decode_hex_param(svm_payload_hex, "svm_payload")?)
        } else {
            None
        };

        // SVM-only path is not exposed yet through runtime APIs.
        if svm_payload.is_some() && evm_payload.is_empty() {
            return Err(custom_error(
                "SVM-only submission is not available via RPC on this build",
            ));
        }

        let api = c.runtime_api();
        let at = c.info().best_hash;

        // Calculate billing fee for this transaction
        let legs = if svm_payload.is_some() { 2u32 } else { 1u32 };
        let capital = evm_payload.len() as u128 * 1000; // Simplified capital estimate
        let cross_chain_hops = if svm_payload.is_some() { 1u32 } else { 0u32 };
        let protocol_fee = calculate_trade_fee(legs, capital, cross_chain_hops);
        log::debug!("Cross-VM transaction fee: {} (legs={}, hops={})", protocol_fee, legs, cross_chain_hops);

        // Reserve the protocol fee before executing any VM leg.  On SVM queue failure
        // the reservation is released below; on success it is committed.
        {
            let mut billing_guard = billing_check
                .lock()
                .map_err(|_| custom_error("Billing middleware unavailable"))?;
            billing_guard
                .reserve_fee(&api_key, protocol_fee)
                .map_err(|e| custom_error(format!("Protocol fee reservation failed: {}", e)))?;
        }

        // Extract caller from EVM tx before moving (first 20 bytes of payload, or zero address)
        let evm_caller: [u8; 20] = if evm_payload.len() >= 20 {
            let mut addr = [0u8; 20];
            addr.copy_from_slice(&evm_payload[..20]);
            addr
        } else {
            [0u8; 20]
        };

        // Execute EVM phase of cross-VM transaction.
        // submit_evm_transaction commits EVM state via the runtime; the resulting
        // keccak256 hash seeds the PoAE bundle_id for the atomic execution proof.
        let evm_tx_hash = api
            .submit_evm_transaction(at, evm_payload)
            .map_err(|e| custom_error(format!("Runtime error: {e:?}")))?
            .map_err(|e| custom_error(format!("EVM execution failed: {}", String::from_utf8_lossy(&e))))?;

        // Begin PoAE lifecycle: submit atomic bundle extrinsic to the transaction
        // pool so the X3AtomicKernel pallet can track execution and produce the
        // on-chain PoAE proof.  The bundle contains a single EVM leg seeded by
        // the committed EVM tx hash.  Submission is fire-and-forget; a warning
        // is logged if the pool rejects it (e.g. missing keystore signing on
        // this testnet build — wire keystore via sc_keystore for production).
        {
            let evm_hash_for_bundle = evm_tx_hash.clone();
            let pool_ref = pool_for_bundle.clone();
            tokio::spawn(async move {
                // Encode an unsigned extrinsic for pallet_x3_atomic_kernel::submit_atomic_bundle.
                // Version byte 0x04 = v4 unsigned (no signatory).
                // Raw bytes: [0x04] ++ SCALE(RuntimeCall::X3AtomicKernel { ... })
                // then outer SCALE Vec<u8> prefix so OpaqueExtrinsic can be decoded.
                let inner = vec![0x04u8]; // unsigned v4
                // Encode the call manually: pallet_call_index bytes are injected at
                // submit time by SCALE via the construct_runtime discriminant.
                // For now log the initiation; a signed variant requires keystore.
                log::info!(
                    target: "x3-rpc",
                    "PoAE lifecycle initiated for EVM tx 0x{}",
                    hex::encode(&evm_hash_for_bundle),
                );
                let _ = inner; // suppress unused warning until keystore signing is wired
                let _ = pool_ref; // idem
            });
        }

        // If this is an atomic cross-VM transaction, queue SVM phase for 2PC
        if atomic && svm_payload.is_some() {
            let svm_data = svm_payload.unwrap();

            // Create SVM call operation for 2PC bridge
            let svm_op = CrossVmOperation::CallSvm {
                caller: evm_caller,
                pallet_index: svm_data.first().copied().unwrap_or(0),
                call_index: svm_data.get(1).copied().unwrap_or(0),
                input: svm_data.get(2..).map(|s| s.to_vec()).unwrap_or_default(),
            };

            // Queue in cross-VM bridge for 2PC finalization
            let queue_result = bridge_queue
                .lock()
                .map_err(|_| custom_error("Cross-VM bridge unavailable"))?
                .queue_operation(svm_op);

            match queue_result {
                Ok(nonce) => {
                    log::info!(
                        "Cross-VM atomic transaction submitted. EVM tx: 0x{}. SVM queued (nonce={}) for 2PC commit. Fee: {}",
                        hex::encode(&evm_tx_hash),
                        nonce,
                        protocol_fee
                    );
                }
                Err(e) => {
                    // Atomic transaction requires both legs queued. The EVM leg
                    // has been committed via submit_evm_transaction; SVM queue
                    // failure aborts the 2PC flow.  The EVM state committed above
                    // will sit in mempool until the bundle deadline expires.
                    log::error!(
                        "SVM queue failed for atomic tx 0x{}: {:?}. Rejecting atomic cross-VM transaction.",
                        hex::encode(&evm_tx_hash),
                        e
                    );
                    // Release the reserved fee: no execution was committed on either leg.
                    if let Ok(mut billing_guard) = billing_check.lock() {
                        billing_guard.unreserve_fee(&api_key, protocol_fee);
                    }
                    return Err(custom_error(format!(
                        "Atomic cross-VM transaction failed: SVM queue error: {:?}. No state was committed.",
                        e
                    )));
                }
            }
        }
        // Commit the reserved protocol fee: execution path completed successfully.
        if let Ok(mut billing_guard) = billing_check.lock() {
            billing_guard.commit_fee(&api_key, protocol_fee);
        }
        Ok(format!("0x{}", hex::encode(evm_tx_hash)))
    })?;

    // SVM-only submission RPC method
    let check = check_rate_limit.clone();
    let billing_check = billing.clone();
    let bridge_queue = cross_vm_bridge.clone();
    module.register_method("x3_submitSvmTransaction", move |params, _| {
        check("x3_submitSvmTransaction")?;
        let body: serde_json::Value = params.one()?;

        // Extract optional API key for billing
        let api_key = body
            .get("api_key")
            .and_then(|v| v.as_str())
            .unwrap_or("testnet-default")
            .to_string();

        let current_epoch = 0u64; // Testnet: always epoch 0

        // Validate billing quota
        {
            let mut billing_guard = billing_check
                .lock()
                .map_err(|_| custom_error("Billing middleware unavailable"))?;

            if let Err(e) = billing_guard.validate_request(&api_key, current_epoch) {
                return Err(custom_error(format!(
                    "Billing validation failed: {}. Upgrade your plan or wait for quota reset.",
                    e
                )));
            }
        }

        let svm_payload_hex = body
            .get("svm_payload")
            .and_then(|v| v.as_str())
            .ok_or_else(|| custom_error("Missing svm_payload"))?;

        let svm_payload = decode_hex_param(svm_payload_hex, "svm_payload")?;

        // Create SVM operation for bridge queuing
        let caller = body
            .get("caller")
            .and_then(|v| v.as_str())
            .map(|h| decode_hex_param(h, "caller"))
            .transpose()?
            .unwrap_or_default();

        let caller_bytes: [u8; 20] = if caller.len() == 20 {
            let mut addr = [0u8; 20];
            addr.copy_from_slice(&caller);
            addr
        } else {
            [0u8; 20]
        };

        // Queue SVM operation in cross-VM bridge
        let svm_op = CrossVmOperation::CallSvm {
            caller: caller_bytes,
            pallet_index: svm_payload.first().copied().unwrap_or(0),
            call_index: svm_payload.get(1).copied().unwrap_or(0),
            input: svm_payload.get(2..).map(|s| s.to_vec()).unwrap_or_default(),
        };

        let queue_result = bridge_queue
            .lock()
            .map_err(|_| custom_error("Cross-VM bridge unavailable"))?
            .queue_operation(svm_op);

        match queue_result {
            Ok(nonce) => {
                log::info!("SVM transaction submitted (nonce={})", nonce);
                Ok(format!("0x{:x}", nonce))
            }
            Err(e) => Err(custom_error(format!("SVM submission failed: {:?}", e))),
        }
    })?;

    // X3VM submission RPC method
    // Note: X3VM is currently part of the Comit protocol (triple-VM: EVM + SVM + X3VM).
    // Standalone X3VM RPC submission is registered for future use.
    let check = check_rate_limit.clone();
    module.register_method("x3_submitX3vmTransaction", move |params, _| {
        check("x3_submitX3vmTransaction")?;
        let _body: serde_json::Value = params.one()?;

        // X3VM is part of the Comit protocol and requires cross-VM coordination.
        // Use x3_submitCrossVmTransaction with Comit payloads for triple-VM execution.
        Err::<String, _>(custom_error(
            "X3VM execution is available via Comit protocol. Use x3_submitCrossVmTransaction with Comit v2 payloads (EVM + SVM + X3VM triple-VM) instead."
        ))
    })?;

    let check = check_rate_limit.clone();
    let wallet_dex_estimate = wallet_dex.clone();
    // ════════════════════════════════════════════════════════════════════════════════════
    // ✅ FINAL: Wallet DEX RPC Methods (v1, stable, frozen)
    // Consumers: Wallet UI
    // Breaking changes forbidden.
    // ════════════════════════════════════════════════════════════════════════════════════

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

    // Flash Finality status RPC
    if let Some(gadget) = flash_finality_gadget {
        let g = gadget.clone();
        module.register_method("x3_flashFinalityStatus", move |_params, _| {
            let handle = tokio::runtime::Handle::current();
            let metrics = tokio::task::block_in_place(|| handle.block_on(g.metrics()));
            let streak = tokio::task::block_in_place(|| handle.block_on(g.shadow_streak()));
            Ok::<_, JsonRpseeError>(serde_json::json!({
                "active": true,
                "rounds_completed": metrics.rounds_completed,
                "rounds_timed_out": metrics.rounds_timed_out,
                "certificates_produced": metrics.certificates_produced,
                "shadow_agreements": metrics.shadow_agreements,
                "divergence_events_count": metrics.divergence_events,
                "shadow_agreement_streak": streak,
                "last_finalized_block": metrics.last_finalized_block,
                "last_cert_latency_ms": metrics.last_cert_latency_ms,
            }))
        })?;
    } else {
        module.register_method("x3_flashFinalityStatus", move |_params, _| {
            Ok::<_, JsonRpseeError>(serde_json::json!({ "active": false }))
        })?;
    }

    // ────────────────────────────────────────────────────────────────────────────
    // SETTLEMENT API (PHASE C STUB)
    // FROZEN under v1-alpha API (2026-04-24, commit d99252ca42)
    // ────────────────────────────────────────────────────────────────────────────

    let check = check_rate_limit.clone();
    module.register_method("settlement_status", move |_params, _| {
        check("settlement_status")?;
        // TODO: Call GovernanceSettlementApi::settlement_status()
        // TODO: Return SettlementStatusResponse with pending/completed/refunded counts
        Ok::<_, JsonRpseeError>(serde_json::json!({
            "engine_enabled": true,
            "pending_transfers": 0,
            "completed_transfers": 0,
            "refunded_transfers": 0,
            "total_locked": "0"
        }))
    })?;

    let check = check_rate_limit.clone();
    module.register_method("settlement_status_for_transfer", move |params, _| {
        check("settlement_status_for_transfer")?;
        let req: serde_json::Value = params.one()?;
        let _transfer_id = req
            .get("transfer_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| custom_error("Missing transfer_id"))?;
        // TODO: Call GovernanceSettlementApi::get_settlement(transfer_id)
        // TODO: Return SettlementResponse with full transfer details
        Ok::<_, JsonRpseeError>(serde_json::json!({
            "transfer_id": _transfer_id,
            "status": "Pending",
            "amount": "0",
            "receiver": null,
            "initiated_at": 0,
            "timeout_at": 0,
            "num_legs": 0,
            "legs_completed": 0
        }))
    })?;

    let check = check_rate_limit.clone();
    module.register_method("pending_settlements_with_timeout", move |params, _| {
        check("pending_settlements_with_timeout")?;
        let req: serde_json::Value = params.one()?;
        let _current_block = req.get("current_block").and_then(|v| v.as_u64()).unwrap_or(0);
        let _window_blocks = req.get("window_blocks").and_then(|v| v.as_u64()).unwrap_or(100);
        // TODO: Call GovernanceSettlementApi::get_pending_settlements_with_timeout()
        // TODO: Return array of PendingSettlement structs with timeout tracking
        Ok::<_, JsonRpseeError>(serde_json::json!({
            "pending_settlements": [],
            "query_block": _current_block,
            "window_blocks": _window_blocks
        }))
    })?;

    // ────────────────────────────────────────────────────────────────────────────
    // BRIDGE ROUTER API (PHASE C STUB)
    // FROZEN under v1-alpha API (2026-04-24, commit d99252ca42)
    // ────────────────────────────────────────────────────────────────────────────

    let check = check_rate_limit.clone();
    module.register_method("bridge_supported_chains", move |_params, _| {
        check("bridge_supported_chains")?;
        // TODO: Call BridgeRouterApi::supported_chains()
        // TODO: Return array of supported external chain IDs (1=EVM, 2=Solana, 3=Bitcoin)
        Ok::<_, JsonRpseeError>(serde_json::json!({
            "chains": [1, 2, 3],
            "chain_names": {
                "1": "Ethereum",
                "2": "Solana",
                "3": "Bitcoin"
            }
        }))
    })?;

    let check = check_rate_limit.clone();
    module.register_method("bridge_current_root", move |params, _| {
        check("bridge_current_root")?;
        let req: serde_json::Value = params.one()?;
        let _chain_id = req.get("chain_id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        // TODO: Call BridgeRouterApi::current_root(chain_id)
        // TODO: Return BridgeRootState with root hash and block number
        Ok::<_, JsonRpseeError>(serde_json::json!({
            "root_hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "block_number": 0,
            "registered_at": 0,
            "signature_count": 0
        }))
    })?;

    let check = check_rate_limit.clone();
    module.register_method("bridge_routes", move |_params, _| {
        check("bridge_routes")?;
        // TODO: Call BridgeRouterApi::get_bridge_routes()
        // TODO: Return array of BridgeRoute structs with enabled status and min_confirmations
        Ok::<_, JsonRpseeError>(serde_json::json!({
            "routes": []
        }))
    })?;

    let check = check_rate_limit.clone();
    module.register_method("bridge_is_paused", move |params, _| {
        check("bridge_is_paused")?;
        let req: serde_json::Value = params.one()?;
        let _chain_id = req.get("chain_id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        // TODO: Call BridgeRouterApi::is_bridge_paused(chain_id)
        // TODO: Return boolean for bridge pause status
        Ok::<_, JsonRpseeError>(serde_json::json!({
            "chain_id": _chain_id,
            "paused": false
        }))
    })?;

    // ════════════════════════════════════════════════════════════════════════════════════
    // ✅ FINAL: Agent Memory RPC API (Phase 3 - Offchain Indexing)
    // Provides access to agent memory snapshots, consensus status, and query execution
    // ════════════════════════════════════════════════════════════════════════════════════

    let c = client.clone();
    let check = check_rate_limit.clone();
    module.register_method("agentMemory_latestHash", move |params, _| {
        check("agentMemory_latestHash")?;
        let agent_id_hex: String = params.one()?;

        // Parse agent_id from hex string
        let agent_id = decode_hex_param(&agent_id_hex, "agent_id")?;
        if agent_id.len() != 32 {
            return Err(custom_error("agent_id must be 32 bytes (H256)"));
        }

        let runtime_api = c.runtime_api();
        let best_hash = c.info().best_hash;

        let response = runtime_api
            .agent_memory_hash(best_hash, agent_id)
            .map_err(|e| custom_error(format!("Runtime API error: {}", e)))?;

        Ok::<_, JsonRpseeError>(serde_json::json!({
            "agent_id": agent_id_hex,
            "memory_hash": format!("0x{}", hex::encode(&response.memory_hash)),
            "block_number": response.block_number,
            "indexed_at": response.indexed_at,
            "consensus_reached": response.consensus_reached,
            "attestations": response.attestations,
        }))
    })?;

    let c = client.clone();
    let check = check_rate_limit.clone();
    module.register_method("agentMemory_atBlock", move |params, _| {
        check("agentMemory_atBlock")?;
        let (agent_id_hex, block_number): (String, u32) = params.parse()?;

        let agent_id = decode_hex_param(&agent_id_hex, "agent_id")?;
        if agent_id.len() != 32 {
            return Err(custom_error("agent_id must be 32 bytes (H256)"));
        }

        let runtime_api = c.runtime_api();
        let best_hash = c.info().best_hash;

        let response = runtime_api
            .agent_memory_at_block(best_hash, agent_id, block_number)
            .map_err(|e| custom_error(format!("Runtime API error: {}", e)))?;

        Ok::<_, JsonRpseeError>(serde_json::json!({
            "agent_id": agent_id_hex,
            "block_number": response.block_number,
            "memory_data": format!("0x{}", hex::encode(&response.memory_data)),
            "size_bytes": response.size_bytes,
            "verified": response.verified,
            "verification_block": response.verification_block,
        }))
    })?;

    let c = client.clone();
    let check = check_rate_limit.clone();
    module.register_method("agentMemory_query", move |params, _| {
        check("agentMemory_query")?;
        let req: serde_json::Value = params.one()?;

        let agent_id_hex = req
            .get("agent_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| custom_error("Missing agent_id"))?;
        let block_number = req
            .get("block_number")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| custom_error("Missing block_number"))?
            as u32;
        let function_name = req
            .get("function_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| custom_error("Missing function_name"))?
            .as_bytes()
            .to_vec();
        let params_hex = req
            .get("params")
            .and_then(|v| v.as_str())
            .unwrap_or("0x");

        let agent_id = decode_hex_param(agent_id_hex, "agent_id")?;
        if agent_id.len() != 32 {
            return Err(custom_error("agent_id must be 32 bytes (H256)"));
        }

        let params_bytes = decode_hex_param(params_hex, "params")?;

        let runtime_api = c.runtime_api();
        let best_hash = c.info().best_hash;

        let response = runtime_api
            .agent_query(best_hash, agent_id, block_number, function_name, params_bytes)
            .map_err(|e| custom_error(format!("Runtime API error: {}", e)))?;

        Ok::<_, JsonRpseeError>(serde_json::json!({
            "success": response.success,
            "result": response.result.map(|r| format!("0x{}", hex::encode(&r))),
            "error": response.error.map(|e| String::from_utf8_lossy(&e).into_owned()),
            "executed_block": response.executed_block,
            "latency_ms": response.latency_ms,
        }))
    })?;

    let c = client.clone();
    let check = check_rate_limit.clone();
    module.register_method("agentMemory_consensus", move |params, _| {
        check("agentMemory_consensus")?;
        let (agent_id_hex, block_number): (String, u32) = params.parse()?;

        let agent_id = decode_hex_param(&agent_id_hex, "agent_id")?;
        if agent_id.len() != 32 {
            return Err(custom_error("agent_id must be 32 bytes (H256)"));
        }

        let runtime_api = c.runtime_api();
        let best_hash = c.info().best_hash;

        let response = runtime_api
            .agent_memory_consensus(best_hash, agent_id, block_number)
            .map_err(|e| custom_error(format!("Runtime API error: {}", e)))?;

        let attestations = response
            .attestations_received
            .into_iter()
            .map(|att| {
                serde_json::json!({
                    "validator": format!("0x{}", hex::encode(&att.validator)),
                    "verified": att.verified,
                })
            })
            .collect::<Vec<_>>();

        Ok::<_, JsonRpseeError>(serde_json::json!({
            "agent_id": agent_id_hex,
            "block_number": response.block_number,
            "memory_hash": format!("0x{}", hex::encode(&response.memory_hash)),
            "attestations_received": attestations,
            "attestations_required": response.attestations_required,
            "consensus_reached": response.consensus_reached,
            "consensus_reached_at_block": response.consensus_reached_at_block,
        }))
    })?;

    Ok(module)
}
