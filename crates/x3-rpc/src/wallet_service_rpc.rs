/// Wallet Service RPC - Phase 3 Implementation
///
/// The endpoint surface a wallet frontend calls: create/import/backup a wallet,
/// read balances, sign and submit a transaction, list history and status, pick a
/// network.
///
/// # State, 2026-09-27: every method refuses
///
/// This module used to answer all of those with invented values, and the worst of
/// them was `create_wallet`, which returned the **public test mnemonic**
/// (`"test test test … test"`, twelve words) and an address built by slicing that
/// string: `format!("0x{}", &mnemonic[0..40])`. A wallet that trusted it would
/// hand a user an address nobody holds a key for and a seed phrase the whole world
/// knows. The rest were the same shape — `sign_transaction` returned "signature:
/// the first 130 characters of the transaction data", `submit_transaction` a hash
/// sliced out of its own input string, `get_balance` a hardcoded 1250 X3 and 2.45
/// ETH, `get_transactions` two invented transactions at invented block heights,
/// `list_wallets` two invented wallets, `get_wallet_status` "synced at block
/// 1234567" for any id, `get_networks` three invented endpoints.
///
/// None of that is fixable in the node: a node does not hold a user's keys, has no
/// keystore, no wallet index and no transaction-history store, and the runtime API
/// this module has in scope (`AtlasKernelRuntimeApi`) has no balance query. So the
/// honest state is a refusal that names what is missing, which is also what
/// `crates/x3-rpc/src/wallet_dex_rpc.rs` and `crates/x3-wallet`'s hardware
/// verifier already do. The input validation each method had is kept, so a
/// malformed request still gets the precise parameter error.
use jsonrpc_core::{Error, Result};
use jsonrpc_derive::rpc;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;

// ============================================================================
// Request/Response Types
// ============================================================================

/// Wallet creation request
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CreateWalletRequest {
    pub wallet_name: String,
    pub password_hash: String,
    pub mnemonic: Option<String>,
    pub network: String,
}

/// Wallet creation response
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CreateWalletResponse {
    pub wallet_id: String,
    pub address: String,
    pub mnemonic: Option<String>,
    pub created_at: u64,
}

/// Wallet import request
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ImportWalletRequest {
    pub mnemonic: String,
    pub password_hash: String,
    pub wallet_name: Option<String>,
    pub network: String,
}

/// Wallet backup request
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BackupWalletRequest {
    pub wallet_id: String,
    pub password_hash: String,
}

/// Wallet backup response
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BackupWalletResponse {
    pub backup_data: String,
    pub backup_hash: String,
    pub timestamp: u64,
}

/// Get wallet balance request
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GetBalanceRequest {
    pub wallet_id: String,
    pub token_id: Option<String>,
    pub network: String,
}

/// Token balance information
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TokenBalance {
    pub token_id: String,
    pub symbol: String,
    pub name: String,
    pub balance: String,
    pub decimals: u32,
    pub value_usd: Option<String>,
    pub network: String,
}

/// Get wallet balance response
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GetBalanceResponse {
    pub wallet_id: String,
    pub total_balance_usd: Option<String>,
    pub tokens: Vec<TokenBalance>,
}

/// Transaction signing request
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SignTransactionRequest {
    pub wallet_id: String,
    pub password_hash: String,
    pub transaction_data: String,
    pub network: String,
}

/// Transaction signing response
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SignTransactionResponse {
    pub signature: String,
    pub signed_transaction: String,
    pub transaction_hash: String,
    pub timestamp: u64,
}

/// Submit transaction request
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SubmitTransactionRequest {
    pub signed_transaction: String,
    pub network: String,
}

/// Submit transaction response
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SubmitTransactionResponse {
    pub transaction_hash: String,
    pub block_hash: Option<String>,
    pub status: String,
    pub timestamp: u64,
}

