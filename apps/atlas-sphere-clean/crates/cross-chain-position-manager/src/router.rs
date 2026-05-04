//! Route optimization system for cross-chain operations
//!
//! This module provides:
//! - Multi-hop route finding
//! - Gas cost optimization
//! - Slippage-aware routing
//! - Fallback route system
//! - Route simulation

use crate::config::PositionManagerConfig;
use crate::error::{PositionManagerError, Result};
use crate::types::{RouteOptimizationParams, SwapRoute, H160, H256, U256};
use serde::{Deserialize, Serialize};
use sp_std::vec::Vec;

/// Route optimizer for finding optimal paths
#[derive(Debug, Clone)]
pub struct RouteOptimizer {
    /// Supported chains
    supported_chains: Vec<u64>,
    /// DEX routers per chain
    dex_routers: sp_std::collections::btree_map::BTreeMap<u64, Vec<DexRouter>>,
    /// Bridge contracts
    bridge_contracts: sp_std::collections::btree_map::BTreeMap<(u64, u64), BridgeContract>,
    /// Route cache
    route_cache: sp_std::collections::btree_map::BTreeMap<RouteKey, CachedRoute>,
    /// Configuration
    config: PositionManagerConfig,
}

impl RouteOptimizer {
    /// Create a new route optimizer
    pub fn new(config: &PositionManagerConfig) -> Result<Self> {
        Ok(Self {
            supported_chains: config.chain_configs.keys().cloned().collect(),
            dex_routers: sp_std::collections::btree_map::BTreeMap::new(),
            bridge_contracts: sp_std::collections::btree_map::BTreeMap::new(),
            route_cache: sp_std::collections::btree_map::BTreeMap::new(),
            config: config.clone(),
        })
    }

    /// Find optimal route between chains
    pub async fn find_optimal_route(
        &self,
        source_chain: u64,
        target_chain: u64,
        source_asset: H160,
        target_asset: H160,
        amount: U256,
        params: &RouteOptimizationParams,
    ) -> Result<SwapRoute> {
        // Check cache first
        let route_key = RouteKey {
            source_chain,
            target_chain,
            source_asset,
            target_asset,
            amount,
        };

        if let Some(cached) = self.route_cache.get(&route_key) {
            if !cached.is_expired() {
                return Ok(cached.route.clone());
            }
        }

        // Find all possible routes
        let routes = self
            .find_all_routes(
                source_chain,
                target_chain,
                source_asset,
                target_asset,
                amount,
                params,
            )
            .await?;

        // Select optimal route based on parameters
        let optimal_route = self.select_optimal_route(&routes, params)?;

        // Cache the result
        // Note: In a real implementation, we'd use a mutable cache
        // For now, we just return the route

        Ok(optimal_route)
    }

    /// Find all possible routes
    async fn find_all_routes(
        &self,
        source_chain: u64,
        target_chain: u64,
        source_asset: H160,
        target_asset: H160,
        amount: U256,
        params: &RouteOptimizationParams,
    ) -> Result<Vec<SwapRoute>> {
        let mut routes = Vec::new();

        // Direct route (if same chain)
        if source_chain == target_chain {
            let direct_route = self
                .build_direct_route(source_chain, source_asset, target_asset, amount)
                .await?;
            routes.push(direct_route);
        }

        // Single-hop bridge route
        if source_chain != target_chain {
            let bridge_route = self
                .build_bridge_route(
                    source_chain,
                    target_chain,
                    source_asset,
                    target_asset,
                    amount,
                )
                .await?;
            routes.push(bridge_route);
        }

        // Multi-hop routes (through intermediate chains)
        if params.max_hops > 1 {
            let multi_hop_routes = self
                .find_multi_hop_routes(
                    source_chain,
                    target_chain,
                    source_asset,
                    target_asset,
                    amount,
                    params,
                )
                .await?;
            routes.extend(multi_hop_routes);
        }

        // Filter routes based on preferences
        routes = self.filter_routes(routes, params)?;

        Ok(routes)
    }

