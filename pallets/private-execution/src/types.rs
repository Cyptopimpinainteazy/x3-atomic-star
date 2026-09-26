//! Types for the Private Execution pallet.
//!
//! Proposal: PRIV-ENCLAVE-003

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::BlockNumberFor;
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

/// Maximum length for GPU model name.
pub const MAX_GPU_MODEL_LEN: u32 = 128;
/// Maximum length for attestation report blob.
pub const MAX_ATTESTATION_LEN: u32 = 4096;
/// Maximum length for encrypted payload.
pub const MAX_ENCRYPTED_PAYLOAD_LEN: u32 = 1_048_576; // 1 MB
/// Maximum length for encrypted state diff.
pub const MAX_STATE_DIFF_LEN: u32 = 524_288; // 512 KB
/// Maximum length for ZK proof.
pub const MAX_ZK_PROOF_LEN: u32 = 65_536; // 64 KB

/// Default ceiling on the total revealed plaintext one ordering window may hold.
///
/// A window is settled in one transaction that reads every reveal, so an
/// unbounded window is a window nobody can settle — and a window that cannot
/// settle is a window whose bonds can never be resolved. Capping the total is
/// what makes "settle always fits in a block" a property of the pallet rather
/// than of the participants' goodwill. Runtimes configure the exact value with
/// `Config::MaxOrderingWindowBytes`; this is the shipped default, and the weight
/// for settling is sized from the configured value.
pub const MAX_ORDERING_WINDOW_PLAINTEXT_BYTES: u32 = 1_048_576; // 1 MiB

/// Enclave attestation status.
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
    RuntimeDebug,
)]
pub enum EnclaveStatus {
    /// Attestation verified, accepting private TXs.
    Verified,
    /// Attestation needs refresh.
    Expired,
    /// Failed attestation or revoked.
    Revoked,
}

/// Private transaction status.
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
    RuntimeDebug,
)]
pub enum PrivateTxStatus {
    /// In encrypted mempool, waiting for execution.
    Pending,
    /// Being executed inside enclave.
    Executing,
    /// State diff committed on chain.
    Committed,
    /// ZK proof verified (if applicable).
    Verified,
    /// Execution failed inside enclave.
    Failed,
}

/// Attestation record for a confidential validator.
#[derive(
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
    RuntimeDebug,
)]
#[scale_info(skip_type_params(T))]
pub struct EnclaveAttestation<T: frame_system::Config> {
    /// Validator account.
    pub validator: T::AccountId,
    /// GPU model name.
    pub gpu_model: BoundedVec<u8, ConstU32<MAX_GPU_MODEL_LEN>>,
    /// Raw attestation report from NVIDIA CC / AMD SEV-SNP.
    pub attestation_report: BoundedVec<u8, ConstU32<MAX_ATTESTATION_LEN>>,
    /// Enclave's ephemeral encryption public key (X25519).
    pub enclave_public_key: [u8; 32],
    /// Block when attestation was last refreshed.
    pub last_refreshed: BlockNumberFor<T>,
    /// Current status.
    pub status: EnclaveStatus,
}

/// Record of a private transaction.
#[derive(
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
    RuntimeDebug,
)]
#[scale_info(skip_type_params(T))]
pub struct PrivateTxRecord<T: frame_system::Config> {
    /// Transaction hash.
    pub tx_hash: sp_core::H256,
    /// Sender account (can be pseudonymous).
    pub sender: T::AccountId,
    /// Encrypted transaction payload (AES-256-GCM).
    pub encrypted_payload: BoundedVec<u8, ConstU32<MAX_ENCRYPTED_PAYLOAD_LEN>>,
    /// Fee commitment (Pedersen commitment to the fee amount).
    pub fee_commitment: sp_core::H256,
    /// Total fee paid (base + premium).
    pub fee_paid: u128,
    /// Current status.
    pub status: PrivateTxStatus,
    /// Block when submitted.
    pub submitted_at: BlockNumberFor<T>,
    /// Confidential validator that executed this TX.
    pub executed_by: Option<T::AccountId>,
}

