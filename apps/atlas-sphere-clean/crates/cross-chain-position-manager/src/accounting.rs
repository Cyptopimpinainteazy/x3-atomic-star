//! Cross-chain accounting engine for position tracking
//!
//! This module provides:
//! - USD normalization across chains
//! - Multi-chain balance tracking
//! - Position snapshot system
//! - Fast state diffing

use crate::config::PositionManagerConfig;
use crate::error::{PositionManagerError, Result};
use crate::types::{AssetInfo, PositionId, PositionState, H160, H256, U256};
use serde::{Deserialize, Serialize};
use sp_std::vec::Vec;

/// Position snapshot for state tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSnapshot {
    pub snapshot_id: H256,
    pub timestamp: u64,
    pub positions: Vec<PositionBalance>,
    pub total_value_usd: U256,
    pub chain_breakdown: Vec<ChainBalance>,
}

/// Balance for a single position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionBalance {
    pub position_id: PositionId,
    pub asset: H160,
    pub amount: U256,
    pub value_usd: U256,
    pub chain_id: u64,
}

/// Balance breakdown for a chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainBalance {
    pub chain_id: u64,
    pub total_value_usd: U256,
    pub asset_count: usize,
    pub positions_count: usize,
}

/// USD normalizer for cross-chain value comparison
#[derive(Debug, Clone)]
pub struct UsdNormalizer {
    /// Price feeds for assets
    price_feeds: sp_std::collections::btree_map::BTreeMap<H160, U256>,
    /// Stablecoin addresses (assumed 1:1 with USD)
    stablecoins: Vec<H160>,
    /// Last update timestamp
    last_update: u64,
}

impl UsdNormalizer {
    /// Create a new USD normalizer
    pub fn new() -> Result<Self> {
        Ok(Self {
            price_feeds: sp_std::collections::btree_map::BTreeMap::new(),
            stablecoins: Vec::new(),
            last_update: 0,
        })
    }

    /// Normalize asset amount to USD value
    pub fn normalize_to_usd(&self, asset: &H160, amount: U256) -> Result<U256> {
        // Check if stablecoin
        if self.stablecoins.contains(asset) {
            return Ok(amount);
        }

        // Get price from feed
        let price = self
            .price_feeds
            .get(asset)
            .ok_or_else(|| PositionManagerError::PriceFeedNotFound(format!("{:?}", asset)))?;

        // Calculate USD value: amount * price / 10^18
        let value = amount
            .checked_mul(*price)
            .ok_or_else(|| PositionManagerError::ArithmeticOverflow)?
            .checked_div(U256::from(10).pow(U256::from(18)))
            .ok_or_else(|| PositionManagerError::ArithmeticOverflow)?;

        Ok(value)
    }

    /// Update price feed for an asset
    pub fn update_price(&mut self, asset: H160, price: U256) {
        self.price_feeds.insert(asset, price);
        self.last_update = sp_io::offchain::timestamp().unix_millis();
    }

    /// Register stablecoin
    pub fn register_stablecoin(&mut self, asset: H160) {
        if !self.stablecoins.contains(&asset) {
            self.stablecoins.push(asset);
        }
    }

    /// Get last update timestamp
    pub fn last_update(&self) -> u64 {
        self.last_update
    }
}

/// Accounting engine for position management
#[derive(Debug, Clone)]
pub struct AccountingEngine {
    /// USD normalizer
    normalizer: UsdNormalizer,
    /// Position snapshots
    snapshots: Vec<PositionSnapshot>,
    /// Current balances
    current_balances: sp_std::collections::btree_map::BTreeMap<PositionId, PositionBalance>,
    /// Configuration
    config: PositionManagerConfig,
}

impl AccountingEngine {
    /// Create a new accounting engine
    pub fn new() -> Result<Self> {
        let normalizer = UsdNormalizer::new()?;
        let config = PositionManagerConfig::default();

        Ok(Self {
            normalizer,
            snapshots: Vec::new(),
            current_balances: sp_std::collections::btree_map::BTreeMap::new(),
            config,
        })
    }

