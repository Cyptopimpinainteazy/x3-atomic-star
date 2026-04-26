//! Benchmarks for x3-settlement-engine pallet
//!
//! These benchmarks measure the cost of critical settlement operations:
//! - Intent creation and lifecycle management
//! - Escrow locking and release
//! - BTC SPV verification
//! - Settlement finalization
//!
//! Weights generated from these benchmarks are used in extrinsic dispatch to ensure
//! blocks don't exceed weight limits and to calculate transaction fees accurately.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{benchmarks, whitelisted_caller, BenchmarkError};
use frame_system::RawOrigin;
use sp_core::H256;
use sp_std::vec;

const SEED: u32 = 0;

fn setup_intent<T: Config>() -> (T::AccountId, T::AccountId, H256, AssetSpec, AssetSpec) {
    let maker: T::AccountId = frame_benchmarking::account("maker", 0, SEED);
    let taker: T::AccountId = frame_benchmarking::account("taker", 1, SEED);
    let secret_hash: H256 = H256::from_low_u64_be(1);
    let asset_a = AssetSpec {
        chain: ExternalChainId::Ethereum,
        token: TokenId::Native,
        amount: 1_000_000u128,
    };
    let asset_b = AssetSpec {
        chain: ExternalChainId::Bitcoin,
        token: TokenId::Native,
        amount: 1_000_000u128,
    };
    (maker, taker, secret_hash, asset_a, asset_b)
}

