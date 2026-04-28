//! # X3 Account Registry Pallet
//!
//! Universal account registry for the X3 chain.
//!
//! ## Responsibilities
//!
//! * **Account registration** — any account holder can register an explicit
//!   on-chain identity anchored to their `AccountId`.
//! * **Nonce tracking** — canonical per-account operation counter used by
//!   cross-VM dispatchers to prevent replay across EVM / SVM / X3-VM.
//! * **Account metadata** — optional `AccountKind` tag so pallets can
//!   distinguish EOA, contract, system, and validator accounts without
//!   coupling to the EVM or SVM runtimes.
//! * **Deregistration** — accounts can be deregistered (metadata cleared)
//!   but the nonce is preserved to prevent replay.
//!
//! ## Design Constraints (v0.4)
//!
//! * Does NOT store balances — that remains in `pallet-balances`.
//! * Does NOT replace `frame-system` account tracking.
//! * Nonce in this pallet is independent of the `frame-system` nonce.
//!   It is the cross-VM replay counter only.
//! * Storage is bounded: `AccountKind` and `DisplayName` are small fixed-size
//!   or length-bounded types that implement `MaxEncodedLen`.
//!
//! ## Storage
//!
//! | Storage item | Key | Value |
//! |---|---|---|
//! | `Accounts` | `T::AccountId` | `AccountInfo<T>` |
//! | `CrossVmNonces` | `T::AccountId` | `u64` |
//! | `AccountCount` | — | `u64` |

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{dispatch::DispatchResult, pallet_prelude::*, traits::Get};
    use frame_system::pallet_prelude::*;
    use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
    use scale_info::TypeInfo;
    #[cfg(feature = "std")]
    use serde::{Deserialize, Serialize};
    use sp_runtime::traits::Zero;
    use sp_std::marker::PhantomData;

    // ─────────────────────────────────────────────────────────────────────────
    // Config
    // ─────────────────────────────────────────────────────────────────────────

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Maximum byte length of an account display name.
        #[pallet::constant]
        type MaxNameLength: Get<u32>;
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Types
    // ─────────────────────────────────────────────────────────────────────────

    /// Classification of an account.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen)]
    #[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
    pub enum AccountKind {
        /// Externally-owned account (default).
        Eoa,
        /// EVM contract address.
        EvmContract,
        /// SVM program address.
        SvmProgram,
        /// X3-VM application zone.
        X3AppZone,
        /// Validator node operator.
        Validator,
        /// System-reserved account (genesis, treasury, …).
        System,
    }

    /// Per-account information stored in the registry.
    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct AccountInfo<T: Config> {
        /// Account classification.
        pub kind: AccountKind,
        /// Optional human-readable name.
        pub display_name: BoundedVec<u8, T::MaxNameLength>,
        /// Block number at registration time.
        pub registered_at: BlockNumberFor<T>,
        /// Whether this account is currently active.
        pub active: bool,
        /// Phantom marker.
        pub _phantom: PhantomData<T>,
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Storage
    // ─────────────────────────────────────────────────────────────────────────

    /// Registry of explicitly registered accounts.
    #[pallet::storage]
    #[pallet::getter(fn accounts)]
    pub type Accounts<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, AccountInfo<T>, OptionQuery>;

    /// Cross-VM replay-protection nonce per account.
    /// Monotonically incremented on each cross-VM dispatch.
    /// Never reset even after deregistration.
    #[pallet::storage]
    #[pallet::getter(fn cross_vm_nonce)]
    pub type CrossVmNonces<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u64, ValueQuery>;

    /// Total number of currently-active registered accounts.
    #[pallet::storage]
    #[pallet::getter(fn account_count)]
    pub type AccountCount<T: Config> = StorageValue<_, u64, ValueQuery>;

    // ─────────────────────────────────────────────────────────────────────────
    // Events
    // ─────────────────────────────────────────────────────────────────────────

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new account was registered. [who, kind]
        AccountRegistered {
            who: T::AccountId,
            kind: AccountKind,
        },
        /// An account was deregistered (metadata cleared, nonce kept). [who]
        AccountDeregistered { who: T::AccountId },
        /// Cross-VM nonce was incremented. [who, new_nonce]
        NonceIncremented { who: T::AccountId, new_nonce: u64 },
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Errors
    // ─────────────────────────────────────────────────────────────────────────

    #[pallet::error]
    pub enum Error<T> {
        /// Account is already registered.
        AlreadyRegistered,
        /// Account is not registered.
        NotRegistered,
        /// Supplied display name exceeds `MaxNameLength`.
        NameTooLong,
        /// Nonce would overflow u64 (should never happen in practice).
        NonceOverflow,
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Pallet struct
    // ─────────────────────────────────────────────────────────────────────────

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ─────────────────────────────────────────────────────────────────────────
    // Calls
    // ─────────────────────────────────────────────────────────────────────────

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register the caller's account with an optional display name.
        ///
        /// Fails if the account is already registered.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10_000, 0).saturating_add(T::DbWeight::get().reads_writes(1, 2)))]
        pub fn register(
            origin: OriginFor<T>,
            kind: AccountKind,
            display_name: Option<BoundedVec<u8, T::MaxNameLength>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                !Accounts::<T>::contains_key(&who),
                Error::<T>::AlreadyRegistered
            );

            let name = display_name.unwrap_or_default();
            let info = AccountInfo::<T> {
                kind,
                display_name: name,
                registered_at: <frame_system::Pallet<T>>::block_number(),
                active: true,
                _phantom: PhantomData,
            };
            Accounts::<T>::insert(&who, info);
            AccountCount::<T>::mutate(|c| *c = c.saturating_add(1));

            Self::deposit_event(Event::AccountRegistered { who, kind });
            Ok(())
        }

        /// Deregister the caller's account.
        ///
        /// The cross-VM nonce is preserved to prevent replay attacks.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(10_000, 0).saturating_add(T::DbWeight::get().reads_writes(1, 2)))]
        pub fn deregister(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(Accounts::<T>::contains_key(&who), Error::<T>::NotRegistered);
            Accounts::<T>::remove(&who);
            AccountCount::<T>::mutate(|c| {
                if !c.is_zero() {
                    *c -= 1;
                }
            });

            Self::deposit_event(Event::AccountDeregistered { who });
            Ok(())
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Internal helpers
    // ─────────────────────────────────────────────────────────────────────────

    impl<T: Config> Pallet<T> {
        /// Atomically increment and return the cross-VM nonce for `who`.
        ///
        /// Called by cross-VM dispatcher pallets — not exposed as a public
        /// extrinsic to avoid front-running.
        pub fn increment_nonce(who: &T::AccountId) -> Result<u64, Error<T>> {
            CrossVmNonces::<T>::try_mutate(who, |n| {
                let next = n.checked_add(1).ok_or(Error::<T>::NonceOverflow)?;
                *n = next;
                Ok(next)
            })
        }

        /// Return the current cross-VM nonce without incrementing.
        pub fn peek_nonce(who: &T::AccountId) -> u64 {
            CrossVmNonces::<T>::get(who)
        }

        /// Return whether the account is registered and active.
        pub fn is_active(who: &T::AccountId) -> bool {
            Accounts::<T>::get(who).map_or(false, |info| info.active)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pallet::{AccountKind, Error, Event};
    use frame_support::{
        assert_noop, assert_ok, construct_runtime, derive_impl, parameter_types, traits::ConstU32,
    };
    use sp_core::H256;
    use sp_io::TestExternalities;
    use sp_runtime::{
        traits::{BlakeTwo256, IdentityLookup},
        BuildStorage,
    };

    type Block = frame_system::mocking::MockBlock<TestRuntime>;

    construct_runtime!(
        pub enum TestRuntime {
            System: frame_system,
            AccountRegistry: crate::pallet,
        }
    );

    parameter_types! {
        pub const MaxNameLength: u32 = 64;
    }

    #[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
    impl frame_system::Config for TestRuntime {
        type BaseCallFilter = frame_support::traits::Everything;
        type BlockWeights = ();
        type BlockLength = ();
        type DbWeight = ();
        type RuntimeOrigin = RuntimeOrigin;
        type RuntimeCall = RuntimeCall;
        type Nonce = u64;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type AccountId = u64;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Block = Block;
        type RuntimeEvent = RuntimeEvent;
        type BlockHashCount = frame_support::traits::ConstU64<250>;
        type Version = ();
        type PalletInfo = PalletInfo;
        type AccountData = ();
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type SystemWeightInfo = ();
        type SS58Prefix = ();
        type OnSetCode = ();
        type MaxConsumers = ConstU32<16>;
    }

    impl crate::pallet::Config for TestRuntime {
        type RuntimeEvent = RuntimeEvent;
        type MaxNameLength = MaxNameLength;
    }

    fn new_test_ext() -> TestExternalities {
        frame_system::GenesisConfig::<TestRuntime>::default()
            .build_storage()
            .unwrap()
            .into()
    }

    #[test]
    fn register_and_retrieve() {
        new_test_ext().execute_with(|| {
            assert_ok!(AccountRegistry::register(
                RuntimeOrigin::signed(1),
                AccountKind::Eoa,
                None,
            ));
            assert!(AccountRegistry::is_active(&1));
            assert_eq!(AccountRegistry::account_count(), 1);
        });
    }

    #[test]
    fn duplicate_registration_rejected() {
        new_test_ext().execute_with(|| {
            assert_ok!(AccountRegistry::register(
                RuntimeOrigin::signed(1),
                AccountKind::Eoa,
                None,
            ));
            assert_noop!(
                AccountRegistry::register(RuntimeOrigin::signed(1), AccountKind::Eoa, None),
                Error::<TestRuntime>::AlreadyRegistered
            );
        });
    }

    #[test]
    fn deregister_removes_account() {
        new_test_ext().execute_with(|| {
            assert_ok!(AccountRegistry::register(
                RuntimeOrigin::signed(1),
                AccountKind::Eoa,
                None,
            ));
            assert_ok!(AccountRegistry::deregister(RuntimeOrigin::signed(1)));
            assert!(!AccountRegistry::is_active(&1));
            assert_eq!(AccountRegistry::account_count(), 0);
        });
    }

    #[test]
    fn deregister_nonexistent_rejected() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                AccountRegistry::deregister(RuntimeOrigin::signed(99)),
                Error::<TestRuntime>::NotRegistered
            );
        });
    }

    #[test]
    fn nonce_increments_monotonically() {
        new_test_ext().execute_with(|| {
            assert_eq!(AccountRegistry::peek_nonce(&1), 0);
            assert_eq!(AccountRegistry::increment_nonce(&1).unwrap(), 1);
            assert_eq!(AccountRegistry::increment_nonce(&1).unwrap(), 2);
            assert_eq!(AccountRegistry::peek_nonce(&1), 2);
        });
    }

    #[test]
    fn nonce_preserved_after_deregister() {
        new_test_ext().execute_with(|| {
            assert_ok!(AccountRegistry::register(
                RuntimeOrigin::signed(1),
                AccountKind::Eoa,
                None,
            ));
            AccountRegistry::increment_nonce(&1).unwrap();
            AccountRegistry::increment_nonce(&1).unwrap();
            assert_ok!(AccountRegistry::deregister(RuntimeOrigin::signed(1)));
            // Nonce must survive deregistration (replay protection).
            assert_eq!(AccountRegistry::peek_nonce(&1), 2);
        });
    }

    #[test]
    fn account_kind_stored_correctly() {
        new_test_ext().execute_with(|| {
            assert_ok!(AccountRegistry::register(
                RuntimeOrigin::signed(2),
                AccountKind::Validator,
                None,
            ));
            let info = AccountRegistry::accounts(&2).unwrap();
            assert_eq!(info.kind, AccountKind::Validator);
        });
    }

    #[test]
    fn event_emitted_on_register() {
        new_test_ext().execute_with(|| {
            frame_system::Pallet::<TestRuntime>::set_block_number(1);
            assert_ok!(AccountRegistry::register(
                RuntimeOrigin::signed(3),
                AccountKind::EvmContract,
                None,
            ));
            let events = frame_system::Pallet::<TestRuntime>::events();
            assert!(events.iter().any(|r| matches!(
                r.event,
                RuntimeEvent::AccountRegistry(Event::AccountRegistered {
                    who: 3,
                    kind: AccountKind::EvmContract,
                })
            )));
        });
    }
}
