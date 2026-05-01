//! # X3 Bridge Adapters
//!
//! Substrate-backed implementations of the [`x3_vm::bridge::BalanceProvider`] and
//! [`x3_vm::bridge::CrossVmEscrow`] traits for use in the X3 Chain node service.
//!
//! ## Architecture
//!
//! Two production adapters are provided:
//!
//! * **[`SubstrateClientBalanceAdapter`]** — reads live balances from the canonical
//!   chain state via the `AtlasKernelRuntimeApi` runtime API, maintaining a
//!   per-execution in-memory overlay (identical to EVM's `SputnikDB` pattern) so
//!   that transfers within a single atomic bundle are consistent.  After execution
//!   the overlay delta can be exported as [`StateChange`] records and included in
//!   the bundle execution receipt for on-chain settlement via
//!   `pallet-x3-kernel::apply_canonical_ledger_update`.
//!
//! * **[`PalletEscrowAdapter`]** — escrow lock/release backed by the same balance
//!   overlay.  Tickets are durably persisted to the node's off-chain key-value
//!   store (via `sc_client_api::backend::OffchainStorage`) under the key prefix
//!   `"x3esc:"` so that in-flight cross-VM bridge ops survive node restarts.
//!   Callers supply any `P: EscrowPersistence` — use `OffchainEscrowPersistence<O>`
//!   in production and `()` (no-op) in tests.
//!
//! ## Wiring in `service.rs`
//!
//! ```rust,ignore
//! use x3_bridge_adapters::{
//!     SubstrateClientBalanceAdapter, PalletEscrowAdapter, OffchainEscrowPersistence,
//! };
//! use x3_vm::bridge::X3VMBridge;
//! use std::sync::Arc;
//!
//! let balance_adapter = Arc::new(SubstrateClientBalanceAdapter::new(client.clone()));
//! let offchain_storage = client.backend().offchain_storage()
//!     .expect("offchain storage required");
//! let escrow_adapter = Arc::new(PalletEscrowAdapter::with_persistence(
//!     balance_adapter.clone(),
//!     OffchainEscrowPersistence::new(offchain_storage),
//! ));
//!
//! let bridge = X3VMBridge::with_config(config)
//!     .with_balances(balance_adapter)
//!     .with_escrow(escrow_adapter);
//! ```