    /// Create with custom configuration
    pub fn with_config(config: PositionManagerConfig) -> Result<Self> {
        let normalizer = UsdNormalizer::new()?;

        Ok(Self {
            normalizer,
            snapshots: Vec::new(),
            current_balances: sp_std::collections::btree_map::BTreeMap::new(),
            config,
        })
    }

    /// Update position balance
    pub fn update_balance(
        &mut self,
        position_id: PositionId,
        asset: H160,
        amount: U256,
        chain_id: u64,
    ) -> Result<()> {
        // Normalize to USD
        let value_usd = self.normalizer.normalize_to_usd(&asset, amount)?;

        let balance = PositionBalance {
            position_id: position_id.clone(),
            asset,
            amount,
            value_usd,
            chain_id,
        };

        self.current_balances.insert(position_id, balance);
        Ok(())
    }

    /// Get position balance
    pub fn get_balance(&self, position_id: &PositionId) -> Option<&PositionBalance> {
        self.current_balances.get(position_id)
    }

    /// Take a snapshot of current state
    pub fn take_snapshot(&mut self) -> Result<PositionSnapshot> {
        let timestamp = sp_io::offchain::timestamp().unix_millis();
        let mut total_value_usd = U256::zero();
        let mut chain_totals: sp_std::collections::btree_map::BTreeMap<u64, (U256, usize, usize)> =
            sp_std::collections::btree_map::BTreeMap::new();

        // Collect balances and calculate totals
        let mut positions = Vec::new();
        for (_, balance) in self.current_balances.iter() {
            positions.push(balance.clone());
            total_value_usd = total_value_usd.saturating_add(balance.value_usd);

            let entry = chain_totals
                .entry(balance.chain_id)
                .or_insert((U256::zero(), 0, 0));
            entry.0 = entry.0.saturating_add(balance.value_usd);
            entry.1 += 1;
            entry.2 += 1;
        }

        // Build chain breakdown
        let chain_breakdown: Vec<ChainBalance> = chain_totals
            .into_iter()
            .map(
                |(chain_id, (total, asset_count, positions_count))| ChainBalance {
                    chain_id,
                    total_value_usd: total,
                    asset_count,
                    positions_count,
                },
            )
            .collect();

        // Generate snapshot ID
        let snapshot_id = self.generate_snapshot_id(timestamp, &positions);

        let snapshot = PositionSnapshot {
            snapshot_id,
            timestamp,
            positions,
            total_value_usd,
            chain_breakdown,
        };

        // Store snapshot
        self.snapshots.push(snapshot.clone());

        // Keep only last 100 snapshots
        if self.snapshots.len() > 100 {
            self.snapshots.remove(0);
        }

        Ok(snapshot)
    }

    /// Get latest snapshot
    pub fn latest_snapshot(&self) -> Option<&PositionSnapshot> {
        self.snapshots.last()
    }

    /// Get snapshot history
    pub fn snapshot_history(&self, limit: usize) -> Vec<&PositionSnapshot> {
        let start = if self.snapshots.len() > limit {
            self.snapshots.len() - limit
        } else {
            0
        };
        self.snapshots[start..].iter().collect()
    }

