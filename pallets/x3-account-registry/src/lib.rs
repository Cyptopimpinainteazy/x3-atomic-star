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
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Encode,
        Decode,
        DecodeWithMemTracking,
        TypeInfo,
        MaxEncodedLen,
    )]
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
    #[derive(
        Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
    )]
    #[scale_info(skip_type_params(T))]
    pub struct AccountInfo<T: Config> {
        /// Account classification.
        egistered {
                account: who,
                atlas_id,
            });

            Ok(())
        }

        /// Anchor the current nonce for cross-VM operations.
        #[pallet::call_index(1)]
        #[pallet::weight(10_000)]
        pub fn anchor_nonce(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Ensure account is registered
            ensure!(
                AccountRegistry::<T>::contains_key(&who),
                Error::<T>::NotRegistered
            );

            let nonce = CrossVmNonces::<T>::get(&who);

            Self::deposit_event(Event::NonceAnchored {
                account: who,
                nonce,
            });

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Get the Atlas ID for an account.
        pub fn get_atlas_id(account: &T::AccountId) -> Option<T::AtlasId> {
            AccountRegistry::<T>::get(account)
        }

        /// Get the account for an Atlas ID.
        pub fn get_account(atlas_id: T::AtlasId) -> Option<T::AccountId> {
            AtlasRegistry::<T>::get(atlas_id)
        }

        /// Get the next cross-VM nonce for an account.
        pub fn get_next_cross_vm_nonce(account: &T::AccountId) -> u64 {
            CrossVmNonces::<T>::get(account)
        }

        /// Increment the cross-VM nonce for an account.
        pub fn increment_cross_vm_nonce(account: &T::AccountId) {
            CrossVmNonces::<T>::mutate(account, |nonce| *nonce = nonce.saturating_add(1));
        }
    }
}