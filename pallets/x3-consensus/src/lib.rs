#![deny(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]

//! # X3 Consensus Pallet
//!
//! Integrates Aura block production and Grandpa finality with validator management
//! and slashing for consensus violations.

pub use pallet::*;
// Re-export the log crate so the outer SessionManager impl can use it.
use log;
// Bring Vec into outer scope for SessionManager + OnOffenceHandler impls (no_std).
use sp_std::vec::Vec;
// Bring the legacy Currency trait into scope for the OnOffenceHandler impl.
use frame_support::traits::Currency as LegacyCurrency;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::weights::WeightInfo as _;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, Get},
        weights::Weight,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{Saturating, Zero};
    use sp_runtime::Perbill;
    use sp_std::vec::Vec;

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_aura::Config + pallet_grandpa::Config + pallet_session::Config
    {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Maximum number of active validators
        #[pallet::constant]
        type MaxValidators: Get<u32>;

        /// Weight information for extrinsics
        type WeightInfo: crate::weights::WeightInfo;

        /// Currency used for slashing validator balances.
        type Currency: frame_support::traits::Currency<Self::AccountId>;

        /// Fraction of the validator's total balance to slash per misbehavior report.
        #[pallet::constant]
        type SlashFraction: Get<Perbill>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Current active validator set
    #[pallet::storage]
    #[pallet::getter(fn validators)]
    pub type Validators<T: Config> =
        StorageValue<_, BoundedVec<T::AccountId, T::MaxValidators>, ValueQuery>;

    /// Next validator set (pending activation)
    #[pallet::storage]
    #[pallet::getter(fn next_validators)]
    pub type NextValidators<T: Config> =
        StorageValue<_, BoundedVec<T::AccountId, T::MaxValidators>, ValueQuery>;

    /// Block number when next validator set should be activated
    #[pallet::storage]
    pub type ValidatorSetActivationBlock<T: Config> =
        StorageValue<_, BlockNumberFor<T>, OptionQuery>;

    /// Consensus state tracking
    #[pallet::storage]
    #[pallet::getter(fn consensus_state)]
    pub type ConsensusState<T: Config> =
        StorageValue<_, ConsensusInfo<BlockNumberFor<T>>, ValueQuery>;

    /// Events
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// New validator set activated
        ValidatorSetChanged { validators: Vec<T::AccountId> },
        /// Consensus state updated
        ConsensusStateUpdated { block_number: BlockNumberFor<T> },
        /// Validator slashed for misbehavior
        ValidatorSlashed {
            validator: T::AccountId,
            reason: SlashReason,
        },
    }

    /// Errors
    #[pallet::error]
    pub enum Error<T> {
        /// Too many validators specified
        TooManyValidators,
        /// Invalid validator set
        InvalidValidatorSet,
        /// Consensus not initialized
        ConsensusNotInitialized,
    }

    /// Consensus information snapshot. Authorities are stored as encoded bytes to keep the
    /// snapshot type stable across runtime upgrades that may change Aura/Grandpa authority types.
    #[derive(
        Clone, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Debug, PartialEq, Eq,
    )]
    #[scale_info(skip_type_params(BlockNumber))]
    pub struct ConsensusInfo<BlockNumber: MaxEncodedLen> {
        /// Current block number
        pub block_number: BlockNumber,
        /// Number of active Aura authorities
        pub aura_authority_count: u32,
        /// Number of active Grandpa authorities
        pub grandpa_authority_count: u32,
        /// Last finalized grandpa set id
        pub last_finalized_set_id: u64,
    }

    /// Slash reasons
    #[derive(
        Clone, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Debug, PartialEq, Eq,
    )]
    pub enum SlashReason {
        /// Double signing detected
        DoubleSign,
        /// Equivocation in consensus
        Equivocation,
        /// Missing blocks
        MissingBlocks,
        /// Invalid finality proof
        InvalidFinality,
    }

    impl<BlockNumber> Default for ConsensusInfo<BlockNumber>
    where
        BlockNumber: Default + MaxEncodedLen,
    {
        fn default() -> Self {
            Self {
                block_number: Default::default(),
                aura_authority_count: 0,
                grandpa_authority_count: 0,
                last_finalized_set_id: 0,
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Set the next validator set (requires governance approval)
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::set_validators())]
        pub fn set_validators(
            origin: OriginFor<T>,
            validators: Vec<T::AccountId>,
            activation_delay: BlockNumberFor<T>,
        ) -> DispatchResult {
            // Only governance can change validators
            ensure_root(origin)?;

            let bounded_validators =
                BoundedVec::try_from(validators).map_err(|_| Error::<T>::TooManyValidators)?;

            let activation_block =
                frame_system::Pallet::<T>::block_number().saturating_add(activation_delay);

            NextValidators::<T>::put(bounded_validators.clone());
            ValidatorSetActivationBlock::<T>::put(activation_block);

            Self::deposit_event(Event::ValidatorSetChanged {
                validators: bounded_validators.into_inner(),
            });

            Ok(())
        }

        /// Report validator misbehavior
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::report_misbehavior())]
        pub fn report_misbehavior(
            origin: OriginFor<T>,
            validator: T::AccountId,
            reason: SlashReason,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            // Slash a fraction of the misbehaving validator's total balance.
            let slash_fraction = T::SlashFraction::get();
            let total = <<T as Config>::Currency as frame_support::traits::Currency<T::AccountId>>::total_balance(&validator);
            let slash_amount = slash_fraction * total;

            if !slash_amount.is_zero() {
                let (_, _) = <<T as Config>::Currency as frame_support::traits::Currency<T::AccountId>>::slash(&validator, slash_amount);
                log::warn!(
                    target: "x3-consensus",
                    "⚡ Slashed validator {:?} by {:?} ({:?} of total balance) for {:?}",
                    validator,
                    slash_amount,
                    slash_fraction,
                    reason,
                );
            }

            Self::deposit_event(Event::ValidatorSlashed { validator, reason });
            Ok(())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(n: BlockNumberFor<T>) -> Weight {
            // Check if we need to activate a new validator set
            if let Some(activation_block) = ValidatorSetActivationBlock::<T>::get() {
                if n >= activation_block {
                    Self::activate_validator_set();
                    ValidatorSetActivationBlock::<T>::kill();
                }
            }

            // Update consensus state
            Self::update_consensus_state(n);

            <T as Config>::WeightInfo::on_initialize()
        }
    }

    impl<T: Config> Pallet<T> {
        /// Activate the next validator set
        fn activate_validator_set() {
            if let Ok(next_validators) = NextValidators::<T>::try_get() {
                Validators::<T>::put(next_validators.clone());
                Self::deposit_event(Event::ValidatorSetChanged {
                    validators: next_validators.into_inner(),
                });
            }
        }

        /// Update the current consensus state
        fn update_consensus_state(current_block: BlockNumberFor<T>) {
            let aura_authority_count = pallet_aura::Authorities::<T>::get().len() as u32;
            let grandpa_authority_count =
                pallet_grandpa::Pallet::<T>::grandpa_authorities().len() as u32;

            let consensus_info = ConsensusInfo {
                block_number: current_block,
                aura_authority_count,
                grandpa_authority_count,
                last_finalized_set_id: pallet_grandpa::Pallet::<T>::current_set_id(),
            };

            ConsensusState::<T>::put(consensus_info);

            Self::deposit_event(Event::ConsensusStateUpdated {
                block_number: current_block,
            });
        }

        /// Get current validator set
        pub fn current_validators() -> Vec<T::AccountId> {
            Validators::<T>::get().into_inner()
        }

        /// Check if account is a validator
        pub fn is_validator(who: &T::AccountId) -> bool {
            Validators::<T>::get().contains(who)
        }
    }
}