    /// Calculate diff between two snapshots
    pub fn calculate_diff(
        &self,
        snapshot1: &PositionSnapshot,
        snapshot2: &PositionSnapshot,
    ) -> SnapshotDiff {
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut changed = Vec::new();

        // Find added and changed positions
        for pos2 in &snapshot2.positions {
            if let Some(pos1) = snapshot1
                .positions
                .iter()
                .find(|p| p.position_id == pos2.position_id)
            {
                if pos1.amount != pos2.amount || pos1.value_usd != pos2.value_usd {
                    changed.push(PositionDiff {
                        position_id: pos2.position_id.clone(),
                        old_amount: pos1.amount,
                        new_amount: pos2.amount,
                        old_value_usd: pos1.value_usd,
                        new_value_usd: pos2.value_usd,
                    });
                }
            } else {
                added.push(pos2.clone());
            }
        }

        // Find removed positions
        for pos1 in &snapshot1.positions {
            if !snapshot2
                .positions
                .iter()
                .any(|p| p.position_id == pos1.position_id)
            {
                removed.push(pos1.clone());
            }
        }

        let value_change = snapshot2
            .total_value_usd
            .checked_sub(snapshot1.total_value_usd)
            .unwrap_or(U256::zero());

        SnapshotDiff {
            from_timestamp: snapshot1.timestamp,
            to_timestamp: snapshot2.timestamp,
            added,
            removed,
            changed,
            total_value_change: value_change,
            percentage_change: if snapshot1.total_value_usd > U256::zero() {
                let change = value_change.as_u128() as f64;
                let base = snapshot1.total_value_usd.as_u128() as f64;
                (change / base) * 100.0
            } else {
                0.0
            },
        }
    }

    /// Get portfolio summary
    pub async fn get_portfolio_summary(&self) -> Result<PortfolioSummary> {
        let mut total_value_usd = U256::zero();
        let mut chain_breakdown = Vec::new();
        let mut asset_breakdown: sp_std::collections::btree_map::BTreeMap<
            H160,
            (String, U256, Vec<(u64, U256)>),
        > = sp_std::collections::btree_map::BTreeMap::new();

        for (_, balance) in self.current_balances.iter() {
            total_value_usd = total_value_usd.saturating_add(balance.value_usd);

            // Update chain breakdown
            if let Some(chain) = chain_breakdown
                .iter_mut()
                .find(|c: &mut ChainSummary| c.chain_id == balance.chain_id)
            {
                chain.total_value_usd = chain.total_value_usd.saturating_add(balance.value_usd);
                chain.positions_count += 1;
            } else {
                chain_breakdown.push(ChainSummary {
                    chain_id: balance.chain_id,
                    total_value_usd: balance.value_usd,
                    positions_count: 1,
                    gas_efficiency_score: 1.0,
                });
            }

            // Update asset breakdown
            let entry = asset_breakdown.entry(balance.asset).or_insert((
                format!("Asset_{:?}", balance.asset),
                U256::zero(),
                Vec::new(),
            ));
            entry.1 = entry.1.saturating_add(balance.amount);
            entry.2.push((balance.chain_id, balance.amount));
        }

        let asset_breakdown: Vec<AssetSummary> = asset_breakdown
            .into_iter()
            .map(|(address, (symbol, total_amount, chains))| AssetSummary {
                asset_address: address,
                symbol,
                total_amount,
                total_value_usd: self
                    .normalizer
                    .normalize_to_usd(&address, total_amount)
                    .unwrap_or(U256::zero()),
                chains_distribution: chains,
            })
            .collect();

        Ok(PortfolioSummary {
            total_value_usd,
            chain_breakdown,
            asset_breakdown,
            risk_score: 0.5, // Placeholder
            rebalance_needed: false,
            active_arbitrage_ops: 0,
        })
    }

