//! X3 Chain node RPC module wiring.
//!
//! Assembles the full JSON-RPC module used by the service.
//! Merges substrate system RPCs, transaction-payment RPCs, and the
//! Frontier-compatible ETH/SVM RPC provided by `rpc_frontier`.

use flash_finality::FlashFinalityGadget;
use jsonrpsee::RpcModule;
use sc_client_api::BlockBackend;
use sc_rpc_api::DenyUnsafe;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use std::sync::Arc;
use x3_chain_runtime::{opaque::Block, AccountId, AssetId, Balance};

use crate::rpc_middleware::RateLimiter;
use crate::service::FullClient;

type RpcError = Box<dyn std::error::Error + Send + Sync>;

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

    Ok(module)
}