/// Implement `pallet_session::SessionManager` so the session pallet calls into this pallet
/// at every session boundary to obtain the next validator set.
///
/// This must live **outside** the `#[frame_support::pallet]` macro block so it is not subject
/// to FRAME's attribute-macro rewriting.
impl<T: Config> pallet_session::SessionManager<T::AccountId> for Pallet<T> {
    /// Called by `pallet_session` before a new session begins.
    ///
    /// Returns `Some(validators)` when a queued validator set is ready to be activated
    /// (i.e., `ValidatorSetActivationBlock` has been set and the current block number is
    /// at or past that block).  Returns `None` to keep the existing set unchanged.
    fn new_session(new_index: u32) -> Option<Vec<T::AccountId>> {
        let current_block = frame_system::Pallet::<T>::block_number();
        if let Some(activation_block) = ValidatorSetActivationBlock::<T>::get() {
            if current_block >= activation_block {
                let next = NextValidators::<T>::get().into_inner();
                if !next.is_empty() {
                    log::info!(
                        target: "x3-consensus",
                        "📋 Session {} activating queued validator set ({} validators)",
                        new_index,
                        next.len()
                    );
                    return Some(next);
                }
            }
        }
        None
    }

    fn end_session(_end_index: u32) {
        // No cleanup needed; ValidatorSetActivationBlock is cleared in on_initialize
        // after the set is activated.
    }