benchmarks! {
    create_intent {
        let (maker, taker, secret_hash, asset_a, asset_b) = setup_intent::<T>();
        let origin = RawOrigin::Signed(maker.clone());
    }: _(origin, taker, asset_a, asset_b, secret_hash, Some(86400u64))
    verify {
        let nonce = TotalIntents::<T>::get();
        assert!(nonce > 0);
    }

    lock_escrow {
        let (maker, taker, secret_hash, asset_a, asset_b) = setup_intent::<T>();

        // Create intent first
        let create_origin = RawOrigin::Signed(maker.clone()).into();
        Pallet::<T>::create_intent(
            create_origin,
            taker.clone(),
            asset_a.clone(),
            asset_b.clone(),
            secret_hash,
            Some(86400u64),
        ).ok();

        let intent_id = Pallet::<T>::generate_intent_id(&maker, &taker, 0);
        let escrow_data = vec![1u8; 64];

        let origin = RawOrigin::Signed(maker.clone());
    }: _(
        origin,
        intent_id,
        0u32,
        ExternalChainId::Ethereum,
        1_000_000u128,
        escrow_data
    )
    verify {
        assert!(EscrowStates::<T>::contains_key(intent_id, 0u32));
    }

    claim_settlement {
        let (maker, taker, secret_hash, asset_a, asset_b) = setup_intent::<T>();

        // Create intent
        let create_origin = RawOrigin::Signed(maker.clone()).into();
        Pallet::<T>::create_intent(
            create_origin,
            taker.clone(),
            asset_a.clone(),
            asset_b.clone(),
            secret_hash,
            Some(86400u64),
        ).ok();

        let intent_id = Pallet::<T>::generate_intent_id(&maker, &taker, 0);

        // Lock escrow
        let escrow_origin = RawOrigin::Signed(maker.clone()).into();
        Pallet::<T>::lock_escrow(
            escrow_origin,
            intent_id,
            0u32,
            ExternalChainId::Ethereum,
            1_000_000u128,
            vec![1u8; 64],
        ).ok();

        let secret = H256::from_low_u64_be(1);
        let origin = RawOrigin::Signed(maker.clone());
    }: _(origin, intent_id, secret)
    verify {
        assert!(ClaimedLegs::<T>::get(intent_id, 0u32));
    }

    finalize_intent {
        let (maker, taker, secret_hash, asset_a, asset_b) = setup_intent::<T>();

        // Create intent
        let create_origin = RawOrigin::Signed(maker.clone()).into();
        Pallet::<T>::create_intent(
            create_origin,
            taker.clone(),
            asset_a.clone(),
            asset_b.clone(),
            secret_hash,
            Some(86400u64),
        ).ok();

        let intent_id = Pallet::<T>::generate_intent_id(&maker, &taker, 0);

        // Lock both escrows
        let escrow_origin_1 = RawOrigin::Signed(maker.clone()).into();
        Pallet::<T>::lock_escrow(
            escrow_origin_1,
            intent_id,
            0u32,
            ExternalChainId::Ethereum,
            1_000_000u128,
            vec![1u8; 64],
        ).ok();

        let escrow_origin_2 = RawOrigin::Signed(taker.clone()).into();
        Pallet::<T>::lock_escrow(
            escrow_origin_2,
            intent_id,
            1u32,
            ExternalChainId::Bitcoin,
            5_000_000u128,
            vec![2u8; 64],
        ).ok();

        let secret = H256::from_low_u64_be(1);
        let origin = RawOrigin::Signed(maker.clone());
    }: _(origin, intent_id, secret)
    verify {
        let state = IntentStates::<T>::get(intent_id);
        assert!(matches!(state, IntentState::Finalized));
    }

    refund_intent {
        let (maker, taker, secret_hash, asset_a, asset_b) = setup_intent::<T>();

        // Create intent with very short timeout
        let create_origin = RawOrigin::Signed(maker.clone()).into();
        Pallet::<T>::create_intent(
            create_origin,
            taker.clone(),
            asset_a.clone(),
            asset_b.clone(),
            secret_hash,
            Some(1u64), // 1 second timeout
        ).ok();

        let intent_id = Pallet::<T>::generate_intent_id(&maker, &taker, 0);

        // Lock escrow
        let escrow_origin = RawOrigin::Signed(maker.clone()).into();
        Pallet::<T>::lock_escrow(
            escrow_origin,
            intent_id,
            0u32,
            ExternalChainId::Ethereum,
            1_000_000u128,
            vec![1u8; 64],
        ).ok();

        let origin = RawOrigin::Signed(maker.clone());
    }: _(origin, intent_id)
    verify {
        let state = IntentStates::<T>::get(intent_id);
        assert!(matches!(state, IntentState::Refunded | IntentState::RefundInitiated));
    }

    verify_btc_proof {
        let (maker, taker, secret_hash, asset_a, asset_b) = setup_intent::<T>();

        // Create intent
        let create_origin = RawOrigin::Signed(maker.clone()).into();
        Pallet::<T>::create_intent(
            create_origin,
            taker.clone(),
            asset_a.clone(),
            asset_b.clone(),
            secret_hash,
            Some(86400u64),
        ).ok();

        let intent_id = Pallet::<T>::generate_intent_id(&maker, &taker, 0);
        let btc_txid = H256::from_low_u64_be(2);
        let merkle_proof = vec![1u8; 256]; // 256-byte merkle path

        let origin = RawOrigin::Signed(maker.clone());
    }: _(origin, intent_id, btc_txid, 1u32, merkle_proof)
    verify {
        // Just verify the extrinsic succeeds; actual BTC proof validation
        // depends on external chain state
    }

    update_btc_block_header {
        let header = BtcBlockHeader {
            version: 1,
            prev_block_hash: H256::from_low_u64_be(0),
            merkle_root: H256::from_low_u64_be(1),
            timestamp: 1234567890u32,
            bits: 0x207fffff,
            nonce: 0,
            height: 0u64,
        };

        let origin = RawOrigin::Root.into();
    }: _(origin, header)
    verify {
        assert!(BtcBestHeight::<T>::get() > 0);
    }

    submit_external_proof {
        let (maker, taker, secret_hash, asset_a, asset_b) = setup_intent::<T>();

        // Create intent
        let create_origin = RawOrigin::Signed(maker.clone()).into();
        Pallet::<T>::create_intent(
            create_origin,
            taker.clone(),
            asset_a.clone(),
            asset_b.clone(),
            secret_hash,
            Some(86400u64),
        ).ok();

        let intent_id = Pallet::<T>::generate_intent_id(&maker, &taker, 0);
        let proof = vec![1u8; 512];

        let origin = RawOrigin::Signed(maker.clone());
    }: _(origin, intent_id, 0u32, proof)
    verify {
        // Verify submission succeeds
    }

    deposit_bond {
        let depositor: T::AccountId = frame_benchmarking::account("depositor", 0, SEED);
        let amount = T::Currency::minimum_balance() * 100u32.into();

        let origin = RawOrigin::Signed(depositor.clone());
    }: _(origin, vec![1u8; 32], amount, 0u8)
    verify {
        let bond_count = BondCounter::<T>::get();
        assert!(bond_count > 0);
    }

    finalize_bond_withdraw {
        let depositor: T::AccountId = frame_benchmarking::account("depositor", 0, SEED);
        let amount = T::Currency::minimum_balance() * 100u32.into();

        // Create bond first
        let create_origin = RawOrigin::Signed(depositor.clone()).into();
        Pallet::<T>::deposit_bond(
            create_origin,
            vec![1u8; 32],
            amount,
            0u8,
        ).ok();

        let bond_id = H256::from_low_u64_be(0);

        let origin = RawOrigin::Signed(depositor.clone());
    }: _(origin, bond_id)
    verify {
        // Verify claim succeeds
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