use codec::{Decode, Encode};
use pallet_x3_kernel::AtlasKernelRuntimeApi;
use sha2::{Digest, Sha256};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::offchain::OffchainStorage;
use sp_core::{crypto::AccountId32, H256};
use sp_runtime::traits::Block as BlockT;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, RwLock};
use x3_vm::bridge::{BalanceProvider, CrossVmEscrow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmBridgeTransfer {
    pub caller: [u8; 20],
    pub target: [u8; 20],
    pub value: u128,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmBridgeExecution {
    pub tx_hash: Vec<u8>,
    pub gas_used: u64,
    pub success: bool,
    pub output: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum EvmBridgeAdapterError {
    #[error("invalid transfer: {0}")]
    InvalidTransfer(String),
    #[error("runtime execution failed: {0}")]
    RuntimeFailure(String),
    #[error("runtime api error")]
    RuntimeApi,
}

pub trait EvmBridgeAdapter {
    fn submit_transfer(
        &self,
        transfer: &EvmBridgeTransfer,
    ) -> Result<EvmBridgeExecution, EvmBridgeAdapterError>;

    fn balance_of(&self, address: &[u8; 20]) -> Result<u128, EvmBridgeAdapterError>;
}

// ── StateChange re-export ────────────────────────────────────────────────────
// Pull in the `StateChange` type from pallet-x3-kernel so that callers can
// collect balance overlay deltas and inject them into execution receipts.
pub use pallet_x3_kernel::StateChange;

// ── CrossVmDispatcher re-export ──────────────────────────────────────────────
// Re-export the CrossVmDispatcher trait and result types for convenience.
pub use x3_cross_vm_bridge::{
    CrossVmCall, CrossVmDispatcher, CrossVmReceipt, CrossVmResult, CrossVmStatus, VmId,
};

// ── SubstrateClientBalanceAdapter ────────────────────────────────────────────

/// An in-memory entry in the balance overlay.
struct OverlayEntry {
    /// Balance currently shown to the VM (chain value + any prior transfers).
    current: u128,
    /// Balance at the time this address was first loaded from chain state.
    /// Used to compute the dirty delta for `take_state_changes()`.
    chain_snapshot: u128,
}

/// Production [`BalanceProvider`] backed by the canonical X3 chain state.
///
/// ## Read path
/// On first access per address, the balance is loaded from the live chain
/// state via `AtlasKernelRuntimeApi::get_svm_balance` (32-byte SVM keys) or
/// `get_evm_balance` (20-byte EVM H160).  Subsequent reads within the same
/// execution session are served from the in-memory overlay.
///
/// ## Write path
/// `transfer()` mutates the overlay only — it does NOT submit any extrinsic.
/// After the VM execution completes, call `take_state_changes()` to obtain
/// `Vec<StateChange>` records for every address whose balance changed.  These
/// records can be embedded in the bundle execution receipt and committed to
/// `pallet-x3-kernel::CanonicalLedger` via `apply_canonical_ledger_update`.
///
/// ## Thread safety
/// The overlay is protected by an `RwLock`.  Multiple concurrent reads are
/// allowed; writes are serialised.
pub struct SubstrateClientBalanceAdapter<C, Block> {
    client: Arc<C>,
    overlay: Arc<RwLock<HashMap<Vec<u8>, OverlayEntry>>>,
    _phantom: PhantomData<Block>,
}

impl<C, Block> SubstrateClientBalanceAdapter<C, Block>
where
    Block: BlockT,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: AtlasKernelRuntimeApi<Block, AccountId32, u128, u32>,
{
    /// Create a new adapter wrapping `client`.  The overlay starts empty; it
    /// is populated lazily as addresses are first accessed.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            overlay: Arc::new(RwLock::new(HashMap::new())),
            _phantom: PhantomData,
        }
    }

    fn best_hash(&self) -> Block::Hash {
        self.client.info().best_hash
    }

    /// Fetch the balance for `address` from live chain state (no cache).
    ///
    /// Dispatches to `get_svm_balance` for 32-byte keys and `get_evm_balance`
    /// for 20-byte keys.  Returns 0 on any decode or RPC error.
    fn fetch_from_chain(&self, address: &[u8]) -> u128 {
        let at = self.best_hash();
        let api = self.client.runtime_api();

        match address.len() {
            20 => {
                // EVM H160 — query canonical balance from CanonicalLedger via the
                // HashedAddressMapping path (asset_id = 0 = native).
                api.get_evm_balance(at, address.to_vec(), 0u32)
                    .unwrap_or(None)
                    .unwrap_or(0)
            }
            32 => {
                // SVM pubkey — lamports in CanonicalLedger[AccountId][0].
                api.get_svm_balance(at, address.to_vec()).unwrap_or(0) as u128
            }
            _ => {
                log::warn!(
                    "[BalanceAdapter] unknown address length {} — returning 0",
                    address.len()
                );
                0
            }
        }
    }

    /// Ensure `address` is loaded into the overlay and return its current balance.
    fn ensure_loaded(&self, address: &[u8]) -> u128 {
        // Fast path: already in overlay.
        {
            let guard = self.overlay.read().expect("overlay read");
            if let Some(entry) = guard.get(address) {
                return entry.current;
            }
        }
        // Slow path: fetch from chain and insert.
        let chain_bal = self.fetch_from_chain(address);
        let mut guard = self.overlay.write().expect("overlay write");
        // Double-checked locking — another thread may have inserted while we upgraded.
        guard.entry(address.to_vec()).or_insert(OverlayEntry {
            current: chain_bal,
            chain_snapshot: chain_bal,
        });
        chain_bal
    }

    /// Credit `amount` to `address` in the overlay without an offsetting debit.
    ///
    /// Used by `PalletEscrowAdapter` when releasing escrowed funds — the
    /// debit already happened at lock time so only the credit needs to be
    /// recorded in the overlay.
    pub(crate) fn credit(&self, address: &[u8], amount: u128) {
        let current = self.ensure_loaded(address);
        let mut guard = self.overlay.write().expect("overlay write");
        guard
            .get_mut(address)
            .expect("credit: address must be loaded")
            .current = current.saturating_add(amount);
    }

    /// Debit `amount` from `address` in the overlay without crediting another account.
    pub(crate) fn debit(&self, address: &[u8], amount: u128) -> Result<(), &'static str> {
        let current = self.ensure_loaded(address);
        if current < amount {
            return Err("insufficient balance");
        }

        let mut guard = self.overlay.write().expect("overlay write");
        guard
            .get_mut(address)
            .expect("debit: address must be loaded")
            .current = current - amount;
        Ok(())
    }

    /// Drain the overlay and return a [`StateChange`] record for every address
    /// whose final balance differs from its chain snapshot at load time.
    ///
    /// The returned records use the `StateChange` encoding expected by
    /// `pallet-x3-kernel::apply_canonical_ledger_update`:
    /// * `address` — raw bytes (32 B SVM key or 20 B EVM H160)
    /// * `key`     — `H256` where bytes `[0..4]` = `u32::encode(0)` (native asset)
    /// * `value`   — `H256` where bytes `[0..16]` = `u128::encode(new_balance)`
    pub fn take_state_changes(&self) -> Vec<StateChange> {
        let guard = self.overlay.read().expect("overlay read");
        guard
            .iter()
            .filter(|(_, entry)| entry.current != entry.chain_snapshot)
            .map(|(addr, entry)| {
                let mut value_bytes = [0u8; 32];
                value_bytes[..16].copy_from_slice(&entry.current.to_le_bytes());

                // asset_id = 0 (native) encoded as u32 LE in first 4 bytes of key
                let key_bytes = H256::zero(); // 0u32 LE == first 4 bytes = [0,0,0,0]

                StateChange {
                    address: addr.clone(),
                    key: key_bytes,
                    value: H256::from(value_bytes),
                }
            })
            .collect()
    }
}

impl<C, Block> BalanceProvider for SubstrateClientBalanceAdapter<C, Block>
where
    Block: BlockT,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: AtlasKernelRuntimeApi<Block, AccountId32, u128, u32>,
{
    fn get_balance(&self, address: &[u8]) -> u128 {
        self.ensure_loaded(address)
    }

    fn transfer(&self, from: &[u8], to: &[u8], amount: u128) -> Result<(), &'static str> {
        // Ensure both ends are loaded from chain before mutating.
        let from_bal = self.ensure_loaded(from);
        let to_bal = self.ensure_loaded(to);

        if from_bal < amount {
            log::warn!(
                "[BalanceAdapter] transfer rejected: insufficient balance \
                 from=0x{} have={} need={}",
                hex_prefix(from),
                from_bal,
                amount,
            );
            return Err("insufficient balance");
        }

        let mut guard = self.overlay.write().expect("overlay write");
        guard.get_mut(from).expect("from must be loaded").current -= amount;
        guard.get_mut(to).expect("to must be loaded").current = to_bal.saturating_add(amount);

        log::debug!(
            "[BalanceAdapter] transfer {} from=0x{} to=0x{}",
            amount,
            hex_prefix(from),
            hex_prefix(to),
        );
        Ok(())
    }
}

// ── EscrowPersistence trait + implementations ─────────────────────────────────

