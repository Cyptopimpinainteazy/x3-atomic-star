//! # X3 Account Registry Pallet
//!
//! Universal account registry for the X3 chain.
//!
//! This pallet tracks a canonical Atlas ID, optional account kind, and a
//! cross-VM nonce for replay prevention across EVM / SVM / X3-VM flows.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{dispatch::DispatchResult, pallet_prelude::*, traits::Get};
    use frame_system::pallet_prelude::*;
    use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
    use scale_info::TypeInfo;
    #[cfg(feature = "std")]
    use serde::{Deserialize, Serialize};
    use sp_std::vec::Vec;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type AtlasId: Parameter + Member + Default + Copy + MaxEncodedLen;

        #[pallet::constant]
        type MaxNameLength: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

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
        Eoa,
        EvmContract,
        SvmProgram,
        X3AppZone,
        Validator,
        System,
    }

    #[pallet::type_value]
    pub fn DefaultForAccountCount() -> u64 {
        0
    }

    #[pallet::storage]
    #[pallet::getter(fn account_registry)]
    pub type AccountRegistry<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::AtlasId, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn atlas_registry)]
    pub type AtlasRegistry<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AtlasId, T::AccountId, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn account_kind)]
    pub type AccountKinds<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, AccountKind, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn cross_vm_nonce)]
    pub type CrossVmNonces<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn account_count)]
    pub type AccountCount<T> = StorageValue<_, u64, ValueQuery, DefaultForAccountCount>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        AccountRegistered {
            account: T::AccountId,
            atlas_id: T::AtlasId,
        },
        AccountDeregistered {
            account: T::AccountId,
            atlas_id: T::AtlasId,
        },
        NonceAnchored {
            account: T::AccountId,
            nonce: u64,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        AlreadyRegistered,
        NotRegistered,
        AtlasIdInUse,
        NameTooLong,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(10_000)]
        pub fn register_account(
            origin: OriginFor<T>,
            atlas_id: T::AtlasId,
            kind: AccountKind,
            display_name: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                !AccountRegistry::<T>::contains_key(&who),
                Error::<T>::AlreadyRegistered
            );
            ensure!(
                !AtlasRegistry::<T>::contains_key(&atlas_id),
                Error::<T>::AtlasIdInUse
            );

            ensure!(
                display_name.len() <= T::MaxNameLength::get() as usize,
                Error::<T>::NameTooLong
            );

            AccountRegistry::<T>::insert(&who, atlas_id);
            AtlasRegistry::<T>::insert(&atlas_id, &who);
            AccountKinds::<T>::insert(&who, kind);
            AccountCount::<T>::mutate(|count| *count = count.saturating_add(1));

            Self::deposit_event(Event::AccountRegistered {
                account: who,
                atlas_id,
            });
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(10_000)]
        pub fn deregister_account(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let atlas_id = AccountRegistry::<T>::take(&who).ok_or(Error::<T>::NotRegistered)?;

            AtlasRegistry::<T>::remove(&atlas_id);
            AccountKinds::<T>::remove(&who);
            AccountCount::<T>::mutate(|count| *count = count.saturating_sub(1));

            Self::deposit_event(Event::AccountDeregistered {
                account: who,
                atlas_id,
            });
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(10_000)]
        pub fn anchor_nonce(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
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
        pub fn get_atlas_id(account: &T::AccountId) -> Option<T::AtlasId> {
            AccountRegistry::<T>::get(account)
        }

        pub fn get_account(atlas_id: T::AtlasId) -> Option<T::AccountId> {
            AtlasRegistry::<T>::get(atlas_id)
        }

        pub fn get_next_cross_vm_nonce(account: &T::AccountId) -> u64 {
            CrossVmNonces::<T>::get(account)
        }

        pub fn increment_cross_vm_nonce(account: &T::AccountId) {
            CrossVmNonces::<T>::mutate(account, |nonce| *nonce = nonce.saturating_add(1));
        }
    }
}
