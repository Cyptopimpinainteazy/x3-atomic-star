//! X3 Chain Atomic Cross-Chain Swap Router
//!
//! Production-ready atomic swap router supporting 103+ chains with:
//! - Route optimization across all EVM chains  
//! - <1% slippage protection with real-time pricing
//! - Atomic execution via Comit bundles
//! - MEV protection and sandwich attack prevention
//! - Gas optimization per chain
//! - Fallback routing for RPC failures

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod atomic_execution;
pub mod fee_calculator;
pub mod gas_optimization;
pub mod mev_protection;
pub mod optimization;
pub mod quote_engine;
pub mod routing;
pub mod slippage_control;

pub use atomic_execution::{AtomicSwapExecutor, ExecutionResult, ExecutionStatus, SwapBundle};
pub use fee_calculator::{FeeCalculator, FeeStructure, ProtocolFees};
pub use gas_optimization::{ChainGasParams, GasEstimate, GasOptimizer};
pub use mev_protection::{Hop, MEVProtector, ProtectionStrategy, Route, SandwichProtection};
pub use optimization::{OptimizationParams, RouteOptimizer, RouteScore};
pub use quote_engine::{PriceOracle, PriceSource, QuoteEngine, QuoteResult};
pub use routing::{HopInfo, RouteConstraints, RouteFinder, SwapRoute};
use serde::{Deserialize, Serialize};
pub use slippage_control::{
    ProtectionLevel, SlippageConfig, SlippageController, SlippageProtectedParams,
};
use sp_core::{H160, H256, U256};

#[cfg(test)]
mod tests;

/// Main atomic swap router for cross-chain transactions
pub struct AtomicSwapRouter {
    route_optimizer: RouteOptimizer,
    quote_engine: QuoteEngine,
    atomic_executor: AtomicSwapExecutor,
    mev_protector: MEVProtector,
    gas_optimizer: GasOptimizer,
    slippage_controller: SlippageController,
    fee_calculator: FeeCalculator,
}

impl AtomicSwapRouter {
    /// Create new atomic swap router
    pub fn new() -> Result<Self, SwapRouterError> {
        log::info!("Initializing X3 Chain Atomic Swap Router...");

        let route_optimizer = RouteOptimizer::new()?;
        let quote_engine = QuoteEngine::new()?;
        let atomic_executor = AtomicSwapExecutor::new()?;
        let mev_protector = MEVProtector::new()?;
        let gas_optimizer = GasOptimizer::new()?;
        let slippage_controller = SlippageController::new()?;
        let fee_calculator = FeeCalculator::new()?;

        Ok(Self {
            route_optimizer,
            quote_engine,
            atomic_executor,
            mev_protector,
            gas_optimizer,
            slippage_controller,
            fee_calculator,
        })
    }

    /// Execute atomic cross-chain swap with optimization
    pub async fn execute_atomic_swap(
        &self,
        params: SwapParams,
    ) -> Result<SwapExecutionResult, SwapRouterError> {
        log::info!("Starting atomic swap execution: {:?}", params);

        // Step 1: Get comprehensive quotes across all routes
        let quotes = self.quote_engine.get_comprehensive_quotes(&params).await?;

        // Step 2: Optimize route for best execution
        let optimized_route = self
            .route_optimizer
            .optimize_route(&quotes, &params)
            .await?;

        // Step 3: Apply MEV protection
        let protected_route = self.mev_protector.protect_route(&optimized_route).await?;

        // Step 4: Calculate gas optimization
        let gas_params = self
            .gas_optimizer
            .calculate_gas(&protected_route.route)
            .await?;

        // Step 5: Apply slippage protection
        let protected_params = self
            .slippage_controller
            .apply_protection(&params, &protected_route.route)
            .await?;

        // Step 6: Calculate fees
        let fees = self
            .fee_calculator
            .calculate_swap_fees(&protected_route.route, &params)
            .await?;

        // Step 7: Execute atomic bundle
        let execution_result = self
            .atomic_executor
            .execute_swap_bundle(&protected_route.route, &gas_params, &protected_params)
            .await?;

        log::info!("Atomic swap completed: {:?}", execution_result);

        Ok(SwapExecutionResult {
            execution_id: execution_result.execution_id,
            route: protected_route.route,
            fees,
            gas_used: execution_result.gas_used,
            slippage_achieved: execution_result.slippage_achieved,
            execution_time_ms: execution_result.execution_time_ms,
            success: execution_result.success,
        })
    }
}

impl Default for AtomicSwapRouter {
    fn default() -> Self {
        Self::new().expect("Failed to create AtomicSwapRouter")
    }
}

/// Parameters for atomic swap execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapParams {
    pub token_in: H160,
    pub token_out: H160,
    pub amount_in: U256,
    pub min_amount_out: U256,
    pub chain_in: u64,
    pub chain_out: u64,
    pub deadline: u64,
    pub recipient: H160,
    pub slippage_tolerance_bps: u16,
    pub gas_price_limit: Option<U256>,
    #[serde(default)]
    pub source_vm: VmType,
    #[serde(default)]
    pub destination_vm: VmType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VmType {
    Evm,
    Svm,
    X3Vm,
    #[default]
    Unknown,
}

/// Result of swap execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapExecutionResult {
    pub execution_id: H256,
    pub route: SwapRoute,
    pub fees: ProtocolFees,
    pub gas_used: U256,
    pub slippage_achieved: f64,
    pub execution_time_ms: u64,
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwapRouterError {
    RouteNotFound,
    InsufficientLiquidity,
    HighSlippage,
    GasTooExpensive,
    ExecutionFailed,
    MEVDetected,
    InvalidParams,
}

impl From<crate::mev_protection::MEVProtectionError> for SwapRouterError {
    fn from(_: crate::mev_protection::MEVProtectionError) -> Self {
        SwapRouterError::MEVDetected
    }
}

impl core::fmt::Display for SwapRouterError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            SwapRouterError::RouteNotFound => write!(f, "No viable route found"),
            SwapRouterError::InsufficientLiquidity => write!(f, "Insufficient liquidity"),
            SwapRouterError::HighSlippage => write!(f, "Slippage exceeds tolerance"),
            SwapRouterError::GasTooExpensive => write!(f, "Gas costs too high"),
            SwapRouterError::ExecutionFailed => write!(f, "Swap execution failed"),
            SwapRouterError::MEVDetected => write!(f, "MEV attack detected"),
            SwapRouterError::InvalidParams => write!(f, "Invalid swap parameters"),
        }
    }
}
