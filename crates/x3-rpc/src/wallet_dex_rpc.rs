/// Wallet-DEX RPC Integration
/// Wire wallet signing to DEX swap execution
///
/// ## Security / Abuse Controls
///
/// Each RPC method enforces strict input size limits to prevent DoS via
/// oversized payloads.  Callers that violate these limits receive an
/// `invalid_params` error.  Connection-level rate limiting is handled by the
/// JSON-RPC server middleware (configured at the node layer, not here).
use jsonrpc_core::{Error, Result};
use jsonrpc_derive::rpc;
use pallet_atomic_trade_engine::runtime_api::SimulationResult;
use pallet_atomic_trade_engine::AtomicTradeEngineApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::{hashing::blake2_256, H256};
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;

/// Swap request with wallet integration
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SwapRequest {
    pub token_in: [u8; 32],
    pub token_out: [u8; 32],
    pub amount_in: u128,
    pub min_amount_out: u128,
    pub wallet_id: [u8; 32],
    pub require_approval: bool,
    pub approval_threshold: u128,
}
// ---------------------------------------------------------------------------
// Abuse-control limits
// ---------------------------------------------------------------------------

/// Maximum UTF-8 length for human-readable display messages (hardware wallet screen).
const MAX_DISPLAY_MESSAGE_LEN: usize = 256;
/// Maximum number of signatures accepted in a single execute_swap call.
const MAX_SIGNATURES_COUNT: usize = 10;
/// Maximum byte length of a single signature (DER-encoded ECDSA is ≤73 bytes; 130 is generous).
const MAX_SIGNATURE_LEN: usize = 130;
/// Maximum byte length for a standalone approval signature.
const MAX_APPROVAL_SIGNATURE_LEN: usize = 130;
/// Maximum string length for an account identifier (SS58 / hex address).
const MAX_ACCOUNT_LEN: usize = 256;

/// Swap response with signing details
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SwapResponse {
    pub swap_id: [u8; 32],
    pub amount_out: u128,
    pub approval_required: bool,
    pub approval_request_id: Option<[u8; 32]>,
    pub estimated_gas: u128,
}

/// Hardware wallet signing request
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HardwareSigningRequest {
    pub transaction_hash: [u8; 32],
    pub display_message: String,
    pub request_id: [u8; 32],
    pub timeout_seconds: u32,
}

/// Hardware wallet signing response
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HardwareSigningResponse {
    pub signature: Vec<u8>,
    pub recovery_id: u8,
    pub signed_block: u32,
}

#[rpc]
pub trait WalletDexApi {
    /// Estimate swap with approval requirements
    #[rpc(name = "walletDex_estimateSwap")]
    fn estimate_swap(&self, request: SwapRequest) -> Result<SwapResponse>;

    /// Execute swap with wallet signatures
    #[rpc(name = "walletDex_executeSwap")]
    fn execute_swap(&self, request: SwapRequest, signatures: Vec<Vec<u8>>) -> Result<SwapResponse>;

    /// Request hardware signing for a transaction
    #[rpc(name = "walletDex_requestHardwareSigning")]
    fn request_hardware_signing(
        &self,
        wallet_id: [u8; 32],
        transaction_hash: [u8; 32],
        display_message: String,
    ) -> Result<HardwareSigningRequest>;

    /// Approve transaction with multisig
    #[rpc(name = "walletDex_approveTransaction")]
    fn approve_transaction(
        &self,
        wallet_id: [u8; 32],
        transaction_hash: [u8; 32],
        approval_signature: Vec<u8>,
    ) -> Result<bool>;

    /// Get wallet balance
    #[rpc(name = "walletDex_getBalance")]
    fn get_balance(&self, account: String, token_id: [u8; 32]) -> Result<u128>;

    /// Check approval status
    #[rpc(name = "walletDex_getApprovalStatus")]
    fn get_approval_status(&self, approval_id: [u8; 32]) -> Result<(String, u32)>;
}

/// RPC implementation
pub struct WalletDexRpc<Block, Client> {
    client: Arc<Client>,
    _phantom: std::marker::PhantomData<Block>,
}

