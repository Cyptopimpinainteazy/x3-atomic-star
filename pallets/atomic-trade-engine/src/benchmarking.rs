//! Benchmarking setup for pallet-atomic-trade-engine
//!
//! Run benchmarks with:
//! ```bash
//! cargo build --release --features runtime-benchmarks
//! ./target/release/x3-chain-node benchmark pallet \
//!     --chain dev \
//!     --pallet pallet_atomic_trade_engine \
//!     --extrinsic "*" \
//!     --steps 50 \
//!     --repeat 20 \
//!     --output pallets/atomic-trade-engine/src/weights.rs
//! ```

use super::*;
use frame_benchmarking::v2::*;
use frame_support::traits::EnsureOrigin;
use frame_system::RawOrigin;
use sp_core::H256;

#[benchmarks]
mod benchmarks {
    use super::*;

    /// Benchmark: Create a trade batch with `l` legs
    #[benchmark]
    fn create_trade_batch(l: Linear<1, 16>) -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();

        // Create trade legs
        let legs: Vec<TradeLegInput> = (0..l)
            .map(|i| TradeLegInput {
                amm_protocol: AmmProtocol::UniswapV2,
                vm_type: VmType::Evm,
                asset_in: H256::from_low_u64_be(i as u64),
                asset_out: H256::from_low_u64_be((i + 1) as u64),
                amount_in: 1_000_000_000_000_000_000u128,
                min_amount_out: 900_000_000_000_000_000u128,
                route_data: BoundedVec::try_from(vec![0u8; 20]).expect("bounded route data"),
            })
            .collect();

        let current_block = frame_system::Pallet::<T>::block_number();
        let deadline = current_block + 100u32.into();
        let nonce = TradeNonces::<T>::get(&caller);

        #[extrinsic_call]
        create_trade_batch(
            RawOrigin::Signed(caller.clone()),
            legs,
            100,
            deadline,
            nonce,
        );

        // Verify batch was created
        let pending = PendingBatches::<T>::get(&caller);
        assert!(!pending.is_empty());