/// An encrypted state diff committed on-chain.
#[derive(
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
    RuntimeDebug,
)]
pub struct EncryptedDiff {
    /// Transaction hash this diff belongs to.
    pub tx_hash: sp_core::H256,
    /// Encrypted state changes (encrypted to chain key).
    pub encrypted_state_changes: BoundedVec<u8, ConstU32<MAX_STATE_DIFF_LEN>>,
    /// Pedersen commitment to the plaintext diff.
    pub commitment: sp_core::H256,
    /// Optional ZK validity proof.
    pub zk_proof: Option<BoundedVec<u8, ConstU32<MAX_ZK_PROOF_LEN>>>,
    /// Signature from enclave attestation key (Ed25519).
    pub enclave_signature: [u8; 64],
    /// Block when committed.
    pub committed_at: u32,
}

// NOTE: EncryptedDiff uses concrete types (u32 for block number) since it's stored
// in a BoundedVec and needs to be T-independent. In production, parameterize properly.

// ──────────────────────────────────────────────────────────────
// Commit–reveal ordering window (X3-MEV-006 / X3-MEV-008)
// ──────────────────────────────────────────────────────────────
//
// The algorithm itself lives in `x3-order-window`; these are only the records the
// chain persists so a window's state survives restarts and so a third party can
// recompute the canonical order from storage. Balances are stored as `u128` for the
// same reason `PrivateTxRecord::fee_paid` is: these records are not generic over
// `T::Currency`.

/// One commit–reveal ordering window.
#[derive(Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, RuntimeDebug)]
#[scale_info(skip_type_params(T))]
pub struct OrderingWindowRecord<T: frame_system::Config> {
    /// Account that opened the window.
    pub opened_by: T::AccountId,
    /// First block a commit or reveal may land in (inclusive).
    pub open_block: u64,
    /// Last block a commit or reveal may land in (inclusive); also the deadline.
    pub close_block: u64,
    /// Bond every commitment in this window must post, fixed at open time.
    pub minimum_bond: u128,
    /// Block the window was opened in.
    pub opened_at: BlockNumberFor<T>,
    /// Ordering beacon, installable only after `close_block`. `None` means the
    /// order key is the commit hash itself.
    pub beacon: Option<sp_core::H256>,
    /// Whether the window has produced its one canonical order.
    pub settled: bool,
    /// Commitments recorded so far, bounded by `MaxOrderingCommits`.
    pub commitment_count: u32,
    /// Reveals accepted so far.
    pub reveal_count: u32,
    /// Total revealed plaintext bytes, bounded by `MaxOrderingWindowBytes`.
    pub revealed_bytes: u32,
}

/// One commitment recorded against a window.
#[derive(Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, RuntimeDebug)]
#[scale_info(skip_type_params(T))]
pub struct OrderingCommitment<T: frame_system::Config> {
    /// The account that posted the bond and that alone may reveal.
    pub sender: T::AccountId,
    /// Bond reserved for this commitment.
    pub bond: u128,
    /// Block the commitment landed in.
    pub committed_at_block: u64,
}

/// One reveal accepted against a window.
///
/// The plaintext is kept because a settlement that ordered transactions without
/// being able to hand them on would order nothing. It is bounded by
/// `x3_order_window::MAX_PLAINTEXT_BYTES` at reveal time.
#[derive(Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, RuntimeDebug)]
pub struct OrderingReveal {
    /// Revealed payload.
    pub plaintext: Vec<u8>,
    /// Reveal nonce, bound into the commitment hash.
    pub nonce: [u8; 32],
    /// Block the reveal landed in.
    pub revealed_at_block: u64,
}

/// The single canonical order for a settled window, and what did not make it in.
///
/// Stored rather than merely emitted: `ordered` is the sequence a verifier can
/// recompute from `beacon` and the committed hashes via
/// `x3_order_window::order_key`, and `unrevealed` names the bonds that were
/// forfeited.
#[derive(Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, RuntimeDebug)]
pub struct OrderingSettlementRecord {
    /// Beacon the order keys were computed with, or `None`.
    pub beacon: Option<sp_core::H256>,
    /// Commit hashes in canonical order (position `i` is `ordered[i]`).
    pub ordered: Vec<sp_core::H256>,
    /// Commit hashes that never revealed, in commit-hash order.
    pub unrevealed: Vec<sp_core::H256>,
    /// Total bond forfeited from the unrevealed set.
    pub forfeited_bond: u128,
    /// Block the window was settled in.
    pub settled_at_block: u64,
}