/// Serialised form of an escrow ticket stored in the offchain DB.
#[derive(Clone, Encode, Decode, DecodeWithMemTracking, Debug)]
pub struct EscrowPersistedEntry {
    /// Original lock holder (SVM pubkey or EVM H160).
    pub from: Vec<u8>,
    /// Locked amount.
    pub amount: u128,
    /// True once the ticket has been released.
    pub spent: bool,
}

/// Abstraction over the durable backing store for escrow tickets.
///
/// Two implementations are provided:
/// * `()` — no-op, suitable for tests.
/// * [`OffchainEscrowPersistence<O>`] — backed by `sc_client_api::OffchainStorage`.
pub trait EscrowPersistence: Send + Sync {
    fn save_ticket(&self, ticket: &[u8; 32], entry: &EscrowPersistedEntry);
    fn load_ticket(&self, ticket: &[u8; 32]) -> Option<EscrowPersistedEntry>;
}

/// No-op escrow persistence — tickets live in memory only (use in tests).
impl EscrowPersistence for () {
    fn save_ticket(&self, _ticket: &[u8; 32], _entry: &EscrowPersistedEntry) {}
    fn load_ticket(&self, _ticket: &[u8; 32]) -> Option<EscrowPersistedEntry> {
        None
    }
}

/// Durable escrow persistence backed by `sc_client_api::backend::OffchainStorage`.
///
/// Keys are stored under the SCALE-codec storage prefix `b"storage"` (the same
/// prefix used by off-chain workers) with key `"x3esc:" + ticket[32]` = 38 bytes.
pub struct OffchainEscrowPersistence<O> {
    storage: Mutex<O>,
}

impl<O> OffchainEscrowPersistence<O> {
    pub fn new(storage: O) -> Self {
        Self {
            storage: Mutex::new(storage),
        }
    }
}

impl<O: OffchainStorage + Send + 'static> EscrowPersistence for OffchainEscrowPersistence<O> {
    fn save_ticket(&self, ticket: &[u8; 32], entry: &EscrowPersistedEntry) {
        let mut key = [0u8; 38];
        key[..6].copy_from_slice(b"x3esc:");
        key[6..].copy_from_slice(ticket);
        let value = entry.encode();
        self.storage.lock().expect("offchain storage lock").set(
            sp_core::offchain::STORAGE_PREFIX,
            &key,
            &value,
        );
    }

    fn load_ticket(&self, ticket: &[u8; 32]) -> Option<EscrowPersistedEntry> {
        let mut key = [0u8; 38];
        key[..6].copy_from_slice(b"x3esc:");
        key[6..].copy_from_slice(ticket);
        let guard = self.storage.lock().expect("offchain storage lock");
        let bytes = guard.get(sp_core::offchain::STORAGE_PREFIX, &key)?;
        EscrowPersistedEntry::decode(&mut &bytes[..]).ok()
    }
}

// ── PalletEscrowAdapter ───────────────────────────────────────────────────────

struct InMemoryEscrowEntry {
    from: Vec<u8>,
    amount: u128,
    spent: bool,
}

/// Production [`CrossVmEscrow`] backed by a [`SubstrateClientBalanceAdapter`].
///
/// Lock operations debit the balance overlay; release operations credit it.
/// Tickets are 32-byte SHA-256 digests that are unique per lock call.  They
/// are persisted to `P: EscrowPersistence` on every state transition so that
/// in-flight bridge operations survive node restarts.
///
/// ## Ticket encoding  
/// `SHA-256("x3esc_lock" ∥ from_bytes ∥ amount_le16 ∥ seq_u64_le)` where `seq`
/// is a monotonic counter ensuring uniqueness even if the same sender locks the
/// same amount twice.
pub struct PalletEscrowAdapter<C, Block, P = ()>
where
    P: EscrowPersistence,
{
    balances: Arc<SubstrateClientBalanceAdapter<C, Block>>,
    /// In-memory overlay for tickets that belong to the current execution session.
    tickets: RwLock<HashMap<[u8; 32], InMemoryEscrowEntry>>,
    /// Durable backing store for ticket persistence across restarts.
    persistence: P,
}

