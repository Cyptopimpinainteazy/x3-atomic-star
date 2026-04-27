//! X3 Chain node RPC module wiring.
//!
//! Assembles the full JSON-RPC module used by the service.

use flash_finality::FlashFinalityGadget;
use jsonrpsee::RpcModule;
use sc_rpc_api::DenyUnsafe;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use std::sync::Arc;
use x3_chain_runtime::opaque::Block;

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
    <FullClient as ProvideRuntimeApi<Block>>::Api: BlockBuilder<Block>,
    <FullClient as ProvideRuntimeApi<Block>>::Api:
        substrate_frame_rpc_system::AccountNonceApi<Block, x3_chain_runtime::AccountId, u32>,
    <FullClient as ProvideRuntimeApi<Block>>::Api:
        pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<
            Block,
            x3_chain_runtime::Balance,
        >,
{
    let mut module = RpcModule::new(());

    let system_rpc = substrate_frame_rpc_system::System::new(client.clone(), pool, deny_unsafe);
    module.merge(substrate_frame_rpc_system::SystemApiServer::into_rpc(system_rpc))?;

    let tx_payment_rpc = pallet_transaction_payment_rpc::TransactionPayment::new(client.clone());
    module.merge(
        pallet_transaction_payment_rpc::TransactionPaymentApiServer::into_rpc(tx_payment_rpc),
    )?;

    Ok(module)
}