    /// Build direct swap route
    async fn build_direct_route(
        &self,
        chain_id: u64,
        source_asset: H160,
        target_asset: H160,
        amount: U256,
    ) -> Result<SwapRoute> {
        let dex_router = self.get_dex_router(chain_id)?;

        // Estimate output amount
        let amount_out = self
            .estimate_swap_output(chain_id, source_asset, target_asset, amount)
            .await?;

        // Estimate gas
        let gas_estimate = self
            .estimate_swap_gas(chain_id, source_asset, target_asset, amount)
            .await?;

        Ok(SwapRoute {
            source_chain: chain_id,
            target_chain: chain_id,
            source_asset,
            target_asset,
            amount_in: amount,
            amount_out,
            hops: vec![chain_id],
            gas_estimate,
            price_impact: self.calculate_price_impact(amount, amount_out)?,
        })
    }

    /// Build bridge route
    async fn build_bridge_route(
        &self,
        source_chain: u64,
        target_chain: u64,
        source_asset: H160,
        target_asset: H160,
        amount: U256,
    ) -> Result<SwapRoute> {
        // Get bridge contract
        let bridge = self.get_bridge_contract(source_chain, target_chain)?;

        // Estimate bridge fee
        let bridge_fee = self
            .estimate_bridge_fee(source_chain, target_chain, amount)
            .await?;

        // Estimate output (after bridge fee)
        let amount_out = amount.checked_sub(bridge_fee).unwrap_or(U256::zero());

        // Estimate gas (includes bridge gas)
        let gas_estimate = self.estimate_bridge_gas(source_chain, target_chain).await?;

        Ok(SwapRoute {
            source_chain,
            target_chain,
            source_asset,
            target_asset,
            amount_in: amount,
            amount_out,
            hops: vec![source_chain, target_chain],
            gas_estimate,
            price_impact: self.calculate_price_impact(amount, amount_out)?,
        })
    }

    /// Find multi-hop routes
    async fn find_multi_hop_routes(
        &self,
        source_chain: u64,
        target_chain: u64,
        source_asset: H160,
        target_asset: H160,
        amount: U256,
        params: &RouteOptimizationParams,
    ) -> Result<Vec<SwapRoute>> {
        let mut routes = Vec::new();

        // Find intermediate chains
        let intermediate_chains =
            self.find_intermediate_chains(source_chain, target_chain, params)?;

        for intermediate in intermediate_chains {
            // Skip if in avoid list
            if params.avoid_chains.contains(&intermediate) {
                continue;
            }

            // Build route through intermediate
            let route = self
                .build_multi_hop_route(
                    source_chain,
                    intermediate,
                    target_chain,
                    source_asset,
                    target_asset,
                    amount,
                )
                .await?;

            routes.push(route);
        }

        Ok(routes)
    }

    /// Find intermediate chains for multi-hop routing
    fn find_intermediate_chains(
        &self,
        source_chain: u64,
        target_chain: u64,
        params: &RouteOptimizationParams,
    ) -> Result<Vec<u64>> {
        let mut intermediates = Vec::new();

        for &chain in &self.supported_chains {
            if chain == source_chain || chain == target_chain {
                continue;
            }

            // Check if bridge exists
            if self.has_bridge(source_chain, chain) && self.has_bridge(chain, target_chain) {
                // Check if preferred
                if params.preferred_chains.is_empty() || params.preferred_chains.contains(&chain) {
                    intermediates.push(chain);
                }
            }
        }

        Ok(intermediates)
    }

    /// Build multi-hop route
    async fn build_multi_hop_route(
        &self,
        source_chain: u64,
        intermediate_chain: u64,
        target_chain: u64,
        source_asset: H160,
        target_asset: H160,
        amount: U256,
    ) -> Result<SwapRoute> {
        // First hop: source -> intermediate
        let first_hop_fee = self
            .estimate_bridge_fee(source_chain, intermediate_chain, amount)
            .await?;
        let amount_after_first = amount.checked_sub(first_hop_fee).unwrap_or(U256::zero());

        // Second hop: intermediate -> target
        let second_hop_fee = self
            .estimate_bridge_fee(intermediate_chain, target_chain, amount_after_first)
            .await?;
        let amount_out = amount_after_first
            .checked_sub(second_hop_fee)
            .unwrap_or(U256::zero());

        // Total gas
        let gas_estimate = self
            .estimate_bridge_gas(source_chain, intermediate_chain)
            .await?
            .checked_add(
                self.estimate_bridge_gas(intermediate_chain, target_chain)
                    .await?,
            )
            .unwrap_or(U256::zero());

        Ok(SwapRoute {
            source_chain,
            target_chain,
            source_asset,
            target_asset,
            amount_in: amount,
            amount_out,
            hops: vec![source_chain, intermediate_chain, target_chain],
            gas_estimate,
            price_impact: self.calculate_price_impact(amount, amount_out)?,
        })
    }