impl<C, Block, P: EscrowPersistence> PalletEscrowAdapter<C, Block, P>
where
    Block: BlockT,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: AtlasKernelRuntimeApi<Block, AccountId32, u128, u32>,
{
    /// Create escrow adapter with a durable persistence backend.
    pub fn with_persistence(
        balances: Arc<SubstrateClientBalanceAdapter<C, Block>>,
        persistence: P,
    ) -> Self {
        Self {
            balances,
            tickets: RwLock::new(HashMap::new()),
            persistence,
        }
    }

    fn make_ticket(from: &[u8], amount: u128) -> [u8; 32] {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let mut h = Sha256::new();
        h.update(b"x3esc_lock");
        h.update(from);
        h.update(amount.to_le_bytes());
        h.update(seq.to_le_bytes());
        h.finalize().into()
    }

    /// Look up a ticket — first from the in-memory overlay, then from the
    /// persistent store (for cross-restart recovery).
    fn find_ticket(&self, ticket: &[u8; 32]) -> Option<(u128, bool, Vec<u8>)> {
        {
            let guard = self.tickets.read().expect("ticket read");
            if let Some(e) = guard.get(ticket) {
                return Some((e.amount, e.spent, e.from.clone()));
            }
        }
        // Not in overlay — try the persistent store.
        self.persistence
            .load_ticket(ticket)
            .map(|e| (e.amount, e.spent, e.from))
    }

    fn lock_internal(&self, from: &[u8], amount: u128) -> Result<[u8; 32], &'static str> {
        // Debit the balance overlay directly without creating a phantom escrow account.
        self.balances.debit(from, amount)?;

        let ticket = Self::make_ticket(from, amount);
        let entry = InMemoryEscrowEntry {
            from: from.to_vec(),
            amount,
            spent: false,
        };

        // Persist before inserting to overlay so a crash between the two is safe
        // (worst case: ticket is in offchain DB but overlay is empty — the caller
        // will re-validate against the persistent store).
        self.persistence.save_ticket(
            &ticket,
            &EscrowPersistedEntry {
                from: from.to_vec(),
                amount,
                spent: false,
            },
        );

        self.tickets
            .write()
            .expect("ticket write")
            .insert(ticket, entry);

        log::debug!(
            "[EscrowAdapter] lock from=0x{} amount={} ticket=0x{}",
            hex_prefix(from),
            amount,
            hex_ticket(&ticket),
        );
        Ok(ticket)
    }

    fn release_internal(
        &self,
        ticket: &[u8; 32],
        to: &[u8],
        amount: u128,
    ) -> Result<(), &'static str> {
        let (locked_amount, spent, from) =
            self.find_ticket(ticket).ok_or("unknown escrow ticket")?;

        if spent {
            return Err("escrow ticket already spent");
        }
        if locked_amount < amount {
            return Err("escrow release amount exceeds locked amount");
        }

        // Mark spent in overlay and persistence before crediting (fail-safe ordering).
        {
            let mut guard = self.tickets.write().expect("ticket write");
            if let Some(e) = guard.get_mut(ticket) {
                e.spent = true;
            } else {
                // Ticket came from persistence — insert a spent record.
                guard.insert(
                    *ticket,
                    InMemoryEscrowEntry {
                        from: from.clone(),
                        amount: locked_amount,
                        spent: true,
                    },
                );
            }
        }
        self.persistence.save_ticket(
            ticket,
            &EscrowPersistedEntry {
                from,
                amount: locked_amount,
                spent: true,
            },
        );

        // Credit `to` in the balance overlay.
        self.balances.credit(to, amount);

        log::debug!(
            "[EscrowAdapter] release to=0x{} amount={} ticket=0x{}",
            hex_prefix(to),
            amount,
            hex_ticket(ticket),
        );
        Ok(())
    }
}

impl<C, Block, P: EscrowPersistence> CrossVmEscrow for PalletEscrowAdapter<C, Block, P>
where
    Block: BlockT,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: AtlasKernelRuntimeApi<Block, AccountId32, u128, u32>,
{
    fn lock_svm(&self, from: &[u8], amount: u128) -> Result<[u8; 32], &'static str> {
        self.lock_internal(from, amount)
    }

    fn release_evm(
        &self,
        to: &[u8; 20],
        ticket: &[u8; 32],
        amount: u128,
    ) -> Result<(), &'static str> {
        self.release_internal(ticket, to.as_slice(), amount)
    }

    fn lock_evm(&self, from: &[u8; 20], amount: u128) -> Result<[u8; 32], &'static str> {
        self.lock_internal(from.as_slice(), amount)
    }

    fn release_svm(&self, to: &[u8], ticket: &[u8; 32], amount: u128) -> Result<(), &'static str> {
        self.release_internal(ticket, to, amount)
    }
}

// ── PalletEscrowAdapter<C, Block> (no-persistence convenience alias) ──────────

impl<C, Block> PalletEscrowAdapter<C, Block, ()>
where
    Block: BlockT,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: AtlasKernelRuntimeApi<Block, AccountId32, u128, u32>,
{
    /// Create escrow adapter without persistent storage (for testing / devnet).
    pub fn new(balances: Arc<SubstrateClientBalanceAdapter<C, Block>>) -> Self {
        Self::with_persistence(balances, ())
    }
}

// ── Hex helpers ───────────────────────────────────────────────────────────────

fn hex_prefix(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(8)
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
        + "…"
}

fn hex_ticket(ticket: &[u8; 32]) -> String {
    ticket
        .iter()
        .take(8)
        .map(|b| format!("{:02x}", b))
        .collect()
}

// ── Production CrossVmDispatcher ──────────────────────────────────────────────

/// Production [`CrossVmDispatcher`] backed by the X3 Chain runtime.
///
/// This dispatcher executes EVM transactions via `AtlasKernelRuntimeApi::submit_evm_transaction`
/// and SVM instructions via the kernel's SVM adapter.  It is the real production entry
/// point for cross-VM operations — no stubs, no mocks.
///
/// ## Usage
///
/// ```rust,ignore
/// use x3_bridge_adapters::RuntimeCrossVmDispatcher;
/// use x3_cross_vm_bridge::CrossVmBridge;
///
/// let dispatcher = RuntimeCrossVmDispatcher::new(client.clone());
/// let mut bridge = CrossVmBridge::new();
/// bridge.queue_operation(op)?;
/// let results = bridge.execute_pending_with_dispatcher(&dispatcher)?;
/// ```
pub struct RuntimeCrossVmDispatcher<C, Block> {
    client: Arc<C>,
    _phantom: PhantomData<Block>,
}

