#![deny(unsafe_code)]
#![allow(deprecated)]
#![allow(missing_docs)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::large_enum_variant)]
//! # Private Execution Environments Pallet
//!
//! Proposal: PRIV-ENCLAVE-003
//!
//! Provides native "private mode" where transactions are routed through encrypted
//! mempools and executed inside trusted GPU enclaves (NVIDIA Confidential Computing).
//! Results are committed as encrypted state diffs with optional ZK proofs.
//!
//! ## Key Invariants
//!
//! - PRIV-EXEC-001: TX content never exposed in plaintext outside enclave
//! - PRIV-EXEC-002: Encrypted state diff == public execution of same TX
//! - PRIV-EXEC-003: No single validator can decrypt (threshold t-of-n)
//! - PRIV-EXEC-004: Attestation verified before joining confidential set
//! - PRIV-EXEC-005: Premium fee correctly collected and split
//! - PRIV-EXEC-006: Finality latency overhead ≤1ms

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

/// Verifies a confidential-computing attestation report against a trust root.
///
/// The pallet cannot check an NVIDIA CC (or any vendor) attestation chain by itself:
/// that needs the vendor's trust root, a certificate chain and a signature check over
/// the report, the GPU model and the enclave key. Until a chain configures a verifier
/// that can do that, the shipped default refuses every report — which disables
/// confidential-validator registration instead of accepting a non-empty byte string as
/// proof. `register_confidential_validator` used to do exactly that: `verify_attestation`
/// was `!report.is_empty()`, so any signed account could take a confidential slot and
/// its premium-fee share with `vec![1]`.
pub trait TeeAttestationVerifier {
    /// `true` only when `report` is a genuine attestation for `gpu_model` and
    /// `enclave_public_key`. Implementations must fail closed: anything they cannot
    /// verify answers `false`.
    fn verify(report: &[u8], gpu_model: &[u8], enclave_public_key: &[u8; 32]) -> bool;
}

/// The default verifier: refuse every report.
///
/// PRIV-EXEC-004 says an attestation is verified before a validator joins the
/// confidential set. With no vendor verifier configured, the only honest way to keep
/// that invariant is to refuse — the type name is the documentation.
pub struct RefuseAllAttestations;

impl TeeAttestationVerifier for RefuseAllAttestations {
    fn verify(_report: &[u8], _gpu_model: &[u8], _enclave_public_key: &[u8; 32]) -> bool {
        false
    }
}

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::WeightInfo;