    fn start_session(_start_index: u32) {
        // Nothing to do at session start; new validators were already returned in new_session.
    }
}

/// Implement `OnOffenceHandler` so the offences pallet can trigger slashing via this pallet.
/// This is wired as `type OnOffenceHandler = X3Consensus` in the runtime's
/// `pallet_offences::Config`.
///
/// The generic `FullId` captures whatever full-identification type is paired with the
/// `ValidatorId` (e.g. `()` in minimal runtimes) without requiring the historical-session
/// feature flag or `pallet_session::historical::Config` bounds.
impl<T: Config, FullId> sp_staking::offence::OnOffenceHandler<
    T::AccountId,
    (T::AccountId, FullId),
    frame_support::weights::Weight,
> for Pallet<T> {
    fn on_offence(
        offenders: &[sp_staking::offence::OffenceDetails<
            T::AccountId,
            (T::AccountId, FullId),
        >],
        slash_fraction: &[sp_runtime::Perbill],
        _session: sp_staking::SessionIndex,
    ) -> frame_support::weights::Weight {
        use frame_support::traits::Get as _;
        use sp_runtime::traits::Zero as _;

        let mut total_weight = frame_support::weights::Weight::zero();

        for (details, fraction) in offenders.iter().zip(slash_fraction.iter()) {
            let offender: &T::AccountId = &details.offender.0;
            let total = <<T as Config>::Currency as LegacyCurrency<T::AccountId>>::total_balance(offender);
            let slash_amount = *fraction * total;

            if !slash_amount.is_zero() {
                let (_, _) = <<T as Config>::Currency as LegacyCurrency<T::AccountId>>::slash(offender, slash_amount);
                log::warn!(
                    target: "x3-consensus",
                    "⚡ OnOffenceHandler: slashed {:?} by fraction {:?}",
                    offender,
                    fraction,
                );
            }
            total_weight =
                total_weight.saturating_add(T::DbWeight::get().reads_writes(2, 1));
        }

        total_weight
    }
}


pub mod weights {
    use frame_support::weights::Weight;

    /// Weight functions for pallet_x3_consensus
    pub trait WeightInfo {
        fn set_validators() -> Weight;
        fn report_misbehavior() -> Weight;
        fn on_initialize() -> Weight;
    }

    /// Default weight implementation
    pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

    impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
        fn set_validators() -> Weight {
            Weight::from_parts(10_000_000, 0)
        }

        fn report_misbehavior() -> Weight {
            Weight::from_parts(5_000_000, 0)
        }

        fn on_initialize() -> Weight {
            Weight::from_parts(1_000_000, 0)
        }
    }

    /// No-op weight implementation for use in mocks and tests.
    impl WeightInfo for () {
        fn set_validators() -> Weight {
            Weight::zero()
        }
        fn report_misbehavior() -> Weight {
            Weight::zero()
        }
        fn on_initialize() -> Weight {
            Weight::zero()
        }
    }
}