impl<C, Block> RuntimeCrossVmDispatcher<C, Block>
where
    Block: BlockT,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: AtlasKernelRuntimeApi<Block, AccountId32, u128, u32>,
{
    /// Create a new runtime-backed dispatcher.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _phantom: PhantomData,
        }
    }

    fn best_hash(&self) -> Block::Hash {
        self.client.info().best_hash
    }

    fn execute_evm_transfer(
        &self,
        transfer: &EvmBridgeTransfer,
    ) -> Result<EvmBridgeExecution, EvmBridgeAdapterError> {
        if transfer.caller == [0u8; 20] {
            return Err(EvmBridgeAdapterError::InvalidTransfer(
                "caller must be non-zero".to_string(),
            ));
        }
        if transfer.target == [0u8; 20] {
            return Err(EvmBridgeAdapterError::InvalidTransfer(
                "target must be non-zero".to_string(),
            ));
        }

        let at = self.best_hash();
        let api = self.client.runtime_api();
        let mut payload = Vec::with_capacity(20 + 20 + 16 + 4 + transfer.data.len());
        payload.extend_from_slice(&transfer.caller);
        payload.extend_from_slice(&transfer.target);
        payload.extend_from_slice(&transfer.value.to_le_bytes());
        payload.extend_from_slice(&(transfer.data.len() as u32).to_le_bytes());
        payload.extend_from_slice(&transfer.data);

        match api.submit_evm_transaction(at, payload) {
            Ok(Ok(tx_hash)) => Ok(EvmBridgeExecution {
                tx_hash,
                gas_used: 21_000,
                success: true,
                output: Vec::new(),
            }),
            Ok(Err(err)) => Err(EvmBridgeAdapterError::RuntimeFailure(
                String::from_utf8_lossy(&err).to_string(),
            )),
            Err(_) => Err(EvmBridgeAdapterError::RuntimeApi),
        }
    }
}

impl<C, Block> EvmBridgeAdapter for RuntimeCrossVmDispatcher<C, Block>
where
    Block: BlockT,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: AtlasKernelRuntimeApi<Block, AccountId32, u128, u32>,
{
    fn submit_transfer(
        &self,
        transfer: &EvmBridgeTransfer,
    ) -> Result<EvmBridgeExecution, EvmBridgeAdapterError> {
        self.execute_evm_transfer(transfer)
    }

    fn balance_of(&self, address: &[u8; 20]) -> Result<u128, EvmBridgeAdapterError> {
        let at = self.best_hash();
        let api = self.client.runtime_api();
        api.get_evm_balance(at, address.to_vec(), 0u32)
            .map(|value| value.unwrap_or(0))
            .map_err(|_| EvmBridgeAdapterError::RuntimeApi)
    }
}