pub mod types;
pub use types::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, OnUnbalanced, ReservableCurrency},
        Blake2_128Concat, PalletId,
    };
    use frame_system::pallet_prelude::*;
    use parity_scale_codec::Encode;
    use sp_runtime::{
        traits::{AccountIdConversion, SaturatedConversion, Saturating, Zero},
        Perbill,
    };
    use sp_std::prelude::*;
    use x3_order_window::{
        commitment_hash, order_key, CommitRevealLane, FairOrderError, OrderingWindow,
        MAX_PLAINTEXT_BYTES,
    };
    use x3_threshold_core::{
        validate_encrypted_transaction, EncryptedTransaction, MempoolError as ThresholdRefusal,
        ThresholdPublicKey,
    };

    type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
        <T as frame_system::Config>::AccountId,
    >>::NegativeImbalance;

    use frame_support::traits::StorageVersion;

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::without_storage_info]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    // ──────────────────────────────────────────────────────────────
    // Config
    // ──────────────────────────────────────────────────────────────

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Currency for fee collection.
        type Currency: ReservableCurrency<Self::AccountId>;

        /// Handler for burned fee portion.
        type BurnDestination: OnUnbalanced<NegativeImbalanceOf<Self>>;

        /// Origin that can manage the confidential validator set.
        type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// The pallet's ID (for fee escrow).
        #[pallet::constant]
        type PalletId: Get<PalletId>;

        /// Premium fee in basis points added on top of base fee.
        /// E.g., 150 = 1.5% premium.
        #[pallet::constant]
        type PrivateFeePremiumBps: Get<u16>;

        /// Minimum confidential validators for quorum.
        #[pallet::constant]
        type MinConfidentialQuorum: Get<u32>;

        /// Maximum confidential validators.
        #[pallet::constant]
        type MaxConfidentialValidators: Get<u32>;

        /// Maximum encrypted state diffs per block.
        #[pallet::constant]
        type MaxDiffsPerBlock: Get<u32>;

        /// Maximum payload size for encrypted transactions (bytes).
        #[pallet::constant]
        type MaxEncryptedPayloadSize: Get<u32>;

        /// Attestation validity period in blocks.
        #[pallet::constant]
        type AttestationValidityPeriod: Get<BlockNumberFor<Self>>;

        /// How an attestation report is verified.
        ///
        /// Required, not optional: PRIV-EXEC-004 is only true when something can
        /// actually verify the report. The shipped default (`RefuseAllAttestations`)
        /// refuses everything, so a chain that has not implemented a vendor verifier
        /// registers no confidential validators at all.
        type AttestationVerifier: TeeAttestationVerifier;

        /// Revenue share to confidential validators (bps out of 10_000).
        #[pallet::constant]
        type ConfidentialValidatorShareBps: Get<u16>;

        /// Revenue share to burn (bps out of 10_000).
        #[pallet::constant]
        type PrivateBurnShareBps: Get<u16>;

        /// Revenue share to stakers (bps out of 10_000).
        #[pallet::constant]
        type PrivateStakerShareBps: Get<u16>;

        /// Minimum bond a commitment to an ordering window must post.
        ///
        /// A commitment that never reveals forfeits this, so the bond is what
        /// makes "commit and then stay silent to grief the window" cost
        /// something. Fixed into a window when it opens, so the terms a
        /// participant committed under cannot move under it.
        #[pallet::constant]
        type MinOrderingBond: Get<BalanceOf<Self>>;

        /// Largest number of commitments one ordering window accepts.
        ///
        /// Bounds both the storage a window can occupy and the work
        /// `settle_ordering_window` does, which is why the settle weight is a
        /// function of this value rather than a constant.
        #[pallet::constant]
        type MaxOrderingCommits: Get<u32>;

        /// Ceiling on the total revealed plaintext one ordering window may hold.
        ///
        /// Bounds what settling a window has to read, so a window can never be
        /// grown past what fits in a block — which would strand every bond in it.
        #[pallet::constant]
        type MaxOrderingWindowBytes: Get<u32>;

        /// Weight info.
        type WeightInfo: WeightInfo;
    }

    // ──────────────────────────────────────────────────────────────
    // Storage
    // ──────────────────────────────────────────────────────────────

    /// Registered confidential validators with attestation data.
    #[pallet::storage]
    #[pallet::getter(fn confidential_validators)]
    pub type ConfidentialValidators<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, EnclaveAttestation<T>, OptionQuery>;

    /// Number of registered confidential validators.
    #[pallet::storage]
    #[pallet::getter(fn confidential_validator_count)]
    pub type ConfidentialValidatorCount<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Private transaction records.
    #[pallet::storage]
    #[pallet::getter(fn private_transactions)]
    pub type PrivateTransactions<T: Config> =
        StorageMap<_, Blake2_128Concat, sp_core::H256, PrivateTxRecord<T>, OptionQuery>;

    /// Encrypted state diffs committed per block.
    #[pallet::storage]
    #[pallet::getter(fn encrypted_state_diffs)]
    pub type EncryptedStateDiffs<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,
        BoundedVec<EncryptedDiff, T::MaxDiffsPerBlock>,
        ValueQuery,
    >;

    /// DKG committee public key (threshold encryption key).
    #[pallet::storage]
    #[pallet::getter(fn committee_public_key)]
    pub type CommitteePublicKey<T: Config> =
        StorageValue<_, BoundedVec<u8, ConstU32<64>>, OptionQuery>;

    /// Current DKG epoch number.
    #[pallet::storage]
    #[pallet::getter(fn dkg_epoch)]
    pub type DkgEpoch<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// The DKG epoch each accepted private transaction was encrypted for.
    ///
    /// New storage rather than a field on `PrivateTxRecord`, so no migration is needed. A
    /// submission is only decryptable by the committee whose key it was encrypted to, so the epoch
    /// travels with the record: the committee that can open it is identifiable without decoding
    /// the payload again.
    #[pallet::storage]
    #[pallet::getter(fn private_tx_epoch)]
    pub type PrivateTxEpoch<T: Config> =
        StorageMap<_, Blake2_128Concat, sp_core::H256, u64, OptionQuery>;

    /// Whether commit-reveal ordering windows may be opened and committed into.
    ///
    /// Off by default, set by the same `AdminOrigin` as the rest of this pallet's switches.
    ///
    /// This is deliberately *not* `Enabled` (private execution) or the confidential-validator
    /// quorum. Those belong to the confidential path, and on this runtime they can never be
    /// satisfied: `AttestationVerifier = RefuseAllAttestations` in the runtime config refuses every
    /// report, so no confidential validator can register and the quorum is never met. Gating a
    /// public commit-reveal lane on them made the whole ordering feature unreachable on a live
    /// chain — measured 2026-09-26, and the reason a live drill could not exist. The lane orders
    /// opaque commitments and needs none of the confidential machinery; it gets its own switch,
    /// and a chain that has not turned it on still refuses to open a window.
    #[pallet::storage]
    #[pallet::getter(fn ordering_windows_enabled)]
    pub type OrderingWindowsEnabled<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// Total private transactions processed.
    #[pallet::storage]
    #[pallet::getter(fn total_private_txs)]
    pub type TotalPrivateTxs<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Total premium fees collected.
    #[pallet::storage]
    #[pallet::getter(fn total_premium_fees)]
    pub type TotalPremiumFees<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// Whether private execution is enabled.
    #[pallet::storage]
    #[pallet::getter(fn is_enabled)]
    pub type Enabled<T: Config> = StorageValue<_, bool, ValueQuery>;

    // ──────────────────────────────────────────────────────────────
    // Commit–reveal ordering window storage (X3-MEV-006 / X3-MEV-008)
    // ──────────────────────────────────────────────────────────────

    /// Id that the next opened ordering window will receive.
    #[pallet::storage]
    #[pallet::getter(fn next_ordering_window_id)]
    pub type NextOrderingWindowId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Windows by id.
    #[pallet::storage]
    #[pallet::getter(fn ordering_windows)]
    pub type OrderingWindows<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, OrderingWindowRecord<T>, OptionQuery>;

    /// Commitments by `(window id, commit hash)`.
    #[pallet::storage]
    #[pallet::getter(fn ordering_commits)]
    pub type OrderingCommits<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,
        Blake2_128Concat,
        sp_core::H256,
        OrderingCommitment<T>,
        OptionQuery,
    >;

    /// The one commitment each account may place per window, by `(window id, account)`.
    ///
    /// This is the authoritative "one commitment per sender per window" check: the
    /// lane also refuses a second commitment from the same 20-byte label, but the
    /// account is what can actually be slashed, so the account is what is keyed.
    #[pallet::storage]
    #[pallet::getter(fn ordering_commit_by_sender)]
    pub type OrderingCommitBySender<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,
        Blake2_128Concat,
        T::AccountId,
        sp_core::H256,
        OptionQuery,
    >;

    /// Reveals by `(window id, commit hash)`.
    #[pallet::storage]
    #[pallet::getter(fn ordering_reveals)]
    pub type OrderingReveals<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,
        Blake2_128Concat,
        sp_core::H256,
        OrderingReveal,
        OptionQuery,
    >;

    /// Settled canonical order per window id.
    #[pallet::storage]
    #[pallet::getter(fn ordering_settlements)]
    pub type OrderingSettlements<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, OrderingSettlementRecord, OptionQuery>;

    // ──────────────────────────────────────────────────────────────
    // Events
    // ──────────────────────────────────────────────────────────────

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A validator registered as confidential with attestation.
        ConfidentialValidatorRegistered {
            validator: T::AccountId,
            gpu_model: Vec<u8>,
        },
        /// A confidential validator was removed.
        ConfidentialValidatorRemoved { validator: T::AccountId },
        /// Attestation was refreshed.
        AttestationRefreshed { validator: T::AccountId, epoch: u64 },
        /// A private transaction was submitted.
        PrivateTxSubmitted {
            tx_hash: sp_core::H256,
            sender: T::AccountId,
            fee: BalanceOf<T>,
        },
        /// A private transaction was executed inside enclave.
        PrivateTxExecuted {
            tx_hash: sp_core::H256,
            enclave_validator: T::AccountId,
        },
        /// An encrypted state diff was committed.
        StateDiffCommitted {
            tx_hash: sp_core::H256,
            block_number: BlockNumberFor<T>,
            has_zk_proof: bool,
        },
        /// DKG key rotation completed.
        DkgKeyRotated {
            epoch: u64,
            validators_participating: u32,
        },
        /// Premium fee distributed.
        PremiumFeeDistributed {
            tx_hash: sp_core::H256,
            validator_share: BalanceOf<T>,
            burned: BalanceOf<T>,
            staker_share: BalanceOf<T>,
        },
        /// Private execution enabled/disabled.
        PrivateExecutionToggled { enabled: bool },
        /// Commit-reveal ordering windows were enabled or disabled.
        OrderingWindowsToggled { enabled: bool },
        /// A commit–reveal ordering window was opened.
        OrderingWindowOpened {
            window_id: u64,
            open_block: u64,
            close_block: u64,
            minimum_bond: BalanceOf<T>,
        },
        /// A commitment was recorded against an ordering window.
        OrderingCommitted {
            window_id: u64,
            sender: T::AccountId,
            commit_hash: sp_core::H256,
            bond: BalanceOf<T>,
        },
        /// A commitment was revealed and its bond released.
        OrderingRevealed {
            window_id: u64,
            sender: T::AccountId,
            commit_hash: sp_core::H256,
        },
        /// An ordering beacon was installed for a closed window.
        OrderingBeaconInstalled {
            window_id: u64,
            beacon: sp_core::H256,
        },
        /// A window settled into its one canonical order.
        ///
        /// `order_digest` is `blake2_256` over the SCALE encoding of the stored
        /// canonical order, so a verifier can check that the order it recomputed
        /// is the one the chain recorded without the event carrying the whole
        /// sequence.
        OrderingWindowSettled {
            window_id: u64,
            ordered: u32,
            unrevealed: u32,
            forfeited_bond: BalanceOf<T>,
            order_digest: sp_core::H256,
        },
    }

    // ──────────────────────────────────────────────────────────────
    // Errors
    // ──────────────────────────────────────────────────────────────

    #[pallet::error]
    pub enum Error<T> {
        /// Validator already registered as confidential.
        AlreadyRegistered,
        /// Validator not found in confidential set.
        ValidatorNotFound,
        /// Invalid or expired attestation report.
        InvalidAttestation,
        /// Attestation has expired and needs refresh.
        AttestationExpired,
        /// Not enough confidential validators for quorum.
        InsufficientQuorum,
        /// Maximum confidential validators reached.
        MaxValidatorsReached,
        /// Private execution is disabled.
        PrivateExecutionDisabled,
        /// State diff limit per block exceeded.
        MaxDiffsExceeded,
        /// Encrypted payload too large.
        PayloadTooLarge,
        /// Transaction already exists.
        TxAlreadyExists,
        /// Transaction not found.
        TxNotFound,
        /// Not a confidential validator.
        NotConfidentialValidator,
        /// DKG committee key not yet established.
        NoDkgKey,
        /// ZK proof verification failed.
        ZkProofInvalid,
        /// Fee calculation overflow.
        ArithmeticOverflow,
        /// Insufficient balance for premium fee.
        InsufficientBalance,
        // ── Commit–reveal ordering window ────────────────────────────
        /// The ordering window's close block precedes its open block.
        OrderingWindowInverted,
        /// The ordering window's close block has already passed.
        OrderingWindowAlreadyClosed,
        /// No ordering window exists with that id.
        OrderingWindowNotFound,
        /// The current block is outside the ordering window.
        OrderingWindowNotOpen,
        /// The ordering window is still open and cannot settle yet.
        OrderingWindowStillOpen,
        /// The ordering window has already settled; there is one order per window.
        OrderingWindowSettled,
        /// The posted bond is below the window's minimum.
        OrderingBondBelowMinimum,
        /// This account already committed in this window.
        OrderingAlreadyCommitted,
        /// This commit hash is already recorded in this window.
        OrderingCommitAlreadyUsed,
        /// The window already holds its maximum number of commitments.
        OrderingWindowFull,
        /// The reveal names a commit hash this window never recorded.
        OrderingUnknownCommitment,
        /// The reveal came from an account that does not own the commitment.
        OrderingNotYourCommitment,
        /// The revealed payload and nonce do not hash to the committed hash.
        OrderingRevealMismatch,
        /// This commitment was already revealed.
        OrderingAlreadyRevealed,
        /// The revealed plaintext is above the lane's size limit.
        OrderingPlaintextTooLarge,
        /// The reveal would take the window above its total plaintext budget.
        OrderingWindowBytesExceeded,
        /// An ordering beacon cannot be installed while the window is open.
        OrderingBeaconWhileOpen,
        /// This window already has an ordering beacon.
        OrderingBeaconAlreadySet,
        /// The window's beacon block has no hash yet, or the chain reports none.
        ///
        /// A refusal, not a fallback: a beacon of zero is a value an attacker can assume, and
        /// settling without the chain's own beacon would let the installer's timing decide the
        /// order — which is the hole this beacon exists to close.
        OrderingBeaconUnavailable,
        /// The stored beacon is not the one this window's close block produces.
        OrderingBeaconMismatch,
        /// The reserved bond was not exactly the amount the commitment posted.
        OrderingBondNotReserved,
        /// The unrevealed bond could not be removed in full.
        OrderingBondNotForfeitable,
        /// Ordering windows are not enabled on this chain.
        ///
        /// Set by `set_ordering_windows_enabled` (AdminOrigin). Distinct from
        /// `PrivateExecutionDisabled` on purpose: the two features are switched separately, and a
        /// chain can order commitments without offering a confidential execution path.
        OrderingWindowsDisabled,
        /// The submitted payload is not a well-formed encrypted transaction.
        ///
        /// Before this existed the pallet stored whatever bytes it was handed — no shape, no
        /// epoch, no committee binding — so a payload nothing in the committee could ever decrypt
        /// was accepted and its fee escrowed.
        InvalidEncryptedPayload,
        /// The `tx_hash` argument is not the id inside the payload.
        EncryptedPayloadHashMismatch,
        /// The payload was encrypted for a different DKG epoch than the committee's current one.
        ///
        /// Unit-only: `frame_support` caps a pallet error at four encoded bytes, and the two epochs
        /// would blow that cap. The operator-facing detail (both epochs) goes in the log line below
        /// rather than in the dispatch error.
        EncryptedPayloadEpochMismatch,
        /// The stored committee key cannot be used as a threshold public key.
        CommitteeKeyUnusable,
        /// Stored window state was refused by the lane when replayed.
        ///
        /// A window that the chain accepted a commitment into must replay. This
        /// error means storage and the lane disagree, which is a fail-closed
        /// condition, never a success.
        OrderingWindowStateCorrupt,
    }

    // ──────────────────────────────────────────────────────────────
    // Genesis
    // ──────────────────────────────────────────────────────────────

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        pub enabled: bool,
        #[serde(skip)]
        pub _phantom: sp_std::marker::PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            Enabled::<T>::put(self.enabled);
            // Validate fee split
            let total = T::ConfidentialValidatorShareBps::get() as u32
                + T::PrivateBurnShareBps::get() as u32
                + T::PrivateStakerShareBps::get() as u32;
            assert_eq!(
                total, 10_000,
                "Private execution fee split must sum to 10_000 bps"
            );
        }
    }

    // ──────────────────────────────────────────────────────────────
    // Hooks
    // ──────────────────────────────────────────────────────────────

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_now: BlockNumberFor<T>) -> Weight {
            Weight::zero()
        }
    }

    // ──────────────────────────────────────────────────────────────
    // Dispatchables
    // ──────────────────────────────────────────────────────────────

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register as a confidential validator with GPU enclave attestation.
        ///
        /// # Invariant: PRIV-EXEC-004
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(2, 2))]
        pub fn register_confidential_validator(
            origin: OriginFor<T>,
            gpu_model: Vec<u8>,
            attestation_report: Vec<u8>,
            enclave_public_key: [u8; 32],
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(Enabled::<T>::get(), Error::<T>::PrivateExecutionDisabled);
            ensure!(
                !ConfidentialValidators::<T>::contains_key(&who),
                Error::<T>::AlreadyRegistered
            );
            ensure!(
                ConfidentialValidatorCount::<T>::get() < T::MaxConfidentialValidators::get(),
                Error::<T>::MaxValidatorsReached
            );

            ensure!(
                Self::verify_attestation(&attestation_report, &gpu_model, &enclave_public_key),
                Error::<T>::InvalidAttestation
            );

            let now = <frame_system::Pallet<T>>::block_number();

            let attestation = EnclaveAttestation {
                validator: who.clone(),
                gpu_model: BoundedVec::try_from(gpu_model.clone())
                    .map_err(|_| Error::<T>::PayloadTooLarge)?,
                attestation_report: BoundedVec::try_from(attestation_report)
                    .map_err(|_| Error::<T>::PayloadTooLarge)?,
                enclave_public_key,
                last_refreshed: now,
                status: EnclaveStatus::Verified,
            };

            ConfidentialValidators::<T>::insert(&who, attestation);
            ConfidentialValidatorCount::<T>::mutate(|c| *c = c.saturating_add(1));

            Self::deposit_event(Event::ConfidentialValidatorRegistered {
                validator: who,
                gpu_model,
            });

            Ok(())
        }

        /// Remove a confidential validator from the set.
        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 2))]
        pub fn deregister_confidential_validator(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                ConfidentialValidators::<T>::contains_key(&who),
                Error::<T>::ValidatorNotFound
            );

            ConfidentialValidators::<T>::remove(&who);
            ConfidentialValidatorCount::<T>::mutate(|c| *c = c.saturating_sub(1));

            Self::deposit_event(Event::ConfidentialValidatorRemoved { validator: who });
            Ok(())
        }

        /// Refresh attestation report (required periodically).
        #[pallet::call_index(2)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn refresh_attestation(
            origin: OriginFor<T>,
            new_attestation_report: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ConfidentialValidators::<T>::try_mutate(&who, |maybe_att| -> DispatchResult {
                let att = maybe_att.as_mut().ok_or(Error::<T>::ValidatorNotFound)?;

                ensure!(
                    Self::verify_attestation(
                        &new_attestation_report,
                        att.gpu_model.as_slice(),
                        &att.enclave_public_key
                    ),
                    Error::<T>::InvalidAttestation
                );

                att.attestation_report = BoundedVec::try_from(new_attestation_report)
                    .map_err(|_| Error::<T>::PayloadTooLarge)?;
                att.last_refreshed = <frame_system::Pallet<T>>::block_number();
                att.status = EnclaveStatus::Verified;

                Ok(())
            })?;

            let epoch = DkgEpoch::<T>::get();
            Self::deposit_event(Event::AttestationRefreshed {
                validator: who,
                epoch,
            });

            Ok(())
        }

        /// Submit an encrypted private transaction.
        ///
        /// # Invariant: PRIV-EXEC-001, PRIV-EXEC-005
        #[pallet::call_index(3)]
        #[pallet::weight(T::DbWeight::get().reads_writes(3, 2))]
        pub fn submit_private_transaction(
            origin: OriginFor<T>,
            tx_hash: sp_core::H256,
            encrypted_payload: Vec<u8>,
            fee_commitment: sp_core::H256,
            priority_fee: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(Enabled::<T>::get(), Error::<T>::PrivateExecutionDisabled);
            ensure!(
                ConfidentialValidatorCount::<T>::get() >= T::MinConfidentialQuorum::get(),
                Error::<T>::InsufficientQuorum
            );
            ensure!(
                CommitteePublicKey::<T>::get().is_some(),
                Error::<T>::NoDkgKey
            );
            ensure!(
                !PrivateTransactions::<T>::contains_key(tx_hash),
                Error::<T>::TxAlreadyExists
            );
            ensure!(
                encrypted_payload.len() <= T::MaxEncryptedPayloadSize::get() as usize,
                Error::<T>::PayloadTooLarge
            );

            // Decode and validate *before* any fee moves. The payload is a SCALE-encoded
            // `EncryptedTransaction` (the format `x3-threshold-core` defines, which is also what
            // `private-mempool` re-exports), and it has to be a submission this committee can
            // actually open: a well-formed record, naming this chain's committee key epoch, whose
            // id is the hash of its own ciphertext and matches the `tx_hash` the caller declared.
            //
            // Measured before this check existed: `submit_private_transaction` accepted
            // `vec![0xCA; 256]` with an arbitrary `tx_hash`, charged the premium and escrowed it,
            // and no validator could ever decrypt it.
            let decoded = EncryptedTransaction::decode(&mut &encrypted_payload[..])
                .map_err(|_| Error::<T>::InvalidEncryptedPayload)?;
            ensure!(
                decoded.id == tx_hash.0,
                Error::<T>::EncryptedPayloadHashMismatch
            );

            let committee_bytes = CommitteePublicKey::<T>::get().ok_or(Error::<T>::NoDkgKey)?;
            ensure!(
                committee_bytes.len() == 32,
                Error::<T>::CommitteeKeyUnusable
            );
            let mut group_key = [0u8; 32];
            group_key.copy_from_slice(&committee_bytes);
            let committee = ThresholdPublicKey {
                group_key,
                epoch: Self::dkg_epoch(),
                threshold: T::MinConfidentialQuorum::get(),
                committee_size: ConfidentialValidatorCount::<T>::get(),
            };
            validate_encrypted_transaction(&decoded, &committee).map_err(|refusal| match refusal {
                ThresholdRefusal::WrongEpoch { expected, got } => {
                    log::warn!(
                        target: "runtime::private-execution",
                        "refused a private submission encrypted for epoch {got} while the committee is at {expected}"
                    );
                    Error::<T>::EncryptedPayloadEpochMismatch
                }
                ThresholdRefusal::InvalidCommitteeKey { .. } => Error::<T>::CommitteeKeyUnusable,
                _ => Error::<T>::InvalidEncryptedPayload,
            })?;

            // Store the *canonical re-encoding* of the record that was validated, so the bytes this
            // pallet holds are the bytes it checked.
            let canonical_payload = decoded.encode();
            // Collect premium fee
            let base_fee = priority_fee;
            let premium_bps = T::PrivateFeePremiumBps::get() as u128;
            let premium = base_fee.saturating_mul(
                premium_bps
                    .try_into()
                    .map_err(|_| Error::<T>::ArithmeticOverflow)?,
            ) / 10_000u32.into();
            let total_fee = base_fee.saturating_add(premium);

            ensure!(
                T::Currency::free_balance(&who) >= total_fee,
                Error::<T>::InsufficientBalance
            );

            let escrow_account = Self::account_id();
            T::Currency::transfer(
                &who,
                &escrow_account,
                total_fee,
                ExistenceRequirement::KeepAlive,
            )?;

            TotalPremiumFees::<T>::mutate(|f| *f = f.saturating_add(premium));

            let now = <frame_system::Pallet<T>>::block_number();

            let record = PrivateTxRecord {
                tx_hash,
                sender: who.clone(),
                encrypted_payload: BoundedVec::try_from(canonical_payload)
                    .map_err(|_| Error::<T>::PayloadTooLarge)?,
                fee_commitment,
                fee_paid: total_fee.saturated_into(),
                status: PrivateTxStatus::Pending,
                submitted_at: now,
                executed_by: None,
            };

            PrivateTransactions::<T>::insert(tx_hash, record);
            PrivateTxEpoch::<T>::insert(tx_hash, decoded.dkg_epoch);
            TotalPrivateTxs::<T>::mutate(|t| *t = t.saturating_add(1));

            Self::deposit_event(Event::PrivateTxSubmitted {
                tx_hash,
                sender: who,
                fee: total_fee,
            });

            Ok(())
        }

        /// Commit an encrypted state diff from enclave execution.
        ///
        /// # Invariant: PRIV-EXEC-002
        #[pallet::call_index(4)]
        #[pallet::weight(T::DbWeight::get().reads_writes(3, 3))]
        pub fn commit_encrypted_state_diff(
            origin: OriginFor<T>,
            tx_hash: sp_core::H256,
            encrypted_state_changes: Vec<u8>,
            commitment: sp_core::H256,
            zk_proof: Option<Vec<u8>>,
            enclave_signature: [u8; 64],
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Must be a confidential validator
            let att = ConfidentialValidators::<T>::get(&who)
                .ok_or(Error::<T>::NotConfidentialValidator)?;
            ensure!(
                att.status == EnclaveStatus::Verified,
                Error::<T>::AttestationExpired
            );

            // TX must exist and be pending
            PrivateTransactions::<T>::try_mutate(tx_hash, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::TxNotFound)?;
                record.status = PrivateTxStatus::Committed;
                record.executed_by = Some(who.clone());
                Ok(())
            })?;

            let now = <frame_system::Pallet<T>>::block_number();
            let has_zk_proof = zk_proof.is_some();

            let diff = EncryptedDiff {
                tx_hash,
                encrypted_state_changes: BoundedVec::try_from(encrypted_state_changes)
                    .map_err(|_| Error::<T>::PayloadTooLarge)?,
                commitment,
                zk_proof: zk_proof.map(|p| BoundedVec::try_from(p).unwrap_or_default()),
                enclave_signature,
                committed_at: now.saturated_into(),
            };

            EncryptedStateDiffs::<T>::try_mutate(now, |diffs| -> DispatchResult {
                diffs
                    .try_push(diff)
                    .map_err(|_| Error::<T>::MaxDiffsExceeded)?;
                Ok(())
            })?;

            // Distribute premium fee to the executing validator
            if let Some(record) = PrivateTransactions::<T>::get(tx_hash) {
                let fee: BalanceOf<T> = record.fee_paid.saturated_into();
                Self::distribute_premium_fee(tx_hash, &who, fee)?;
            }

            Self::deposit_event(Event::StateDiffCommitted {
                tx_hash,
                block_number: now,
                has_zk_proof,
            });

            Ok(())
        }

        /// Set the DKG committee public key (called after DKG ceremony).
        #[pallet::call_index(5)]
        #[pallet::weight(T::DbWeight::get().writes(2))]
        pub fn set_committee_key(origin: OriginFor<T>, public_key: Vec<u8>) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            let bounded =
                BoundedVec::try_from(public_key).map_err(|_| Error::<T>::PayloadTooLarge)?;
            CommitteePublicKey::<T>::put(bounded);

            let new_epoch = DkgEpoch::<T>::mutate(|e| {
                *e = e.saturating_add(1);
                *e
            });

            Self::deposit_event(Event::DkgKeyRotated {
                epoch: new_epoch,
                validators_participating: ConfidentialValidatorCount::<T>::get(),
            });

            Ok(())
        }

        /// Enable or disable private execution (admin only).
        #[pallet::call_index(6)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn set_enabled(origin: OriginFor<T>, enabled: bool) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            Enabled::<T>::put(enabled);

            Self::deposit_event(Event::PrivateExecutionToggled { enabled });
            Ok(())
        }

        /// Enable or disable commit-reveal ordering windows (admin only).
        ///
        /// Separate from `set_enabled` on purpose: ordering windows are a public mechanism over
        /// opaque commitments and do not need a confidential execution path. On this runtime the
        /// confidential path is unreachable anyway (`AttestationVerifier = RefuseAllAttestations`),
        /// so tying the two together left the ordering lane with no live entry point at all.
        #[pallet::call_index(12)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn set_ordering_windows_enabled(origin: OriginFor<T>, enabled: bool) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            OrderingWindowsEnabled::<T>::put(enabled);

            Self::deposit_event(Event::OrderingWindowsToggled { enabled });
            Ok(())
        }

        // ────────────────────────────────────────────────────────────
        // Commit–reveal ordering window (X3-MEV-006 / X3-MEV-008)
        // ────────────────────────────────────────────────────────────

        /// Open a commit–reveal ordering window.
        ///
        /// A window fixes the block range a commit and its reveal must both land
        /// inside, and the bond every commitment must post. Opening requires the
        /// same guards private submission does (`Enabled` and the confidential
        /// quorum), because a window is new exposure: it is opened by anyone, and
        /// its participants' payloads are handed on in the canonical order at
        /// settle. Reveals and settles deliberately do *not* carry those guards —
        /// see [`Pallet::reveal_ordering`].
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::open_ordering_window())]
        pub fn open_ordering_window(
            origin: OriginFor<T>,
            open_block: u64,
            close_block: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_ordering_accepts_new_commitments()?;

            let window = OrderingWindow::new(open_block, close_block)
                .map_err(|_| Error::<T>::OrderingWindowInverted)?;

            let now: u64 = <frame_system::Pallet<T>>::block_number().saturated_into();
            // A window that has already closed can accept nothing, so opening one
            // would be a promise the chain cannot keep. Refuse rather than record.
            ensure!(
                now <= window.close_block,
                Error::<T>::OrderingWindowAlreadyClosed
            );

            let minimum_bond: BalanceOf<T> = T::MinOrderingBond::get();
            let window_id = NextOrderingWindowId::<T>::get();
            NextOrderingWindowId::<T>::put(window_id.saturating_add(1));

            OrderingWindows::<T>::insert(
                window_id,
                OrderingWindowRecord::<T> {
                    opened_by: who,
                    open_block: window.open_block,
                    close_block: window.close_block,
                    minimum_bond: minimum_bond.saturated_into(),
                    opened_at: <frame_system::Pallet<T>>::block_number(),
                    beacon: None,
                    settled: false,
                    commitment_count: 0,
                    reveal_count: 0,
                    revealed_bytes: 0,
                },
            );

            Self::deposit_event(Event::OrderingWindowOpened {
                window_id,
                open_block: window.open_block,
                close_block: window.close_block,
                minimum_bond,
            });

            Ok(())
        }

        /// Commit to an ordering window, reserving `bond`.
        ///
        /// The caller must have computed
        /// `x3_order_window::commitment_hash(<its ordering label>, plaintext, nonce)`;
        /// the plaintext stays off-chain until the reveal, which is the point of
        /// the lane. The bond is reserved here and released on reveal, or
        /// forfeited at settle if the commitment never reveals.
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::commit_ordering())]
        pub fn commit_ordering(
            origin: OriginFor<T>,
            window_id: u64,
            commit_hash: sp_core::H256,
            bond: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_ordering_accepts_new_commitments()?;

            let record =
                OrderingWindows::<T>::get(window_id).ok_or(Error::<T>::OrderingWindowNotFound)?;
            ensure!(!record.settled, Error::<T>::OrderingWindowSettled);

            let now: u64 = <frame_system::Pallet<T>>::block_number().saturated_into();
            ensure!(
                now >= record.open_block && now <= record.close_block,
                Error::<T>::OrderingWindowNotOpen
            );

            let minimum_bond: BalanceOf<T> = record.minimum_bond.saturated_into();
            ensure!(bond >= minimum_bond, Error::<T>::OrderingBondBelowMinimum);

            ensure!(
                record.commitment_count < T::MaxOrderingCommits::get(),
                Error::<T>::OrderingWindowFull
            );
            ensure!(
                !OrderingCommitBySender::<T>::contains_key(window_id, &who),
                Error::<T>::OrderingAlreadyCommitted
            );
            ensure!(
                !OrderingCommits::<T>::contains_key(window_id, commit_hash),
                Error::<T>::OrderingCommitAlreadyUsed
            );

            // Reserve before recording: a commitment the chain cannot back with a
            // held bond must not exist, because the bond is the only thing that
            // makes a silent commit cost anything.
            T::Currency::reserve(&who, bond)?;

            OrderingCommits::<T>::insert(
                window_id,
                commit_hash,
                OrderingCommitment::<T> {
                    sender: who.clone(),
                    bond: bond.saturated_into(),
                    committed_at_block: now,
                },
            );
            OrderingCommitBySender::<T>::insert(window_id, &who, commit_hash);
            OrderingWindows::<T>::mutate(window_id, |maybe| {
                if let Some(record) = maybe.as_mut() {
                    record.commitment_count = record.commitment_count.saturating_add(1);
                }
            });

            Self::deposit_event(Event::OrderingCommitted {
                window_id,
                sender: who,
                commit_hash,
                bond,
            });

            Ok(())
        }

        /// Reveal a commitment, releasing its bond.
        ///
        /// Deliberately *not* gated on `Enabled` or the confidential quorum, even
        /// though commits are: a gate on reveal would let a quorum drop, or an
        /// admin toggling the switch off, strand bonds that committed while both
        /// guards held. Refusing to close out an existing window is the one
        /// failure mode a gate here would create, so reveal and settle stay
        /// open and only the checks that protect the window itself apply.
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::reveal_ordering())]
        pub fn reveal_ordering(
            origin: OriginFor<T>,
            window_id: u64,
            commit_hash: sp_core::H256,
            plaintext: Vec<u8>,
            nonce: [u8; 32],
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let record =
                OrderingWindows::<T>::get(window_id).ok_or(Error::<T>::OrderingWindowNotFound)?;
            ensure!(!record.settled, Error::<T>::OrderingWindowSettled);

            let now: u64 = <frame_system::Pallet<T>>::block_number().saturated_into();
            ensure!(
                now >= record.open_block && now <= record.close_block,
                Error::<T>::OrderingWindowNotOpen
            );

            ensure!(
                plaintext.len() <= MAX_PLAINTEXT_BYTES,
                Error::<T>::OrderingPlaintextTooLarge
            );
            let reveal_len = plaintext.len() as u32;
            // Bound the window's total, not just each reveal: settle reads all of
            // them in one transaction, so an unbounded total is a window that can
            // never be settled and bonds that can never be resolved.
            ensure!(
                record.revealed_bytes.saturating_add(reveal_len)
                    <= T::MaxOrderingWindowBytes::get(),
                Error::<T>::OrderingWindowBytesExceeded
            );

            let commitment = OrderingCommits::<T>::get(window_id, commit_hash)
                .ok_or(Error::<T>::OrderingUnknownCommitment)?;
            ensure!(
                commitment.sender == who,
                Error::<T>::OrderingNotYourCommitment
            );
            ensure!(
                !OrderingReveals::<T>::contains_key(window_id, commit_hash),
                Error::<T>::OrderingAlreadyRevealed
            );

            // The reveal must reproduce the commitment, label included, so a
            // commitment lifted from the mempool cannot be revealed by anyone
            // else and a substituted payload cannot be swapped in.
            ensure!(
                commitment_hash(Self::ordering_sender_label(&who), &plaintext, &nonce)
                    == commit_hash,
                Error::<T>::OrderingRevealMismatch
            );

            let bond: BalanceOf<T> = commitment.bond.saturated_into();
            // `unreserve` returns the amount it could *not* release, so a
            // commitment whose reserve is smaller than the bond it recorded is
            // a fail-closed condition rather than a partial release.
            let not_released = T::Currency::unreserve(&who, bond);
            ensure!(not_released.is_zero(), Error::<T>::OrderingBondNotReserved);

            OrderingReveals::<T>::insert(
                window_id,
                commit_hash,
                OrderingReveal {
                    plaintext,
                    nonce,
                    revealed_at_block: now,
                },
            );
            OrderingWindows::<T>::mutate(window_id, |maybe| {
                if let Some(record) = maybe.as_mut() {
                    record.reveal_count = record.reveal_count.saturating_add(1);
                    record.revealed_bytes = record.revealed_bytes.saturating_add(reveal_len);
                }
            });

            Self::deposit_event(Event::OrderingRevealed {
                window_id,
                sender: who,
                commit_hash,
            });

            Ok(())
        }

        /// Install the ordering beacon for a closed window.
        ///
        /// The beacon is what closes the placement hole a commit–reveal lane has
        /// on its own: with no beacon the order key is the commit hash, so a
        /// participant that can try many nonces can choose where its own
        /// commitment lands. Installing it is refused while the window is open,
        /// because a value participants could still commit against would not be a
        /// beacon.
        ///
        /// The value is **derived from the chain**, not supplied: it is the hash of the block right
        /// after the window closed. Until 2026-09-26 this extrinsic took a caller-supplied `beacon`
        /// and was admin-gated, which left the same hole in a different place — whoever could call
        /// it chose the value every order key is folded with. Deriving it removes the choice, and
        /// the call is permissionless for the same reason settlement is: nothing it does depends on
        /// who asks.
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::install_ordering_beacon())]
        pub fn install_ordering_beacon(origin: OriginFor<T>, window_id: u64) -> DispatchResult {
            ensure_signed(origin)?;

            let record =
                OrderingWindows::<T>::get(window_id).ok_or(Error::<T>::OrderingWindowNotFound)?;
            ensure!(!record.settled, Error::<T>::OrderingWindowSettled);
            let now: u64 = <frame_system::Pallet<T>>::block_number().saturated_into();
            ensure!(
                now > record.close_block,
                Error::<T>::OrderingBeaconWhileOpen
            );
            ensure!(
                record.beacon.is_none(),
                Error::<T>::OrderingBeaconAlreadySet
            );

            let beacon = Self::derive_ordering_beacon(window_id)?;
            OrderingWindows::<T>::mutate(window_id, |maybe| {
                if let Some(record) = maybe.as_mut() {
                    record.beacon = Some(beacon);
                }
            });

            Self::deposit_event(Event::OrderingBeaconInstalled { window_id, beacon });
            Ok(())
        }

        /// Settle a closed window into its canonical order.
        ///
        /// Permissionless: the outcome is a pure function of the closed
        /// commitment set in storage, so the caller cannot influence it and the
        /// chain does not depend on one account staying up to settle. Unrevealed
        /// commitments forfeit their bonds here.
        #[pallet::call_index(11)]
        // Charged for what the window actually holds, not for its capacity: a two-commit window
        // must not cost what a full one costs, or the settle is refused by the pool and the bonds
        // it holds can never be released (measured on local3, 2026-09-26).
        #[pallet::weight(Pallet::<T>::settle_ordering_weight(*window_id))]
        pub fn settle_ordering_window(origin: OriginFor<T>, window_id: u64) -> DispatchResult {
            ensure_signed(origin)?;

            let record =
                OrderingWindows::<T>::get(window_id).ok_or(Error::<T>::OrderingWindowNotFound)?;
            ensure!(!record.settled, Error::<T>::OrderingWindowSettled);

            let now: u64 = <frame_system::Pallet<T>>::block_number().saturated_into();
            ensure!(
                now > record.close_block,
                Error::<T>::OrderingWindowStillOpen
            );

            // The beacon is the chain's, and settlement does not depend on someone having called
            // the installer: derive it here, take it if the window has none, and refuse if a stored
            // value is not the one this close block produces. That makes the order a function of
            // the commitment set *and* a block hash no participant could see while committing.
            let beacon = Self::derive_ordering_beacon(window_id)?;
            if let Some(stored) = record.beacon {
                ensure!(stored == beacon, Error::<T>::OrderingBeaconMismatch);
            } else {
                OrderingWindows::<T>::mutate(window_id, |maybe| {
                    if let Some(record) = maybe.as_mut() {
                        record.beacon = Some(beacon);
                    }
                });
            }
            let mut record = record;
            record.beacon = Some(beacon);

            let settlement = Self::settle_ordering_lane(window_id, &record, now)?;

            // Forfeit the bonds of commitments that never revealed. The lane only
            // reports them (it does not move a bond); applying it is this
            // pallet's job, and it happens before the window is marked settled so
            // a partial application cannot leave a settled window holding bonds.
            let mut forfeited: BalanceOf<T> = 0u32.into();
            let mut unrevealed_hashes: Vec<sp_core::H256> =
                Vec::with_capacity(settlement.unrevealed.len());
            for commitment in &settlement.unrevealed {
                let sender = OrderingCommits::<T>::get(window_id, commitment.commit_hash)
                    .ok_or(Error::<T>::OrderingWindowStateCorrupt)?
                    .sender;
                let bond: BalanceOf<T> = commitment.bond.saturated_into();
                let (imbalance, remaining) = T::Currency::slash_reserved(&sender, bond);
                ensure!(remaining.is_zero(), Error::<T>::OrderingBondNotForfeitable);
                T::BurnDestination::on_unbalanced(imbalance);
                forfeited = forfeited.saturating_add(bond);
                unrevealed_hashes.push(commitment.commit_hash);
            }

            let ordered = settlement.canonical_order();
            let digest = sp_core::H256::from(sp_core::hashing::blake2_256(&ordered.encode()));

            OrderingSettlements::<T>::insert(
                window_id,
                OrderingSettlementRecord {
                    beacon: settlement.beacon,
                    ordered,
                    unrevealed: unrevealed_hashes,
                    forfeited_bond: forfeited.saturated_into(),
                    settled_at_block: now,
                },
            );
            OrderingWindows::<T>::mutate(window_id, |maybe| {
                if let Some(record) = maybe.as_mut() {
                    record.settled = true;
                }
            });

            Self::deposit_event(Event::OrderingWindowSettled {
                window_id,
                ordered: settlement.order.len() as u32,
                unrevealed: settlement.unrevealed.len() as u32,
                forfeited_bond: forfeited,
                order_digest: digest,
            });

            Ok(())
        }
    }

    // ──────────────────────────────────────────────────────────────
    // Helpers
    // ──────────────────────────────────────────────────────────────

    impl<T: Config> Pallet<T> {
        /// Get the escrow account for this pallet.
        pub fn account_id() -> T::AccountId {
            T::PalletId::get().into_account_truncating()
        }

        /// Verify an attestation report through the configured verifier.
        ///
        /// This delegates rather than guessing: a report, a GPU model and an enclave key
        /// are only bound together by the vendor's attestation chain, and the pallet has
        /// no trust root to check it against. `T::AttestationVerifier` supplies one, and
        /// its default refuses.
        fn verify_attestation(
            report: &[u8],
            gpu_model: &[u8],
            enclave_public_key: &[u8; 32],
        ) -> bool {
            T::AttestationVerifier::verify(report, gpu_model, enclave_public_key)
        }

        /// Distribute premium fees.
        ///
        /// # Invariant: PRIV-EXEC-005
        fn distribute_premium_fee(
            tx_hash: sp_core::H256,
            validator: &T::AccountId,
            total: BalanceOf<T>,
        ) -> DispatchResult {
            let validator_bps = T::ConfidentialValidatorShareBps::get() as u32;
            let burn_bps = T::PrivateBurnShareBps::get() as u32;

            let validator_share = Perbill::from_parts(validator_bps * 100_000) * total;
            let burn_share = Perbill::from_parts(burn_bps * 100_000) * total;
            let staker_share = total
                .saturating_sub(validator_share)
                .saturating_sub(burn_share);

            let escrow_account = Self::account_id();

            // Pay validator
            T::Currency::transfer(
                &escrow_account,
                validator,
                validator_share,
                ExistenceRequirement::AllowDeath,
            )?;

            // Burn
            let imbalance = T::Currency::slash(&escrow_account, burn_share).0;
            T::BurnDestination::on_unbalanced(imbalance);

            Self::deposit_event(Event::PremiumFeeDistributed {
                tx_hash,
                validator_share,
                burned: burn_share,
                staker_share,
            });

            Ok(())
        }

        // ── Commit–reveal ordering window helpers ────────────────────

        /// Guards for operations that add new exposure to an ordering window.
        ///
        /// These mirror `submit_private_transaction`: private execution must be
        /// on and there must be enough confidential validators to run it. They
        /// gate *opening* a window and *committing* to one. They deliberately do
        /// not gate revealing or settling — see `reveal_ordering`.
        fn ensure_ordering_accepts_new_commitments() -> DispatchResult {
            ensure!(
                OrderingWindowsEnabled::<T>::get(),
                Error::<T>::OrderingWindowsDisabled
            );
            Ok(())
        }

        /// The 20-byte label a commitment hash binds for `who`.
        ///
        /// The lane is keyed and hashed on `H160`, while the chain's account is
        /// generic. This is a domain-separated, deterministic truncation of the
        /// SCALE-encoded account — a label for the hash, not an address, and not
        /// an authority: the account that may reveal is the one recorded in
        /// `OrderingCommits`, checked by equality before the label is used.
        pub fn ordering_sender_label(who: &T::AccountId) -> sp_core::H160 {
            let digest = sp_core::hashing::blake2_256(&who.encode());
            sp_core::H160::from_slice(&digest[..20])
        }

        /// Rebuild the lane from storage and settle it.
        ///
        /// The window is replayed through the same `CommitRevealLane` its own
        /// tests exercise, rather than a second ordering rule written here: the
        /// chain's order and the tested order are one algorithm. Every step that
        /// the chain accepted at commit/reveal time must be accepted again; if it
        /// is not, storage and the lane disagree and this fails closed with
        /// `OrderingWindowStateCorrupt` instead of settling something else.
        fn settle_ordering_lane(
            window_id: u64,
            record: &OrderingWindowRecord<T>,
            now: u64,
        ) -> Result<x3_order_window::WindowSettlement, sp_runtime::DispatchError> {
            let window = OrderingWindow::new(record.open_block, record.close_block)
                .map_err(|_| Error::<T>::OrderingWindowInverted)?;
            let mut lane = CommitRevealLane::new(window, record.minimum_bond);

            for (commit_hash, commitment) in OrderingCommits::<T>::iter_prefix(window_id) {
                lane.commit(
                    Self::ordering_sender_label(&commitment.sender),
                    commit_hash,
                    commitment.bond,
                    commitment.committed_at_block,
                )
                .map_err(Self::map_fair_order_error)?;
            }

            for (commit_hash, reveal) in OrderingReveals::<T>::iter_prefix(window_id) {
                let commitment = OrderingCommits::<T>::get(window_id, commit_hash)
                    .ok_or(Error::<T>::OrderingWindowStateCorrupt)?;
                lane.reveal(
                    Self::ordering_sender_label(&commitment.sender),
                    commit_hash,
                    &reveal.plaintext,
                    &reveal.nonce,
                    reveal.revealed_at_block,
                )
                .map_err(Self::map_fair_order_error)?;
            }

            if let Some(beacon) = record.beacon {
                lane.install_beacon(beacon, now)
                    .map_err(Self::map_fair_order_error)?;
            }

            let settlement = lane.settle(now).map_err(Self::map_fair_order_error)?;

            // The settlement's published sequence and its recomputable canonical
            // order are the same by construction; assert it rather than trust it,
            // so a future change to the lane cannot quietly diverge the two.
            let published: Vec<sp_core::H256> = settlement
                .order
                .iter()
                .map(|entry| entry.commit_hash)
                .collect();
            ensure!(
                published == settlement.canonical_order(),
                Error::<T>::OrderingWindowStateCorrupt
            );
            for entry in &settlement.order {
                ensure!(
                    entry.order_key == order_key(settlement.beacon, &entry.commit_hash),
                    Error::<T>::OrderingWindowStateCorrupt
                );
            }

            Ok(settlement)
        }

        /// The weight a settle of `window_id` is charged: its recorded commitments and revealed
        /// bytes, or the configured capacity when the window does not exist yet (the dispatch then
        /// fails with `OrderingWindowNotFound`, so the charge only has to be non-zero).
        fn settle_ordering_weight(window_id: u64) -> Weight {
            match OrderingWindows::<T>::get(window_id) {
                Some(record) => T::WeightInfo::settle_ordering_window(
                    record.commitment_count,
                    record.revealed_bytes,
                ),
                None => T::WeightInfo::settle_ordering_window(T::MaxOrderingCommits::get(), 0),
            }
        }

        /// The block whose hash is a window's ordering beacon: the one right after it closed.
        ///
        /// Fixed by the window's own `close_block`, so neither the installer nor whoever settles
        /// chooses it, and it does not exist while participants can still commit.
        fn ordering_beacon_block(record: &OrderingWindowRecord<T>) -> u64 {
            record.close_block.saturating_add(1)
        }

        /// The chain's beacon for `window_id`, or a refusal.
        ///
        /// Zero means the chain reports no hash for that block — too early, or pruned. That is a
        /// refusal rather than a fallback: a zero beacon is a constant an attacker can assume, and
        /// folding a value the caller could pick is exactly the hole the beacon closes.
        fn derive_ordering_beacon(
            window_id: u64,
        ) -> Result<sp_core::H256, sp_runtime::DispatchError> {
            let record =
                OrderingWindows::<T>::get(window_id).ok_or(Error::<T>::OrderingWindowNotFound)?;
            // `BlockHash` is keyed by the runtime's block number type; the window stores blocks as
            // `u64` (the lane's type), so convert rather than reinterpret.
            let beacon_block: BlockNumberFor<T> =
                Self::ordering_beacon_block(&record).saturated_into();
            let block_hash = <frame_system::Pallet<T>>::block_hash(beacon_block);
            let encoded = block_hash.encode();
            ensure!(encoded.len() == 32, Error::<T>::OrderingBeaconUnavailable);
            let beacon = sp_core::H256::from_slice(&encoded);
            ensure!(
                beacon != sp_core::H256::zero(),
                Error::<T>::OrderingBeaconUnavailable
            );
            Ok(beacon)
        }

        /// Map a lane refusal onto the pallet's named error.
        ///
        /// Mostly diagnostic: the pre-checks in each dispatchable refuse the
        /// caller-facing cases by name before the lane is consulted. A refusal
        /// here that the pre-checks already allow means the replay disagrees with
        /// the state the chain recorded, which is a corruption signal rather than
        /// a user error.
        fn map_fair_order_error(error: FairOrderError) -> sp_runtime::DispatchError {
            match error {
                FairOrderError::InvertedWindow { .. } => Error::<T>::OrderingWindowInverted.into(),
                FairOrderError::CommitBeforeWindow { .. }
                | FairOrderError::CommitAfterWindowClosed { .. }
                | FairOrderError::RevealBeforeWindow { .. }
                | FairOrderError::RevealAfterWindowClosed { .. } => {
                    Error::<T>::OrderingWindowNotOpen.into()
                }
                FairOrderError::BondBelowMinimum { .. } => {
                    Error::<T>::OrderingBondBelowMinimum.into()
                }
                FairOrderError::DuplicateCommit { .. } => {
                    Error::<T>::OrderingAlreadyCommitted.into()
                }
                FairOrderError::DuplicateCommitHash { .. } => {
                    Error::<T>::OrderingCommitAlreadyUsed.into()
                }
                FairOrderError::WindowFull { .. } => Error::<T>::OrderingWindowFull.into(),
                FairOrderError::RevealWithoutCommit { .. } => {
                    Error::<T>::OrderingUnknownCommitment.into()
                }
                FairOrderError::RevealSenderMismatch { .. } => {
                    Error::<T>::OrderingNotYourCommitment.into()
                }
                FairOrderError::RevealHashMismatch { .. } => {
                    Error::<T>::OrderingRevealMismatch.into()
                }
                FairOrderError::DuplicateReveal { .. } => {
                    Error::<T>::OrderingAlreadyRevealed.into()
                }
                FairOrderError::WindowStillOpen { .. } => {
                    Error::<T>::OrderingWindowStillOpen.into()
                }
                FairOrderError::WindowSettled => Error::<T>::OrderingWindowSettled.into(),
                FairOrderError::BeaconWhileWindowOpen { .. } => {
                    Error::<T>::OrderingBeaconWhileOpen.into()
                }
                FairOrderError::DuplicateBeacon => Error::<T>::OrderingBeaconAlreadySet.into(),
                FairOrderError::PlaintextTooLarge { .. } => {
                    Error::<T>::OrderingPlaintextTooLarge.into()
                }
            }
        }
    }
}
