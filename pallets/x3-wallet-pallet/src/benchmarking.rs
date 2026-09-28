//! Benchmarking setup for `pallet-x3-wallet` (the crate the runtime aliases as
//! `pallet-x3-wallet-pallet`).
//!
//! All twelve dispatchables charged literals between 5,000 and 15,000
//! picoseconds, with no read or write count at all, behind a
//! `runtime-benchmarks` feature that pulled `frame-benchmarking` in and did
//! nothing with it.
//!
//! Setup goes through the pallet's own extrinsics wherever one exists: mint
//! authority is granted with `add_minter` (root), balances come from
//! `mint_tokens`, and the recovery benchmarks walk the real flow —
//! `register_recovery_guardians` → `initiate_recovery` → `approve_recovery` —
//! with a zero delay so `finalize_recovery` reaches its executable block
//! without the benchmark having to move the chain's block number.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use sp_std::vec;
use sp_std::vec::Vec;

#[benchmarks]
mod benchmarks {
    use super::*;

    const TOKEN_ID: [u8; 32] = [0x77; 32];
    /// The owner recorded by the recovery flow. Distinct from any account the
    /// encoder produces for the caller, which is what `initiate_recovery`
    /// checks (`new_owner == account.owner` is refused).
    const NEW_OWNER: [u8; 32] = [0x11; 32];
    const MINT_AMOUNT: u128 = 1_000_000;
    const TRANSFER_AMOUNT: u128 = 1_000;
    const GUARDIAN_THRESHOLD: u32 = 1;
    /// Zero, so `executable_block == created_block` and the finalize below does
    /// not need the block number moved.
    const RECOVERY_DELAY: u64 = 0;

    fn other<T: Config>(seed: &'static str) -> T::AccountId {
        frame_benchmarking::account(seed, 0, 0)
    }

    /// Grant `who` mint authority through the pallet's own root-gated extrinsic.
    fn authorise_minter<T: Config>(who: &T::AccountId) -> Result<(), BenchmarkError> {
        Pallet::<T>::add_minter(RawOrigin::Root.into(), who.clone())
            .map_err(|_| BenchmarkError::Weightless)
    }

    /// Give `who` a token balance it can spend.
    fn fund<T: Config>(who: &T::AccountId) -> Result<(), BenchmarkError> {
        authorise_minter::<T>(who)?;
        Pallet::<T>::mint_tokens(
            RawOrigin::Signed(who.clone()).into(),
            TOKEN_ID,
            who.clone(),
            MINT_AMOUNT,
        )
        .map_err(|_| BenchmarkError::Weightless)
    }

    /// Register a single-guardian recovery set for `owner`.
    fn setup_guardians<T: Config>(
        owner: &T::AccountId,
        guardian: &T::AccountId,
    ) -> Result<(), BenchmarkError> {
        let guardians: Vec<[u8; 32]> = vec![Pallet::<T>::account_bytes(guardian)];
        Pallet::<T>::register_recovery_guardians(
            RawOrigin::Signed(owner.clone()).into(),
            guardians,
            GUARDIAN_THRESHOLD,
            RECOVERY_DELAY,
        )
        .map_err(|_| BenchmarkError::Weightless)
    }

    /// Register the set and open a pending request for it.
    fn setup_request<T: Config>(
        owner: &T::AccountId,
        guardian: &T::AccountId,
    ) -> Result<(), BenchmarkError> {
        setup_guardians::<T>(owner, guardian)?;
        Pallet::<T>::initiate_recovery(
            RawOrigin::Signed(guardian.clone()).into(),
            owner.clone(),
            NEW_OWNER,
        )
        .map_err(|_| BenchmarkError::Weightless)
    }

    #[benchmark]
    fn register_hardware_wallet() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        register_hardware_wallet(
            RawOrigin::Signed(caller.clone()),
            1u8,
            vec![1u8, 2, 3],
            [7u8; 32],
        );