    /// Filter routes based on parameters
    fn filter_routes(
        &self,
        routes: Vec<SwapRoute>,
        params: &RouteOptimizationParams,
    ) -> Result<Vec<SwapRoute>> {
        let mut filtered = Vec::new();

        for route in routes {
            // Check minimum liquidity
            if route.amount_out < params.min_liquidity {
                continue;
            }

            // Check if chains are in avoid list
            if route
                .hops
                .iter()
                .any(|chain| params.avoid_chains.contains(chain))
            {
                continue;
            }

            filtered.push(route);
        }

        Ok(filtered)
    }

    /// Select optimal route from candidates
    fn select_optimal_route(
        &self,
        routes: &[SwapRoute],
        params: &RouteOptimizationParams,
    ) -> Result<SwapRoute> {
        if routes.is_empty() {
            return Err(PositionManagerError::NoRoutesFound);
        }

        // Score routes based on parameters
        let mut best_route = routes[0].clone();
        let mut best_score = self.calculate_route_score(&best_route, params)?;

        for route in routes.iter().skip(1) {
            let score = self.calculate_route_score(route, params)?;
            if score > best_score {
                best_score = score;
                best_route = route.clone();
            }
        }

        Ok(best_route)
    }

    /// Calculate route score
    fn calculate_route_score(
        &self,
        route: &SwapRoute,
        params: &RouteOptimizationParams,
    ) -> Result<f64> {
        let mut score = 0.0;

        // Output amount score (higher is better)
        let output_score = route.amount_out.as_u128() as f64;
        score += output_score * params.slippage_weight;

        // Gas cost score (lower is better)
        let gas_score = 1.0 / (route.gas_estimate.as_u128() as f64 + 1.0);
        score += gas_score * params.gas_weight;

        // Hop count score (fewer is better)
        let hop_score = 1.0 / (route.hops.len() as f64);
        score += hop_score * params.time_weight;

        Ok(score)
    }

    /// Estimate swap output
    async fn estimate_swap_output(
        &self,
        chain_id: u64,
        source_asset: H160,
        target_asset: H160,
        amount: U256,
    ) -> Result<U256> {
        // Placeholder - would query DEX for actual output
        Ok(amount) // 1:1 for now
    }

    /// Estimate swap gas
    async fn estimate_swap_gas(
        &self,
        chain_id: u64,
        source_asset: H160,
        target_asset: H160,
        amount: U256,
    ) -> Result<U256> {
        let chain_config = self
            .config
            .chain_configs
            .get(&chain_id)
            .ok_or_else(|| PositionManagerError::ChainNotFound(chain_id))?;

        Ok(U256::from(150_000)) // Base swap gas
    }

    /// Estimate bridge fee
    async fn estimate_bridge_fee(
        &self,
        source_chain: u64,
        target_chain: u64,
        amount: U256,
    ) -> Result<U256> {
        // Base fee + percentage
        let base_fee = U256::from(1_000_000_000_000_000u128); // 0.001 ETH
        let percentage_fee = amount
            .checked_mul(U256::from(30)) // 0.3%
            .unwrap_or(U256::zero())
            .checked_div(U256::from(10000))
            .unwrap_or(U256::zero());

        Ok(base_fee.saturating_add(percentage_fee))
    }

    /// Estimate bridge gas
    async fn estimate_bridge_gas(&self, source_chain: u64, target_chain: u64) -> Result<U256> {
        Ok(U256::from(200_000)) // Base bridge gas
    }

    /// Calculate price impact
    fn calculate_price_impact(&self, amount_in: U256, amount_out: U256) -> Result<f64> {
        if amount_in.is_zero() {
            return Ok(0.0);
        }

        let diff = if amount_in > amount_out {
            amount_in - amount_out
        } else {
            amount_out - amount_in
        };

        let impact = diff.as_u128() as f64 / amount_in.as_u128() as f64;
        Ok(impact)
    }

    /// Get DEX router for a chain
    fn get_dex_router(&self, chain_id: u64) -> Result<&DexRouter> {
        self.dex_routers
            .get(&chain_id)
            .and_then(|routers| routers.first())
            .ok_or_else(|| PositionManagerError::DexRouterNotFound(chain_id))
    }