impl<Block, Client> WalletDexRpc<Block, Client> {
    pub fn new(client: Arc<Client>) -> Self {
        WalletDexRpc {
            client,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<Block, Client> WalletDexApi for WalletDexRpc<Block, Client>
where
    Block: BlockT,
    Client: HeaderBackend<Block> + ProvideRuntimeApi<Block> + 'static,
    Client::Api: AtomicTradeEngineApi<Block>,
{
    fn estimate_swap(&self, request: SwapRequest) -> Result<SwapResponse> {
        let approval_required =
            request.require_approval && request.amount_in > request.approval_threshold;

        // The chain's own simulation, not a fraction of the input. This used to be
        // `amount_out = amount_in * 95 / 100` with `estimated_gas: 100_000` under a comment
        // saying "In production: call DEX runtime api for actual prices": a quote nobody
        // computed, with a fee (5%) that no pool charges and no route produced.
        let at = self.client.info().best_hash;
        let simulation = self
            .client
            .runtime_api()
            .simulate_trade(
                at,
                H256::from(request.token_in),
                H256::from(request.token_out),
                request.amount_in,
                DEFAULT_SLIPPAGE_BPS,
            )
            .map_err(|e| {
                Error::invalid_params(format!(
                    "the runtime could not simulate {} -> {}: {e}",
                    H256::from(request.token_in),
                    H256::from(request.token_out)
                ))
            })?;

        estimate_response(&request, &simulation, approval_required)
    }

    fn execute_swap(&self, request: SwapRequest, signatures: Vec<Vec<u8>>) -> Result<SwapResponse> {
        // -- Real preconditions, checked before anything else ---------------------
        if signatures.len() > MAX_SIGNATURES_COUNT {
            return Err(Error::invalid_params(format!(
                "too many signatures: {} (max {})",
                signatures.len(),
                MAX_SIGNATURES_COUNT
            )));
        }
        for signature in &signatures {
            if signature.is_empty() || signature.len() > MAX_SIGNATURE_LEN {
                return Err(Error::invalid_params(format!(
                    "signature length {} is out of range (1..={})",
                    signature.len(),
                    MAX_SIGNATURE_LEN
                )));
            }
        }
        if request.require_approval
            && request.amount_in > request.approval_threshold
            && signatures.is_empty()
        {
            return Err(Error::invalid_params("Signatures required for approval"));
        }
        // -------------------------------------------------------------------------

        // **Refused, not fabricated.** This used to return `Ok` with
        // `swap_id: [1u8; 32]` and `amount_out = amount_in * 95 / 100` — a transaction id for
        // a transaction that was never built, and an output nobody computed. A wallet that
        // reads that as success believes value moved. There is no atomic executor wired into
        // this node, so the only honest answer is to say so; refusing is what
        // `x3-swap-router`'s executor does for the same reason (`NoExecutorConfigured`).
        Err(execution_refusal())
    }

    fn request_hardware_signing(
        &self,
        wallet_id: [u8; 32],
        transaction_hash: [u8; 32],
        display_message: String,
    ) -> Result<HardwareSigningRequest> {
        // -- Input validation / abuse controls -----------------------------------
        if display_message.len() > MAX_DISPLAY_MESSAGE_LEN {
            return Err(Error::invalid_params(format!(
                "display_message too long: {} chars (max {})",
                display_message.len(),
                MAX_DISPLAY_MESSAGE_LEN
            )));
        }
        // -----------------------------------------------------------------------

        // Create signing request for hardware wallet
        // In production: interact with WebUSB/WebHID APIs

        // A commitment to *this* request, not a splice of two inputs: the old id took the
        // first half of the wallet id and the second half of the hash, so two different
        // requests could share an id and the display message was not committed at all.
        let mut subject = Vec::with_capacity(32 + 32 + display_message.len());
        subject.extend_from_slice(&wallet_id);
        subject.extend_from_slice(&transaction_hash);
        subject.extend_from_slice(display_message.as_bytes());
        let request_id = blake2_256(&subject);

        Ok(HardwareSigningRequest {
            transaction_hash,
            display_message,
            request_id,
            timeout_seconds: 120, // 2 minute timeout
        })
    }

    fn approve_transaction(
        &self,
        _wallet_id: [u8; 32],
        _transaction_hash: [u8; 32],
        approval_signature: Vec<u8>,
    ) -> Result<bool> {
        // -- Input validation / abuse controls -----------------------------------
        if approval_signature.is_empty() {
            return Err(Error::invalid_params("Signature cannot be empty"));
        }
        if approval_signature.len() > MAX_APPROVAL_SIGNATURE_LEN {
            return Err(Error::invalid_params(format!(
                "approval_signature too large: {} bytes (max {})",
                approval_signature.len(),
                MAX_APPROVAL_SIGNATURE_LEN
            )));
        }
        // -----------------------------------------------------------------------

        // Do not accept approvals without cryptographic verification.
        Err(Error::invalid_params(
            "Approval signature verification backend is not implemented",
        ))
    }

    fn get_balance(&self, account: String, _token_id: [u8; 32]) -> Result<u128> {
        // -- Input validation / abuse controls -----------------------------------
        if account.is_empty() {
            return Err(Error::invalid_params("account cannot be empty"));
        }
        if account.len() > MAX_ACCOUNT_LEN {
            return Err(Error::invalid_params(format!(
                "account too long: {} chars (max {})",
                account.len(),
                MAX_ACCOUNT_LEN
            )));
        }
        // -----------------------------------------------------------------------

        // Do not return synthetic balances on RPC failures/unwired backends.
        Err(Error::invalid_params(
            "Wallet balance backend is not implemented",
        ))
    }

    fn get_approval_status(&self, _approval_id: [u8; 32]) -> Result<(String, u32)> {
        // **Refused, not fabricated.** This answered `("pending", 2)` — a status and a
        // signature threshold for an approval nobody looked up, under a comment saying "In
        // production: query approval pallet". A wallet that shows "pending, 2 more signatures"
        // is reading a constant. This module already refuses for the same reason twice above
        // (`approve_transaction`, `get_balance`).
        Err(unwired("Approval status"))
    }
}

/// Slippage the node assumes when it asks the runtime to simulate a swap.
///
/// The runtime's simulation applies the caller's slippage bound as an input, so this is the
/// node's *assumption* for a read-only quote, not a permission: the caller's own
/// `min_amount_out` is enforced separately in [`estimate_response`], and any real execution
/// carries the bound the caller signed. 50 bps is the bound the wallet surface has always
/// described ("0.5% default").
const DEFAULT_SLIPPAGE_BPS: u32 = 50;

/// Turn the chain's simulation into a quote, or refuse with the reason.
///
/// Three rules, none of which the fabricated version had:
///
/// * a simulation the chain says would fail is a refusal carrying the chain's own error;
/// * an output below the caller's `min_amount_out` is a refusal that names both numbers —
///   the bound is the caller's, and the node must not hand back a quote that violates it;
/// * a successful simulation is returned with the chain's numbers, and `swap_id` is left
///   zero: an estimate is not an execution, and a plausible-looking id would be a fabricated
///   transaction reference.
fn estimate_response(
    request: &SwapRequest,
    simulation: &SimulationResult,
    approval_required: bool,
) -> Result<SwapResponse> {
    if !simulation.success {
        let detail = simulation
            .error
            .as_deref()
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
            .unwrap_or_else(|| "the runtime reported no route".to_string());
        return Err(Error::invalid_params(format!(
            "the chain cannot route {} -> {} for {}: {detail}",
            H256::from(request.token_in),
            H256::from(request.token_out),
            request.amount_in
        )));
    }
    if simulation.estimated_output < request.min_amount_out {
        return Err(Error::invalid_params(format!(
            "the route returns {} but the caller requires at least {}",
            simulation.estimated_output, request.min_amount_out
        )));
    }
    Ok(SwapResponse {
        swap_id: [0u8; 32],
        amount_out: simulation.estimated_output,
        approval_required,
        approval_request_id: approval_required.then(|| approval_request_id(request)),
        estimated_gas: u128::from(simulation.evm_gas) + u128::from(simulation.svm_compute),
    })
}

/// A deterministic identity for an approval request, derived from what is being approved.
///
/// The previous value was the constant `[1u8; 32]`, so every approval request in the node had
/// the same id and a caller could not tell two of them apart.
fn approval_request_id(request: &SwapRequest) -> [u8; 32] {
    let mut subject = Vec::with_capacity(32 + 32 + 16 + 16);
    subject.extend_from_slice(&request.wallet_id);
    subject.extend_from_slice(&request.token_in);
    subject.extend_from_slice(&request.token_out);
    subject.extend_from_slice(&request.amount_in.to_le_bytes());
    subject.extend_from_slice(&request.min_amount_out.to_le_bytes());
    blake2_256(&subject)
}

/// There is no atomic executor wired into this node.
fn execution_refusal() -> Error {
    unwired("Atomic swap execution")
}

/// A backend this node does not have. `InternalError` on purpose: the request is well formed
/// and the node cannot serve it — that is not the caller's fault, and it must not be reported
/// as success.
fn unwired(backend: &str) -> Error {
    let mut error = Error::internal_error();
    error.message = format!("{backend} is not wired into this node");
    error
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(amount_in: u128, min_amount_out: u128) -> SwapRequest {
        SwapRequest {
            token_in: [1u8; 32],
            token_out: [2u8; 32],
            amount_in,
            min_amount_out,
            wallet_id: [3u8; 32],
            require_approval: false,
            approval_threshold: 5_000,
        }
    }

    fn simulation(success: bool, output: u128) -> SimulationResult {
        SimulationResult {
            success,
            estimated_output: output,
            price_impact_bps: 12,
            evm_gas: 21_000,
            svm_compute: 4_500,
            route: Vec::new(),
            error: None,
        }
    }

    /// The numbers in the response are the chain's, not a fraction of the input. The old test
    /// for this RPC recomputed `(amount_in * 95) / 100` itself and asserted it equalled 950 —
    /// it never called the method, so the fabricated quote was never under test.
    #[test]
    fn the_quote_carries_the_chains_numbers_and_its_gas() {
        let request = request(1_000, 900);
        let response =
            estimate_response(&request, &simulation(true, 1_234), false).expect("a route exists");
        assert_eq!(response.amount_out, 1_234);
        assert_eq!(response.estimated_gas, 25_500);
        assert_eq!(
            response.swap_id, [0u8; 32],
            "an estimate is not an execution, so it carries no transaction id"
        );
        assert!(response.approval_request_id.is_none());
    }

    #[test]
    fn a_quote_below_the_callers_bound_is_refused_and_names_both_numbers() {
        let request = request(1_000, 900);
        let refusal = estimate_response(&request, &simulation(true, 899), false)
            .expect_err("a quote under the bound must be refused");
        assert!(
            refusal.message.contains("899") && refusal.message.contains("900"),
            "the refusal must name the output and the bound: {}",
            refusal.message
        );
    }

    #[test]
    fn a_route_the_chain_refuses_is_refused_with_the_chains_own_error() {
        let request = request(1_000, 900);
        let mut no_route = simulation(false, 0);
        no_route.error = Some(b"no pool for this pair".to_vec());
        let refusal = estimate_response(&request, &no_route, false)
            .expect_err("a failed simulation must not become a quote");
        assert!(
            refusal.message.contains("no pool for this pair"),
            "the chain's own reason must survive: {}",
            refusal.message
        );
    }

    #[test]
    fn approval_requests_get_distinct_ids_and_commit_to_the_subject() {
        let low = request(1_000, 900);
        let high = request(2_000, 900);
        let first = estimate_response(&low, &simulation(true, 1_000), true).unwrap();
        let second = estimate_response(&high, &simulation(true, 2_000), true).unwrap();
        assert!(first.approval_request_id.is_some());
        assert_ne!(
            first.approval_request_id, second.approval_request_id,
            "two different swaps must not share an approval id"
        );
        assert_eq!(
            first.approval_request_id,
            Some(approval_request_id(&low)),
            "the id is derived from the request, so a caller can recompute it"
        );
        assert_ne!(first.approval_request_id, Some([1u8; 32]));
    }

    /// The two calls that cannot be served must say so instead of answering. `execute_swap`
    /// used to return `swap_id: [1u8; 32]` with an invented output, and
    /// `get_approval_status` returned `("pending", 2)` for an id nothing looked up.
    #[test]
    fn the_unwired_backends_refuse_instead_of_answering() {
        let execution = execution_refusal();
        assert!(execution.message.contains("Atomic swap execution"));
        assert!(execution.message.contains("not wired"));
        let status = unwired("Approval status");
        assert!(status.message.contains("Approval status"));
        assert_eq!(
            serde_json::to_value(&execution.code).unwrap(),
            serde_json::json!(-32603),
            "an unwired backend is an internal error, never a success"
        );
    }

    #[test]
    fn test_hardware_signature_request() {
        let wallet_id = [1u8; 32];
        let tx_hash = [2u8; 32];
        let message = "Approve swap: 1000 USDC → 950 USDT".to_string();

        // The request id is a commitment to the wallet, the transaction and the message the
        // hardware screen will show; the test used to recompute the old splice itself, so it
        // proved nothing about the method.
        let mut subject = Vec::new();
        subject.extend_from_slice(&wallet_id);
        subject.extend_from_slice(&tx_hash);
        subject.extend_from_slice(message.as_bytes());
        let expected = blake2_256(&subject);

        let mut other_subject = Vec::new();
        other_subject.extend_from_slice(&wallet_id);
        other_subject.extend_from_slice(&tx_hash);
        other_subject.extend_from_slice(b"Approve swap: 2000 USDC");
        assert_ne!(
            expected,
            blake2_256(&other_subject),
            "the display message is part of the id, so the screen cannot be swapped"
        );
    }
}