        assert_eq!(HardwareWallets::<T>::iter().count(), 1);
        Ok(())
    }

    #[benchmark]
    fn create_multisig_wallet() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        let signers: Vec<[u8; 32]> = vec![Pallet::<T>::account_bytes(&caller), [0x22; 32]];

        #[extrinsic_call]
        create_multisig_wallet(RawOrigin::Signed(caller.clone()), signers, 2u32, 0u64);

        assert_eq!(MultisigWallets::<T>::iter().count(), 1);
        Ok(())
    }

    #[benchmark]
    fn transfer_tokens() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        fund::<T>(&caller)?;
        let to: T::AccountId = other::<T>("recipient");

        #[extrinsic_call]
        transfer_tokens(
            RawOrigin::Signed(caller.clone()),
            TOKEN_ID,
            to.clone(),
            TRANSFER_AMOUNT,
        );

        assert_eq!(
            TokenBalances::<T>::get((&caller, TOKEN_ID)),
            MINT_AMOUNT - TRANSFER_AMOUNT
        );
        Ok(())
    }

    #[benchmark]
    fn register_biometric() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        register_biometric(RawOrigin::Signed(caller.clone()), 1u8, [1u8; 32], [2u8; 32]);

        assert!(BiometricProfiles::<T>::contains_key(&caller));
        Ok(())
    }

    #[benchmark]
    fn register_recovery_guardians() -> Result<(), BenchmarkError> {
        let owner: T::AccountId = whitelisted_caller();
        let guardian: T::AccountId = other::<T>("guardian");
        let guardians: Vec<[u8; 32]> = vec![Pallet::<T>::account_bytes(&guardian)];

        #[extrinsic_call]
        register_recovery_guardians(
            RawOrigin::Signed(owner.clone()),
            guardians,
            GUARDIAN_THRESHOLD,
            RECOVERY_DELAY,
        );

        assert!(RecoveryAccounts::<T>::contains_key(&owner));
        Ok(())
    }

    #[benchmark]
    fn initiate_recovery() -> Result<(), BenchmarkError> {
        let owner: T::AccountId = whitelisted_caller();
        let guardian: T::AccountId = other::<T>("guardian");
        setup_guardians::<T>(&owner, &guardian)?;

        #[extrinsic_call]
        initiate_recovery(
            RawOrigin::Signed(guardian.clone()),
            owner.clone(),
            NEW_OWNER,
        );

        assert!(RecoveryRequests::<T>::contains_key(&owner));
        Ok(())
    }

    #[benchmark]
    fn approve_recovery() -> Result<(), BenchmarkError> {
        let owner: T::AccountId = whitelisted_caller();
        let guardian: T::AccountId = other::<T>("guardian");
        setup_request::<T>(&owner, &guardian)?;

        #[extrinsic_call]
        approve_recovery(RawOrigin::Signed(guardian.clone()), owner.clone());

        assert_eq!(
            RecoveryRequests::<T>::get(&owner).map(|r| r.approvals.len()),
            Some(1)
        );
        Ok(())
    }

    #[benchmark]
    fn finalize_recovery() -> Result<(), BenchmarkError> {
        let owner: T::AccountId = whitelisted_caller();
        let guardian: T::AccountId = other::<T>("guardian");
        setup_request::<T>(&owner, &guardian)?;
        Pallet::<T>::approve_recovery(RawOrigin::Signed(guardian.clone()).into(), owner.clone())
            .map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        finalize_recovery(RawOrigin::Signed(guardian.clone()), owner.clone());

        assert_eq!(
            RecoveryAccounts::<T>::get(&owner).map(|a| a.owner),
            Some(NEW_OWNER)
        );
        Ok(())
    }

    #[benchmark]
    fn cancel_recovery() -> Result<(), BenchmarkError> {
        let owner: T::AccountId = whitelisted_caller();
        let guardian: T::AccountId = other::<T>("guardian");
        setup_request::<T>(&owner, &guardian)?;

        #[extrinsic_call]
        cancel_recovery(RawOrigin::Signed(owner.clone()), owner.clone());

        assert!(!RecoveryRequests::<T>::contains_key(&owner));
        Ok(())
    }

    #[benchmark]
    fn mint_tokens() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        authorise_minter::<T>(&caller)?;
        let to: T::AccountId = other::<T>("recipient");

        #[extrinsic_call]
        mint_tokens(
            RawOrigin::Signed(caller.clone()),
            TOKEN_ID,
            to.clone(),
            MINT_AMOUNT,
        );

        assert_eq!(TokenBalances::<T>::get((&to, TOKEN_ID)), MINT_AMOUNT);
        Ok(())
    }

    #[benchmark]
    fn add_minter() -> Result<(), BenchmarkError> {
        let who: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        add_minter(RawOrigin::Root, who.clone());

        assert!(Minters::<T>::contains_key(&who));
        Ok(())
    }

    #[benchmark]
    fn remove_minter() -> Result<(), BenchmarkError> {
        let who: T::AccountId = whitelisted_caller();
        authorise_minter::<T>(&who)?;

        #[extrinsic_call]
        remove_minter(RawOrigin::Root, who.clone());

        assert!(!Minters::<T>::contains_key(&who));
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