    /// Generate snapshot ID
    fn generate_snapshot_id(&self, timestamp: u64, positions: &[PositionBalance]) -> H256 {
        use sp_core::Hasher;
        use sp_runtime::traits::BlakeTwo256;

        let mut hasher = BlakeTwo256::default();
        hasher.hash(&timestamp.to_le_bytes());
        for pos in positions {
            hasher.hash(pos.position_id.as_bytes());
            hasher.hash(&pos.amount.as_bytes());
        }
        hasher.hash(&self.config.risk_config.max_position_size_usd.as_bytes());
        hasher.hash(&positions.len().to_le_bytes());
        hasher.hash(&self.last_update().to_le_bytes());
        hasher.hash(&self.snapshots.len().to_le_bytes());
        hasher.hash(&self.config.risk_config.max_exposure_per_chain.to_le_bytes());
        hasher.hash(&self.config.risk_config.max_correlation.to_le_bytes());
        hasher.hash(&self.config.risk_config.liquidation_threshold.to_le_bytes());
        hasher.hash(&self.config.risk_config.stop_loss_percentage.to_le_bytes());
        hasher.hash(&self.normalizer.last_update().to_le_bytes());
        hasher.hash(&self.normalizer.stablecoins.len().to_le_bytes());
        hasher.hash(&self.normalizer.price_feeds.len().to_le_bytes());
        hasher.hash(&self.config.chain_configs.len().to_le_bytes());
        hasher.hash(&self.config.rebalance_threshold.to_le_bytes());
        hasher.hash(&self.config.auto_rebalance.to_le_bytes());
        hasher.hash(&self.config.auto_arbitrage.to_le_bytes());
        hasher.hash(&self.config.tracking_interval_ms.to_le_bytes());
        hasher.hash(&self.config.snapshot_interval_ms.to_le_bytes());
        hasher.hash(&self.config.max_concurrent_operations.to_le_bytes());
        hasher.hash(&self.config.operation_timeout_ms.to_le_bytes());
        hasher.hash(&self.config.event_retention_hours.to_le_bytes());
        hasher.hash(&self.config.kill_switch_enabled.to_le_bytes());
        hasher.hash(&self.config.emergency_contact.to_le_bytes());
        hasher.hash(&self.config.log_level.as_bytes());
        hasher.hash(&self.config.metrics_enabled.to_le_bytes());
        hasher.hash(&self.config.debug_mode.to_le_bytes());
        H256::from_slice(hasher.finish().as_ref())
    }

    /// Get last update timestamp
    pub fn last_update(&self) -> u64 {
        self.normalizer.last_update()
    }
}

/// Difference between two snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDiff {
    pub from_timestamp: u64,
    pub to_timestamp: u64,
    pub added: Vec<PositionBalance>,
    pub removed: Vec<PositionBalance>,
    pub changed: Vec<PositionDiff>,
    pub total_value_change: U256,
    pub percentage_change: f64,
}

/// Change in a position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionDiff {
    pub position_id: PositionId,
    pub old_amount: U256,
    pub new_amount: U256,
    pub old_value_usd: U256,
    pub new_value_usd: U256,
}

/// Portfolio summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSummary {
    pub total_value_usd: U256,
    pub chain_breakdown: Vec<ChainSummary>,
    pub asset_breakdown: Vec<AssetSummary>,
    pub risk_score: f64,
    pub rebalance_needed: bool,
    pub active_arbitrage_ops: usize,
}

/// Chain summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainSummary {
    pub chain_id: u64,
    pub total_value_usd: U256,
    pub positions_count: usize,
    pub gas_efficiency_score: f64,
}

/// Asset summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSummary {
    pub asset_address: H160,
    pub symbol: String,
    pub total_amount: U256,
    pub total_value_usd: U256,
    pub chains_distribution: Vec<(u64, U256)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usd_normalizer() {
        let mut normalizer = UsdNormalizer::new().unwrap();
        let asset = H160::random();
        let price = U256::from(1_000_000_000_000_000_000u128); // 1.0 in 18 decimals

        normalizer.update_price(asset, price);

        let amount = U256::from(2_000_000_000_000_000_000u128); // 2.0 tokens
        let value = normalizer.normalize_to_usd(&asset, amount).unwrap();

        assert_eq!(value, U256::from(2_000_000_000_000_000_000u128));
    }

    #[test]
    fn test_accounting_engine() {
        let mut engine = AccountingEngine::new().unwrap();
        let position_id = PositionId::new();
        let asset = H160::random();
        let amount = U256::from(1_000_000_000_000_000_000u128);

        engine
            .update_balance(position_id.clone(), asset, amount, 1)
            .unwrap();

        let balance = engine.get_balance(&position_id).unwrap();
        assert_eq!(balance.amount, amount);
    }
}