    /// Get bridge contract between chains
    fn get_bridge_contract(&self, source_chain: u64, target_chain: u64) -> Result<&BridgeContract> {
        self.bridge_contracts
            .get(&(source_chain, target_chain))
            .ok_or_else(|| PositionManagerError::BridgeNotFound(source_chain, target_chain))
    }

    /// Check if bridge exists between chains
    fn has_bridge(&self, source_chain: u64, target_chain: u64) -> bool {
        self.bridge_contracts
            .contains_key(&(source_chain, target_chain))
    }

    /// Add DEX router
    pub fn add_dex_router(&mut self, chain_id: u64, router: DexRouter) {
        self.dex_routers
            .entry(chain_id)
            .or_insert_with(Vec::new)
            .push(router);
    }

    /// Add bridge contract
    pub fn add_bridge_contract(
        &mut self,
        source_chain: u64,
        target_chain: u64,
        bridge: BridgeContract,
    ) {
        self.bridge_contracts
            .insert((source_chain, target_chain), bridge);
    }

    /// Simulate route execution
    pub async fn simulate_route(&self, route: &SwapRoute) -> Result<SimulationResult> {
        // Check liquidity
        let liquidity_check = self.check_liquidity(route).await?;

        // Estimate actual output
        let actual_output = self.estimate_actual_output(route).await?;

        // Calculate actual price impact
        let actual_impact = self.calculate_price_impact(route.amount_in, actual_output)?;

        Ok(SimulationResult {
            feasible: liquidity_check,
            estimated_output: actual_output,
            actual_price_impact: actual_impact,
            gas_used: route.gas_estimate,
            warnings: if !liquidity_check {
                vec!["Insufficient liquidity".to_string()]
            } else {
                Vec::new()
            },
        })
    }

    /// Check route liquidity
    async fn check_liquidity(&self, route: &SwapRoute) -> Result<bool> {
        // Placeholder - would check actual DEX liquidity
        Ok(true)
    }

    /// Estimate actual output
    async fn estimate_actual_output(&self, route: &SwapRoute) -> Result<U256> {
        // Placeholder - would get actual quote from DEX
        Ok(route.amount_out)
    }
}

/// DEX router information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexRouter {
    pub chain_id: u64,
    pub router_address: H160,
    pub name: String,
    pub version: String,
}

/// Bridge contract information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeContract {
    pub source_chain: u64,
    pub target_chain: u64,
    pub contract_address: H160,
    pub name: String,
    pub fee_percentage: f64,
}

/// Route cache key
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RouteKey {
    pub source_chain: u64,
    pub target_chain: u64,
    pub source_asset: H160,
    pub target_asset: H160,
    pub amount: U256,
}

/// Cached route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedRoute {
    pub route: SwapRoute,
    pub timestamp: u64,
    pub ttl_ms: u64,
}

impl CachedRoute {
    /// Check if cache entry is expired
    pub fn is_expired(&self) -> bool {
        let now = sp_io::offchain::timestamp().unix_millis();
        now > self.timestamp + self.ttl_ms
    }
}

/// Simulation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub feasible: bool,
    pub estimated_output: U256,
    pub actual_price_impact: f64,
    pub gas_used: U256,
    pub warnings: Vec<String>,
}

/// Execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub route: SwapRoute,
    pub steps: Vec<ExecutionStep>,
    pub total_gas: U256,
    pub estimated_time_ms: u64,
}

/// Execution step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub step_type: StepType,
    pub chain_id: u64,
    pub contract: H160,
    pub data: Vec<u8>,
    pub value: U256,
    pub gas_estimate: U256,
}

/// Step types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepType {
    Approve,
    Swap,
    Bridge,
    Claim,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_optimizer() {
        let config = PositionManagerConfig::default();
        let optimizer = RouteOptimizer::new(&config).unwrap();

        assert_eq!(optimizer.supported_chains.len(), config.chain_configs.len());
    }

    #[test]
    fn test_cached_route_expiry() {
        let route = SwapRoute {
            source_chain: 1,
            target_chain: 137,
            source_asset: H160::zero(),
            target_asset: H160::zero(),
            amount_in: U256::from(1000),
            amount_out: U256::from(1000),
            hops: vec![1, 137],
            gas_estimate: U256::from(100_000),
            price_impact: 0.001,
        };

        let cached = CachedRoute {
            route,
            timestamp: 0,
            ttl_ms: 60000,
        };

        assert!(cached.is_expired());
    }
}