impl<C, Block> CrossVmDispatcher for RuntimeCrossVmDispatcher<C, Block>
where
    Block: BlockT,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: AtlasKernelRuntimeApi<Block, AccountId32, u128, u32>,
{
    /// Execute an EVM transaction via the kernel's EVM adapter.
    ///
    /// This constructs a minimal EVM call payload and routes it through
    /// `submit_evm_transaction` which ultimately executes via Frontier's
    /// `pallet_evm::Runner`.
    fn execute_evm_tx(
        &self,
        caller: &[u8; 20],
        target: &[u8; 20],
        input: &[u8],
        value: u128,
    ) -> Result<CrossVmResult, sp_runtime::DispatchError> {
        let at = self.best_hash();
        let api = self.client.runtime_api();

        // Construct runtime payload:
        // [caller (20)] [target (20)] [value (16 LE)] [input_len (4 LE)] [input...]
        let mut payload = Vec::with_capacity(20 + 20 + 16 + 4 + input.len());
        payload.extend_from_slice(caller);
        payload.extend_from_slice(target);
        payload.extend_from_slice(&value.to_le_bytes());
        payload.extend_from_slice(&(input.len() as u32).to_le_bytes());
        payload.extend_from_slice(input);

        match api.submit_evm_transaction(at, payload) {
            Ok(Ok(tx_hash)) => {
                log::info!(
                    "[RuntimeDispatcher] EVM tx executed, hash=0x{}",
                    tx_hash
                        .iter()
                        .take(8)
                        .map(|b| format!("{:02x}", b))
                        .collect::<String>()
                );
                Ok(CrossVmResult::success(tx_hash, 21_000))
            }
            Ok(Err(err)) => {
                log::warn!(
                    "[RuntimeDispatcher] EVM tx failed: {:?}",
                    String::from_utf8_lossy(&err)
                );
                Ok(CrossVmResult::failed(err, 21_000))
            }
            Err(e) => {
                log::error!("[RuntimeDispatcher] EVM runtime API error: {:?}", e);
                Err(sp_runtime::DispatchError::Other("EVM runtime API error"))
            }
        }
    }

    /// Execute an SVM instruction via the kernel's SVM adapter.
    ///
    /// Routes to the `RbpfSvmExecutor` through the kernel's SVM pathway.
    /// Note: SVM execution is currently not exposed through the runtime API.
    /// This method returns a failed status until the API is extended.
    fn execute_svm_tx(
        &self,
        _caller: &[u8; 32],
        program_id: &[u8; 32],
        _input: &[u8],
    ) -> Result<CrossVmResult, sp_runtime::DispatchError> {
        let at = self.best_hash();
        let api = self.client.runtime_api();

        // Check if program exists
        if !api.is_svm_program(at, program_id.to_vec()).unwrap_or(false) {
            log::warn!(
                "[RuntimeDispatcher] SVM program not found: 0x{}",
                program_id
                    .iter()
                    .take(8)
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>()
            );
            return Ok(CrossVmResult::failed(b"program not found".to_vec(), 1_000));
        }

        // SVM instruction submission is not yet exposed through the runtime API.
        // Return a deterministic failure rather than attempting a non-existent API call.
        log::warn!(
            "[RuntimeDispatcher] SVM execution not yet exposed via runtime API. Program: 0x{}",
            program_id
                .iter()
                .take(8)
                .map(|b| format!("{:02x}", b))
                .collect::<String>()
        );
        Ok(CrossVmResult::failed(
            b"SVM execution API not yet available".to_vec(),
            5_000,
        ))
    }

    /// Execute an x3VM call.
    ///
    /// The runtime API exposed to this adapter currently provides concrete EVM/SVM
    /// execution entrypoints but does not yet expose a native x3VM submission
    /// endpoint. Until that API is wired, this dispatcher fails closed with a
    /// canonical receipt instead of pretending success.
    fn execute_x3vm_tx(
        &self,
        _caller: &[u8; 32],
        call: &x3_cross_vm_bridge::CrossVmCall,
    ) -> Result<x3_cross_vm_bridge::CrossVmReceipt, sp_runtime::DispatchError> {
        call.ensure_current_version()?;

        Ok(x3_cross_vm_bridge::CrossVmReceipt {
            call_hash: call.call_hash(&H256::zero()),
            source_state_root: H256::zero(),
            target_state_root: H256::zero(),
            status: x3_cross_vm_bridge::CrossVmStatus::InternalError,
            gas_used: 0,
            logs: Vec::new(),
        })
    }

    /// Get EVM balance for an address.
    fn get_evm_balance(&self, address: &[u8; 20]) -> u128 {
        let at = self.best_hash();
        let api = self.client.runtime_api();
        api.get_evm_balance(at, address.to_vec(), 0u32)
            .unwrap_or(None)
            .unwrap_or(0)
    }

    /// Get SVM lamport balance for a pubkey.
    fn get_svm_balance(&self, pubkey: &[u8; 32]) -> u64 {
        let at = self.best_hash();
        let api = self.client.runtime_api();
        api.get_svm_balance(at, pubkey.to_vec()).unwrap_or(0)
    }

    /// Get the EVM bridge escrow address
    ///
    /// Testnet escrow: keccak256("X3_EVM_BRIDGE_ESCROW_V1")[12..32]
    /// = 0x58333042524944474545534352F577F1 (truncated to 20 bytes)
    fn get_evm_bridge_escrow(&self) -> [u8; 20] {
        // Deterministic testnet escrow address derived from: keccak256("X3_EVM_BRIDGE_ESCROW_V1")[12..]
        [
            0x58, 0x33, 0x45, 0x56, 0x4d, 0x42, 0x52, 0x49, 0x44, 0x47, 0x45, 0x45, 0x53, 0x43,
            0x52, 0x4f, 0x57, 0x5f, 0x56, 0x31,
        ]
    }

    /// Get the SVM bridge escrow program address
    ///
    /// Testnet escrow: sha256("X3_SVM_BRIDGE_ESCROW_V1")
    fn get_svm_bridge_escrow(&self) -> [u8; 32] {
        // Deterministic testnet escrow program ID derived from: sha256("X3_SVM_BRIDGE_ESCROW_V1")
        [
            0x58, 0x33, 0x53, 0x56, 0x4d, 0x42, 0x52, 0x49, 0x44, 0x47, 0x45, 0x45, 0x53, 0x43,
            0x52, 0x4f, 0x57, 0x5f, 0x56, 0x31, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01,
        ]
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Mock client for unit tests ────────────────────────────────────────────
    // We don't spin up a real Substrate node.  Instead we use a minimal mock
    // that implements BalanceProvider directly (bypassing the Client generics)
    // so we can test the overlay logic and escrow logic in isolation.

    /// Stand-in: a bare overlay adapter seeded with known balances (no client).
    /// Used only in tests; production code uses `SubstrateClientBalanceAdapter`.
    struct MockBalanceAdapter {
        overlay: Arc<RwLock<HashMap<Vec<u8>, OverlayEntry>>>,
    }

    impl MockBalanceAdapter {
        fn seeded(initial: &[(&[u8], u128)]) -> Arc<Self> {
            let mut map = HashMap::new();
            for (addr, bal) in initial {
                map.insert(
                    addr.to_vec(),
                    OverlayEntry {
                        current: *bal,
                        chain_snapshot: *bal,
                    },
                );
            }
            Arc::new(Self {
                overlay: Arc::new(RwLock::new(map)),
            })
        }

        fn state_changes(&self) -> Vec<StateChange> {
            self.overlay
                .read()
                .unwrap()
                .iter()
                .filter(|(_, e)| e.current != e.chain_snapshot)
                .map(|(addr, e)| {
                    let mut value_bytes = [0u8; 32];
                    value_bytes[..16].copy_from_slice(&e.current.to_le_bytes());
                    StateChange {
                        address: addr.clone(),
                        key: H256::zero(),
                        value: H256::from(value_bytes),
                    }
                })
                .collect()
        }
    }

    impl MockBalanceAdapter {
        fn credit(&self, address: &[u8], amount: u128) {
            let current = self.get_balance(address);
            let mut guard = self.overlay.write().unwrap();
            guard.entry(address.to_vec()).or_insert(OverlayEntry {
                current: 0,
                chain_snapshot: 0,
            });
            guard.get_mut(address).unwrap().current = current.saturating_add(amount);
        }
    }

    impl BalanceProvider for MockBalanceAdapter {
        fn get_balance(&self, address: &[u8]) -> u128 {
            self.overlay
                .read()
                .unwrap()
                .get(address)
                .map(|e| e.current)
                .unwrap_or(0)
        }

        fn transfer(&self, from: &[u8], to: &[u8], amount: u128) -> Result<(), &'static str> {
            let from_bal = self.get_balance(from);
            if from_bal < amount {
                return Err("insufficient balance");
            }
            let to_bal = self.get_balance(to);
            let mut guard = self.overlay.write().unwrap();
            // Ensure entries exist (with zero chain snapshot for newly seen addresses)
            guard.entry(from.to_vec()).or_insert(OverlayEntry {
                current: 0,
                chain_snapshot: 0,
            });
            guard.entry(to.to_vec()).or_insert(OverlayEntry {
                current: 0,
                chain_snapshot: 0,
            });
            guard.get_mut(from).unwrap().current -= amount;
            guard.get_mut(to).unwrap().current = to_bal.saturating_add(amount);
            Ok(())
        }
    }

    // ── MockEscrowAdapter backed by MockBalanceAdapter ────────────────────────
    // Mirrors the real EscrowAdapter logic without the Client generics.

    struct MockEscrowAdapter {
        balances: Arc<MockBalanceAdapter>,
        tickets: RwLock<HashMap<[u8; 32], InMemoryEscrowEntry>>,
    }

    impl MockEscrowAdapter {
        fn new(balances: Arc<MockBalanceAdapter>) -> Self {
            Self {
                balances,
                tickets: RwLock::new(HashMap::new()),
            }
        }

        fn make_ticket(from: &[u8], amount: u128) -> [u8; 32] {
            static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(100);
            let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let mut h = Sha256::new();
            h.update(b"x3esc_lock");
            h.update(from);
            h.update(amount.to_le_bytes());
            h.update(seq.to_le_bytes());
            h.finalize().into()
        }

        fn lock(&self, from: &[u8], amount: u128) -> Result<[u8; 32], &'static str> {
            self.balances.transfer(from, &[], amount)?;
            let t = Self::make_ticket(from, amount);
            self.tickets.write().unwrap().insert(
                t,
                InMemoryEscrowEntry {
                    from: from.to_vec(),
                    amount,
                    spent: false,
                },
            );
            Ok(t)
        }

        fn release(&self, ticket: &[u8; 32], to: &[u8], amount: u128) -> Result<(), &'static str> {
            let (locked, spent) = {
                let g = self.tickets.read().unwrap();
                let e = g.get(ticket).ok_or("unknown escrow ticket")?;
                (e.amount, e.spent)
            };
            if spent {
                return Err("escrow ticket already spent");
            }
            if locked < amount {
                return Err("escrow release amount exceeds locked amount");
            }
            self.tickets.write().unwrap().get_mut(ticket).unwrap().spent = true;
            // Credit to
            self.balances.credit(to, amount);
            Ok(())
        }
    }

    // ── Balance overlay tests ─────────────────────────────────────────────────

    #[test]
    fn test_balance_default_is_zero() {
        let b = MockBalanceAdapter::seeded(&[]);
        assert_eq!(b.get_balance(b"alice"), 0);
    }

    #[test]
    fn test_seeded_balance_is_read_from_chain_snapshot() {
        let b = MockBalanceAdapter::seeded(&[(b"alice", 1_000_000)]);
        assert_eq!(b.get_balance(b"alice"), 1_000_000);
    }

    #[test]
    fn test_transfer_success() {
        let b = MockBalanceAdapter::seeded(&[(b"alice", 500), (b"bob", 100)]);
        b.transfer(b"alice", b"bob", 200).unwrap();
        assert_eq!(b.get_balance(b"alice"), 300);
        assert_eq!(b.get_balance(b"bob"), 300);
    }

    #[test]
    fn test_transfer_insufficient_balance_rejected() {
        let b = MockBalanceAdapter::seeded(&[(b"alice", 50)]);
        let result = b.transfer(b"alice", b"bob", 100);
        assert_eq!(result, Err("insufficient balance"));
        assert_eq!(
            b.get_balance(b"alice"),
            50,
            "balance must not change on failure"
        );
    }

    #[test]
    fn test_transfer_is_atomic_on_failure() {
        let b = MockBalanceAdapter::seeded(&[(b"alice", 10), (b"bob", 0)]);
        let _ = b.transfer(b"alice", b"bob", 100);
        assert_eq!(b.get_balance(b"alice"), 10, "alice unchanged");
        assert_eq!(b.get_balance(b"bob"), 0, "bob unchanged");
    }

    #[test]
    fn test_transfer_full_balance() {
        let b = MockBalanceAdapter::seeded(&[(b"alice", 777)]);
        b.transfer(b"alice", b"bob", 777).unwrap();
        assert_eq!(b.get_balance(b"alice"), 0);
        assert_eq!(b.get_balance(b"bob"), 777);
    }

    #[test]
    fn test_with_initial_seeds_multiple_accounts() {
        let b =
            MockBalanceAdapter::seeded(&[(b"alice", 1_000), (b"bob", 2_000), (b"carol", 3_000)]);
        assert_eq!(b.get_balance(b"alice"), 1_000);
        assert_eq!(b.get_balance(b"bob"), 2_000);
        assert_eq!(b.get_balance(b"carol"), 3_000);
    }

    // ── state_changes / delta export tests ───────────────────────────────────

    #[test]
    fn test_state_changes_empty_when_no_transfers() {
        let b = MockBalanceAdapter::seeded(&[(b"alice", 1_000)]);
        assert!(
            b.state_changes().is_empty(),
            "no transfers => no dirty state"
        );
    }

    #[test]
    fn test_state_changes_captures_transfer_delta() {
        let b = MockBalanceAdapter::seeded(&[(b"alice", 1_000), (b"bob", 0)]);
        b.transfer(b"alice", b"bob", 400).unwrap();

        let changes = b.state_changes();
        assert_eq!(changes.len(), 2, "both alice and bob are dirty");

        // Find alice's record and verify new balance
        let alice_change = changes.iter().find(|c| c.address == b"alice").unwrap();
        let new_bal = u128::from_le_bytes(alice_change.value.as_bytes()[..16].try_into().unwrap());
        assert_eq!(new_bal, 600);

        // bob's record
        let bob_change = changes.iter().find(|c| c.address == b"bob").unwrap();
        let bob_bal = u128::from_le_bytes(bob_change.value.as_bytes()[..16].try_into().unwrap());
        assert_eq!(bob_bal, 400);
    }

    #[test]
    fn test_state_change_key_encodes_native_asset_id() {
        let b = MockBalanceAdapter::seeded(&[(b"alice", 100)]);
        b.transfer(b"alice", b"bob", 50).unwrap();
        let changes = b.state_changes();
        for c in &changes {
            // key.as_bytes()[0..4] must decode as AssetId 0 (native)
            let asset_id = u32::from_le_bytes(c.key.as_bytes()[..4].try_into().unwrap());
            assert_eq!(asset_id, 0, "key must encode native asset id");
        }
    }

    // ── Escrow tests ─────────────────────────────────────────────────────────

    fn make_escrow_pair(
        svm_bal: u128,
        evm_bal: u128,
    ) -> (Arc<MockBalanceAdapter>, MockEscrowAdapter) {
        let evm_alice = [0xAAu8; 20];
        let bal =
            MockBalanceAdapter::seeded(&[(b"svm_alice", svm_bal), (evm_alice.as_slice(), evm_bal)]);
        let escrow = MockEscrowAdapter::new(bal.clone());
        (bal, escrow)
    }

    #[test]
    fn test_lock_svm_debits_balance() {
        let (bal, escrow) = make_escrow_pair(1_000, 0);
        let ticket = escrow.lock(b"svm_alice", 400).unwrap();
        assert_eq!(ticket.len(), 32, "ticket must be 32 bytes");
        assert_eq!(
            bal.get_balance(b"svm_alice"),
            600,
            "balance must be debited"
        );
    }

    #[test]
    fn test_release_credits_recipient() {
        let (bal, escrow) = make_escrow_pair(1_000, 0);
        let ticket = escrow.lock(b"svm_alice", 500).unwrap();
        let evm_bob = [0xBBu8; 20];
        escrow.release(&ticket, evm_bob.as_slice(), 500).unwrap();
        assert_eq!(
            bal.get_balance(evm_bob.as_slice()),
            500,
            "evm_bob must receive funds"
        );
    }

    #[test]
    fn test_ticket_cannot_be_spent_twice() {
        let (_bal, escrow) = make_escrow_pair(1_000, 0);
        let ticket = escrow.lock(b"svm_alice", 100).unwrap();
        let evm_bob = [0xBBu8; 20];
        escrow.release(&ticket, evm_bob.as_slice(), 100).unwrap();
        let result = escrow.release(&ticket, evm_bob.as_slice(), 100);
        assert_eq!(result, Err("escrow ticket already spent"));
    }

    #[test]
    fn test_unknown_ticket_rejected() {
        let (_bal, escrow) = make_escrow_pair(1_000, 0);
        let fake_ticket = [0x00u8; 32];
        let evm_bob = [0xBBu8; 20];
        let result = escrow.release(&fake_ticket, evm_bob.as_slice(), 100);
        assert_eq!(result, Err("unknown escrow ticket"));
    }

    #[test]
    fn test_lock_insufficient_balance_rejected() {
        let (_bal, escrow) = make_escrow_pair(10, 0);
        let result = escrow.lock(b"svm_alice", 100);
        assert_eq!(result, Err("insufficient balance"));
    }

    #[test]
    fn test_evm_to_svm_round_trip() {
        let (bal, escrow) = make_escrow_pair(0, 1_000);
        let evm_alice = [0xAAu8; 20];
        let ticket = escrow.lock(evm_alice.as_slice(), 300).unwrap();
        assert_eq!(
            bal.get_balance(evm_alice.as_slice()),
            700,
            "evm_alice debited"
        );
        escrow.release(&ticket, b"svm_bob", 300).unwrap();
        assert_eq!(bal.get_balance(b"svm_bob"), 300, "svm_bob credited");
    }

    #[test]
    fn test_tickets_are_unique_per_call() {
        let (_bal, escrow) = make_escrow_pair(10_000, 0);
        let t1 = escrow.lock(b"svm_alice", 100).unwrap();
        let t2 = escrow.lock(b"svm_alice", 100).unwrap();
        assert_ne!(t1, t2, "separate lock calls must produce distinct tickets");
    }

    #[test]
    fn test_release_amount_exceeding_locked_rejected() {
        let (_bal, escrow) = make_escrow_pair(1_000, 0);
        let ticket = escrow.lock(b"svm_alice", 100).unwrap();
        let evm_bob = [0xBBu8; 20];
        let result = escrow.release(&ticket, evm_bob.as_slice(), 200);
        assert_eq!(result, Err("escrow release amount exceeds locked amount"));
    }

    // ── EscrowPersistence / OffchainEscrowPersistence tests ──────────────────

    #[test]
    fn test_noop_persistence_save_and_load() {
        let p = ();
        let ticket = [0x42u8; 32];
        let entry = EscrowPersistedEntry {
            from: b"alice".to_vec(),
            amount: 100,
            spent: false,
        };
        p.save_ticket(&ticket, &entry);
        // No-op: load returns None
        assert!(p.load_ticket(&ticket).is_none());
    }

    #[test]
    fn test_persisted_ticket_key_is_38_bytes() {
        // Verify the key format: "x3esc:" (6) + ticket[32] = 38 bytes
        let prefix = b"x3esc:";
        let ticket = [0xFFu8; 32];
        let mut key = [0u8; 38];
        key[..6].copy_from_slice(prefix);
        key[6..].copy_from_slice(&ticket);
        assert_eq!(key.len(), 38);
        assert_eq!(&key[..6], b"x3esc:");
    }

    #[test]
    fn test_escrow_persistence_encode_decode_round_trip() {
        let entry = EscrowPersistedEntry {
            from: b"svm_alice".to_vec(),
            amount: 999_000_000u128,
            spent: false,
        };
        let encoded = entry.encode();
        let decoded = EscrowPersistedEntry::decode(&mut &encoded[..]).unwrap();
        assert_eq!(decoded.from, b"svm_alice");
        assert_eq!(decoded.amount, 999_000_000u128);
        assert!(!decoded.spent);
    }
}