/// Get wallet transactions request
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GetTransactionsRequest {
    pub wallet_id: String,
    pub network: String,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

/// Transaction information
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TransactionInfo {
    pub hash: String,
    pub from: String,
    pub to: String,
    pub amount: String,
    pub token: String,
    pub status: String,
    pub block_number: Option<u64>,
    pub timestamp: u64,
    pub fee: Option<String>,
}

/// Get wallet transactions response
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GetTransactionsResponse {
    pub wallet_id: String,
    pub transactions: Vec<TransactionInfo>,
    pub total_count: u32,
    pub page: u32,
    pub page_size: u32,
}

/// Get wallet status request
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GetWalletStatusRequest {
    pub wallet_id: String,
}

/// Wallet status information
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WalletStatus {
    pub wallet_id: String,
    pub is_connected: bool,
    pub network: String,
    pub last_sync_block: u64,
    pub sync_status: String,
    pub balance_updated_at: u64,
}

/// Get wallet status response
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GetWalletStatusResponse {
    pub status: WalletStatus,
}

/// List wallets request
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ListWalletsRequest {
    pub network: Option<String>,
}

/// Wallet summary
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WalletSummary {
    pub wallet_id: String,
    pub name: String,
    pub address: String,
    pub network: String,
    pub created_at: u64,
    pub last_active: u64,
    pub total_balance_usd: Option<String>,
}

/// List wallets response
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ListWalletsResponse {
    pub wallets: Vec<WalletSummary>,
    pub total_count: u32,
}

/// Network configuration
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NetworkConfig {
    pub name: String,
    pub chain_id: u64,
    pub rpc_url: String,
    pub ws_url: Option<String>,
    pub explorer_url: Option<String>,
    pub is_testnet: bool,
}

/// Set network request
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SetNetworkRequest {
    pub wallet_id: String,
    pub network: String,
}

/// Set network response
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SetNetworkResponse {
    pub wallet_id: String,
    pub network: String,
    pub success: bool,
}

// ============================================================================
// Wallet Service API Trait
// ============================================================================

#[rpc]
pub trait WalletServiceApi {
    /// Create a new wallet
    #[rpc(name = "wallet_createWallet")]
    fn create_wallet(&self, request: CreateWalletRequest) -> Result<CreateWalletResponse>;

    /// Import an existing wallet from mnemonic
    #[rpc(name = "wallet_importWallet")]
    fn import_wallet(&self, request: ImportWalletRequest) -> Result<CreateWalletResponse>;

    /// Backup wallet data
    #[rpc(name = "wallet_backupWallet")]
    fn backup_wallet(&self, request: BackupWalletRequest) -> Result<BackupWalletResponse>;

    /// Get wallet balance
    #[rpc(name = "wallet_getBalance")]
    fn get_balance(&self, request: GetBalanceRequest) -> Result<GetBalanceResponse>;

    /// Sign a transaction
    #[rpc(name = "wallet_signTransaction")]
    fn sign_transaction(&self, request: SignTransactionRequest) -> Result<SignTransactionResponse>;

    /// Submit a signed transaction
    #[rpc(name = "wallet_submitTransaction")]
    fn submit_transaction(
        &self,
        request: SubmitTransactionRequest,
    ) -> Result<SubmitTransactionResponse>;

    /// Get transaction history
    #[rpc(name = "wallet_getTransactions")]
    fn get_transactions(&self, request: GetTransactionsRequest) -> Result<GetTransactionsResponse>;

    /// Get wallet status
    #[rpc(name = "wallet_getWalletStatus")]
    fn get_wallet_status(&self, request: GetWalletStatusRequest)
        -> Result<GetWalletStatusResponse>;

    /// List all wallets
    #[rpc(name = "wallet_listWallets")]
    fn list_wallets(&self, request: ListWalletsRequest) -> Result<ListWalletsResponse>;

    /// Set network for wallet
    #[rpc(name = "wallet_setNetwork")]
    fn set_network(&self, request: SetNetworkRequest) -> Result<SetNetworkResponse>;

    /// Get available networks
    #[rpc(name = "wallet_getNetworks")]
    fn get_networks(&self) -> Result<Vec<NetworkConfig>>;
}

// ============================================================================
// Wallet Service RPC Implementation
// ============================================================================

/// Wallet Service RPC implementation
pub struct WalletServiceRpc<Block, Client> {
    client: Arc<Client>,
    _phantom: std::marker::PhantomData<Block>,
}

impl<Block, Client> WalletServiceRpc<Block, Client> {
    pub fn new(client: Arc<Client>) -> Self {
        WalletServiceRpc {
            client,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<Block, Client> WalletServiceApi for WalletServiceRpc<Block, Client>
where
    Block: BlockT,
    Client: HeaderBackend<Block> + 'static,
{
    fn create_wallet(&self, request: CreateWalletRequest) -> Result<CreateWalletResponse> {
        validate_create_wallet(&request)?;

        // **Refused, not fabricated.** This returned the public test mnemonic and an address
        // built by slicing it, so every "wallet" it created was the same, unspendable and
        // world-readable. Key generation and address derivation belong to the client (or to
        // `crates/x3-wallet` with a real KDF), and a node that claims to have done them is
        // lying about custody.
        Err(unwired("Wallet key generation and address derivation"))
    }

    fn import_wallet(&self, request: ImportWalletRequest) -> Result<CreateWalletResponse> {
        validate_import_wallet(&request)?;

        // **Refused, not fabricated.** Importing returned an address sliced out of the mnemonic
        // string, which is not a derivation of anything: the address did not correspond to the
        // seed the user imported.
        Err(unwired("Wallet address derivation from a mnemonic"))
    }

    fn backup_wallet(&self, request: BackupWalletRequest) -> Result<BackupWalletResponse> {
        // **Refused, not fabricated.** The "backup" was `format!("backup_{wallet_id}")` and its
        // "hash" was `format!("hash_{backup_data}")`: no key material, no encryption, and a
        // checksum that would change if the string did. A user who kept that file as a backup
        // would have kept nothing.
        let _ = &request.wallet_id;
        Err(unwired("Wallet keystore and encrypted backup"))
    }

    fn get_balance(&self, request: GetBalanceRequest) -> Result<GetBalanceResponse> {
        // **Refused, not fabricated.** This answered with 1250 X3 and 2.45 ETH, a USD valuation
        // for each, and a total — for any wallet id, from no source at all. A wallet UI showing
        // those numbers is showing constants. The runtime API in scope has no balance query, and
        // the node has no wallet-to-account mapping to look one up with.
        let _ = (&request.wallet_id, &request.token_id, &request.network);
        Err(unwired("Wallet balance query"))
    }

    fn sign_transaction(&self, request: SignTransactionRequest) -> Result<SignTransactionResponse> {
        validate_password_hash(&request.password_hash)?;

        // **Refused, not fabricated.** The "signature" was the first 130 characters of the
        // transaction data and the "hash" was the string `hash_signed_<data>`. A wallet that
        // submitted that would submit an unsigned transaction with a signature field full of the
        // message. A node must not sign for a user in any case: it holds no wallet keys.
        let _ = &request.transaction_data;
        Err(unwired("Wallet transaction signing"))
    }

    fn submit_transaction(
        &self,
        request: SubmitTransactionRequest,
    ) -> Result<SubmitTransactionResponse> {
        // **Refused, not fabricated.** It reported `status: "pending"` with a hash sliced out of
        // its own input string (`&signed_transaction[7..71]`) and never touched the transaction
        // pool. Nothing was submitted and nothing is pending. The submission surface a wallet
        // should use is `author_submitExtrinsic`.
        let _ = &request.signed_transaction;
        Err(unwired("Wallet transaction submission"))
    }

    fn get_transactions(&self, request: GetTransactionsRequest) -> Result<GetTransactionsResponse> {
        // **Refused, not fabricated.** This returned two transactions with made-up hashes
        // (`0x1234…` / `0xabcd…`), addresses, amounts and block heights, timestamped relative to
        // now, for any wallet id. Transaction history needs an indexer that this node does not
        // have; `chain_getBlock` and the explorer's own index are the real surfaces.
        let _ = (&request.wallet_id, request.page, request.page_size);
        Err(unwired("Wallet transaction history index"))
    }

    fn get_wallet_status(
        &self,
        request: GetWalletStatusRequest,
    ) -> Result<GetWalletStatusResponse> {
        // **Refused, not fabricated.** Every id got `is_connected: true`, `network: "mainnet"`,
        // `last_sync_block: 1234567`, `sync_status: "synced"`. A wallet showing "synced" at a
        // block that does not exist is worse than one showing nothing.
        let _ = &request.wallet_id;
        Err(unwired("Wallet status store"))
    }

    fn list_wallets(&self, request: ListWalletsRequest) -> Result<ListWalletsResponse> {
        // **Refused, not fabricated.** It returned two invented wallets ("Main Wallet", "Trading
        // Wallet") with balances, for every caller. A wallet list is the client's own store.
        let _ = &request.network;
        Err(unwired("Wallet keystore"))
    }

    fn set_network(&self, request: SetNetworkRequest) -> Result<SetNetworkResponse> {
        validate_network(&request.network)?;

        // **Refused, not fabricated.** The name was validated and then `success: true` was
        // returned for a selection nothing stored, so a wallet that had asked to switch to
        // testnet was told it had. Network selection is client state; the node can only report
        // which chain it is (that is what the system RPC methods are for).
        let _ = (&request.wallet_id, &request.network);
        Err(unwired("Wallet network selection store"))
    }

    fn get_networks(&self) -> Result<Vec<NetworkConfig>> {
        // **Refused, not fabricated.** The three entries were invented: chain ids 123456789/88/87
        // and endpoints (`rpc.x3chain.io`, `explorer.x3chain.io`) that this repository does not
        // configure anywhere. A wallet that took them would point users at hosts that may not
        // exist. The node's own identity is available from the system RPC methods.
        Err(unwired("Wallet network registry"))
    }
}

/// A backend this node does not have.
///
/// `InternalError` on purpose: the request is well formed and the node cannot serve it, which
/// is not the caller's fault and must never be reported as success.
fn unwired(backend: &str) -> Error {
    let mut error = Error::internal_error();
    error.message = format!("{backend} is not wired into this node");
    error
}

/// Wallet names are 1-64 characters.
fn validate_wallet_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 64 {
        return Err(Error::invalid_params("Wallet name must be 1-64 characters"));
    }
    Ok(())
}

/// The password hash the caller supplies must look like one (32 characters or more).
fn validate_password_hash(hash: &str) -> Result<()> {
    if hash.len() < 32 {
        return Err(Error::invalid_params(
            "Password hash must be at least 32 characters",
        ));
    }
    Ok(())
}

fn validate_create_wallet(request: &CreateWalletRequest) -> Result<()> {
    validate_wallet_name(&request.wallet_name)?;
    validate_password_hash(&request.password_hash)
}

fn validate_import_wallet(request: &ImportWalletRequest) -> Result<()> {
    let words = request.mnemonic.split_whitespace().count();
    if words != 12 && words != 24 {
        return Err(Error::invalid_params("Mnemonic must have 12 or 24 words"));
    }
    validate_password_hash(&request.password_hash)
}

/// The names the wallet surface accepts. Validated even though the selection cannot be stored:
/// a caller that misspells a network should hear that, not "not implemented".
fn validate_network(name: &str) -> Result<()> {
    const VALID_NETWORKS: [&str; 3] = ["mainnet", "testnet", "local"];
    if !VALID_NETWORKS.contains(&name) {
        return Err(Error::invalid_params(format!(
            "Invalid network. Must be one of: {}",
            VALID_NETWORKS.join(", ")
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create(name: &str, password: &str) -> CreateWalletRequest {
        CreateWalletRequest {
            wallet_name: name.to_string(),
            password_hash: password.to_string(),
            mnemonic: None,
            network: "local".to_string(),
        }
    }

    /// The validations that were always real stay real, so a malformed request gets the
    /// parameter error rather than "not implemented".
    #[test]
    fn validation_rejects_a_malformed_request_before_anything_else() {
        let long_name = create(&"x".repeat(65), &"p".repeat(32));
        assert!(validate_create_wallet(&long_name)
            .unwrap_err()
            .message
            .contains("1-64 characters"));

        let short_password = create("wallet", "short");
        assert!(validate_create_wallet(&short_password)
            .unwrap_err()
            .message
            .contains("at least 32"));

        let bad_mnemonic = ImportWalletRequest {
            mnemonic: "one two three".to_string(),
            password_hash: "p".repeat(32),
            wallet_name: None,
            network: "local".to_string(),
        };
        assert!(validate_import_wallet(&bad_mnemonic)
            .unwrap_err()
            .message
            .contains("12 or 24 words"));

        let good_mnemonic = ImportWalletRequest {
            mnemonic: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
            password_hash: "p".repeat(32),
            wallet_name: None,
            network: "local".to_string(),
        };
        assert!(validate_import_wallet(&good_mnemonic).is_ok());

        assert!(validate_network("not-a-network")
            .unwrap_err()
            .message
            .contains("Invalid network"));
        assert!(validate_network("testnet").is_ok());
    }

    /// A well-formed request is refused with the *missing backend*, never with invented data.
    /// Every implementation this module had would fail this: the address came from slicing a
    /// mnemonic, the balance was a constant, the hash was a slice of its own input.
    #[test]
    fn every_unwired_backend_names_itself_and_never_invents_data() {
        assert!(validate_create_wallet(&create("wallet", &"p".repeat(32))).is_ok());

        for backend in [
            "Wallet key generation and address derivation",
            "Wallet address derivation from a mnemonic",
            "Wallet keystore and encrypted backup",
            "Wallet balance query",
            "Wallet transaction signing",
            "Wallet transaction submission",
            "Wallet transaction history index",
            "Wallet status store",
            "Wallet keystore",
            "Wallet network selection store",
            "Wallet network registry",
        ] {
            let error = unwired(backend);
            assert!(error.message.contains(backend), "{}", error.message);
            assert!(
                error.message.contains("not wired into this node"),
                "{}",
                error.message
            );
            assert_ne!(
                serde_json::to_value(&error.code).unwrap(),
                serde_json::json!(0),
                "an unwired backend is never a success"
            );
        }

        let refusal = unwired("Wallet key generation and address derivation").message;
        assert!(
            !refusal.contains("test test test") && !refusal.contains("0x"),
            "the refusal must not echo a mnemonic or an address: {refusal}"
        );
    }
}