        Ok(())
    }

    /// Benchmark: Execute a trade batch
    #[benchmark]
    fn execute_trade_batch() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();

        // Create a single-leg batch
        let legs = vec![TradeLegInput {
            amm_protocol: AmmProtocol::UniswapV2,
            vm_type: VmType::Evm,
            asset_in: H256::from_low_u64_be(1),
            asset_out: H256::from_low_u64_be(2),
            amount_in: 1_000_000_000_000_000_000u128,
            min_amount_out: 1u128, // Very low min to ensure success
            route_data: BoundedVec::try_from(vec![0u8; 20]).expect("bounded route data"),
        }];

        let current_block = frame_system::Pallet::<T>::block_number();
        let deadline = current_block + 100u32.into();
        let nonce = TradeNonces::<T>::get(&caller);

        // Create the batch first
        Pallet::<T>::create_trade_batch(
            RawOrigin::Signed(caller.clone()).into(),
            legs,
            100,
            deadline,
            nonce,
        )?;

        // Get the batch ID
        let pending = PendingBatches::<T>::get(&caller);
        let batch_id = pending[0];

        #[extrinsic_call]
        execute_trade_batch(RawOrigin::Signed(caller.clone()), batch_id);

        // Verify batch was executed
        let batch = TradeBatches::<T>::get(batch_id).unwrap();
        assert!(batch.status == BatchStatus::Completed || batch.status == BatchStatus::Failed);

        Ok(())
    }

    /// Benchmark: Cancel a pending trade batch
    #[benchmark]
    fn cancel_trade_batch() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();

        // Create a batch first
        let legs = vec![TradeLegInput {
            amm_protocol: AmmProtocol::UniswapV2,
            vm_type: VmType::Evm,
            asset_in: H256::from_low_u64_be(1),
            asset_out: H256::from_low_u64_be(2),
            amount_in: 1_000_000_000_000_000_000u128,
            min_amount_out: 900_000_000_000_000_000u128,
            route_data: BoundedVec::try_from(vec![0u8; 20]).expect("bounded route data"),
        }];

        let current_block = frame_system::Pallet::<T>::block_number();
        let deadline = current_block + 100u32.into();
        let nonce = TradeNonces::<T>::get(&caller);

        Pallet::<T>::create_trade_batch(
            RawOrigin::Signed(caller.clone()).into(),
            legs,
            100,
            deadline,
            nonce,
        )?;

        let pending = PendingBatches::<T>::get(&caller);
        let batch_id = pending[0];

        #[extrinsic_call]
        cancel_trade_batch(RawOrigin::Signed(caller.clone()), batch_id);

        // Verify batch was removed
        assert!(TradeBatches::<T>::get(batch_id).is_none());

        Ok(())
    }

    /// Benchmark: Register an AMM adapter
    #[benchmark]
    fn register_amm_adapter() -> Result<(), BenchmarkError> {
        let config = AmmAdapterConfig {
            vm_type: VmType::Evm,
            address: BoundedVec::try_from(vec![0xab; 20]).expect("bounded address"),
            fee_bps: 30,
            enabled: true,
        };

        #[extrinsic_call]
        register_amm_adapter(RawOrigin::Root, AmmProtocol::UniswapV2, config);

        // Verify adapter was registered
        assert!(AmmAdapters::<T>::get(AmmProtocol::UniswapV2).is_some());

        Ok(())
    }

    /// Benchmark: Remove an AMM adapter
    #[benchmark]
    fn remove_amm_adapter() -> Result<(), BenchmarkError> {
        // First register an adapter
        let config = AmmAdapterConfig {
            vm_type: VmType::Svm,
            address: BoundedVec::try_from(vec![0xcd; 32]).expect("bounded address"),
            fee_bps: 25,
            enabled: true,
        };

        Pallet::<T>::register_amm_adapter(RawOrigin::Root.into(), AmmProtocol::Raydium, config)?;

        #[extrinsic_call]
        remove_amm_adapter(RawOrigin::Root, AmmProtocol::Raydium);

        // Verify adapter was removed
        assert!(AmmAdapters::<T>::get(AmmProtocol::Raydium).is_none());

        Ok(())
    }

    /// Benchmark: Create a manual checkpoint
    #[benchmark]
    fn create_manual_checkpoint() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();

        // Create and start executing a batch
        let legs = vec![TradeLegInput {
            amm_protocol: AmmProtocol::UniswapV2,
            vm_type: VmType::Evm,
            asset_in: H256::from_low_u64_be(1),
            asset_out: H256::from_low_u64_be(2),
            amount_in: 1_000_000_000_000_000_000u128,
            min_amount_out: 900_000_000_000_000_000u128,
            route_data: BoundedVec::try_from(vec![0u8; 20]).expect("bounded route data"),
        }];

        let current_block = frame_system::Pallet::<T>::block_number();
        let deadline = current_block + 100u32.into();
        let nonce = TradeNonces::<T>::get(&caller);

        Pallet::<T>::create_trade_batch(
            RawOrigin::Signed(caller.clone()).into(),
            legs,
            100,
            deadline,
            nonce,
        )?;

        let pending = PendingBatches::<T>::get(&caller);
        let batch_id = pending[0];

        // Manually set batch to executing state for checkpoint test
        TradeBatches::<T>::mutate(batch_id, |maybe_batch| {
            if let Some(batch) = maybe_batch {
                batch.status = BatchStatus::Executing;
            }
        });

        #[extrinsic_call]
        create_manual_checkpoint(RawOrigin::Signed(caller), batch_id);

        // Verify checkpoint was created
        let checkpoints = Checkpoints::<T>::get(batch_id);
        assert!(!checkpoints.is_empty());

        Ok(())
    }

    /// Benchmark: Register (or upsert) an on-chain liquidity pool.
    ///
    /// The four `AmmRegistrarOrigin` calls (`register_liquidity_pool`,
    /// `update_liquidity_pool`, `sync_pool_price`, `submit_price_observation`) charged
    /// hand-written literal weights until these benchmarks existed: the pallet ships a
    /// generated `weights.rs`, and none of the four was in it.
    #[benchmark]
    fn register_liquidity_pool() -> Result<(), BenchmarkError> {
        let origin = T::AmmRegistrarOrigin::try_successful_origin()
            .map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        register_liquidity_pool(
            origin,
            AmmProtocol::UniswapV2,
            VmType::Evm,
            H256::from_low_u64_be(1),
            H256::from_low_u64_be(2),
            1_000_000_000_000_000_000u128,
            2_000_000_000_000_000_000u128,
            30u32,
            vec![0xab; 20],
        );

        assert_eq!(LiquidityPools::<T>::iter().count(), 1);
        Ok(())
    }

    /// Benchmark: Update the reserves of an already registered pool.
    #[benchmark]
    fn update_liquidity_pool() -> Result<(), BenchmarkError> {
        let origin = T::AmmRegistrarOrigin::try_successful_origin()
            .map_err(|_| BenchmarkError::Weightless)?;
        let token_a = H256::from_low_u64_be(1);
        let token_b = H256::from_low_u64_be(2);

        Pallet::<T>::register_liquidity_pool(
            origin.clone(),
            AmmProtocol::UniswapV2,
            VmType::Evm,
            token_a,
            token_b,
            1_000_000_000_000_000_000u128,
            2_000_000_000_000_000_000u128,
            30u32,
            vec![0xab; 20],
        )?;
        let pool_id = LiquidityPools::<T>::iter_keys()
            .next()
            .ok_or(BenchmarkError::Weightless)?;

        #[extrinsic_call]
        update_liquidity_pool(
            origin,
            pool_id,
            3_000_000_000_000_000_000u128,
            4_000_000_000_000_000_000u128,
        );

        let pool = LiquidityPools::<T>::get(pool_id).ok_or(BenchmarkError::Weightless)?;
        assert_eq!(pool.reserve_a, 3_000_000_000_000_000_000u128);
        assert_eq!(pool.reserve_b, 4_000_000_000_000_000_000u128);
        Ok(())
    }

    /// Benchmark: Sync one pool's spot price into the TWAP oracle.
    #[benchmark]
    fn sync_pool_price() -> Result<(), BenchmarkError> {
        let origin = T::AmmRegistrarOrigin::try_successful_origin()
            .map_err(|_| BenchmarkError::Weightless)?;
        let token_a = H256::from_low_u64_be(1);
        let token_b = H256::from_low_u64_be(2);

        Pallet::<T>::register_liquidity_pool(
            origin.clone(),
            AmmProtocol::UniswapV2,
            VmType::Evm,
            token_a,
            token_b,
            1_000_000_000_000_000_000u128,
            2_000_000_000_000_000_000u128,
            30u32,
            vec![0xab; 20],
        )?;
        let pool_id = LiquidityPools::<T>::iter_keys()
            .next()
            .ok_or(BenchmarkError::Weightless)?;

        #[extrinsic_call]
        sync_pool_price(origin, pool_id);

        // A pool sync writes both directions of the pair.
        assert!(PriceObservations::<T>::contains_key((token_a, token_b)));
        assert!(PriceObservations::<T>::contains_key((token_b, token_a)));
        Ok(())
    }

    /// Benchmark: Submit a price observation for the TWAP oracle.
    #[benchmark]
    fn submit_price_observation() -> Result<(), BenchmarkError> {
        let origin = T::AmmRegistrarOrigin::try_successful_origin()
            .map_err(|_| BenchmarkError::Weightless)?;
        let token_a = H256::from_low_u64_be(1);
        let token_b = H256::from_low_u64_be(2);

        #[extrinsic_call]
        submit_price_observation(
            origin,
            token_a,
            token_b,
            1_500_000_000_000_000_000u128,
            AmmProtocol::UniswapV2,
        );

        assert!(PriceObservations::<T>::contains_key((token_a, token_b)));
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
