//! Pool creation and initial liquidity bootstrap.
//!
//! Wraps `x3_dex::AMMPool::create_pool` and adds basic sanity checks that
//! the raw DEX layer does not enforce.

/// Request descriptor for creating a new AMM pool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchRequest {
    pub token_a: u64,
    pub token_b: u64,
    /// Initial liquidity for token A side (raw units).
    pub initial_a: u128,
    /// Initial liquidity for token B side (raw units).
    pub initial_b: u128,
    /// Fee in basis points (max 1000 = 10 %).
    pub fee_bps: u32,
}

/// Errors returned by `Launchpad`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchError {
    /// Both token IDs are the same.
    SameToken,
    /// At least one initial liquidity value is zero.
    ZeroInitialLiquidity,
    /// Fee exceeds the 1000 bps ceiling.
    FeeTooHigh,
}

/// Default fee used when none is supplied: 30 bps (0.30 %).
pub const DEFAULT_FEE_BPS: u32 = 30;
/// Maximum allowed pool fee: 1000 bps (10 %).
pub const MAX_FEE_BPS: u32 = 1_000;

/// AMM pool launch helper.
pub struct Launchpad;

impl Launchpad {
    /// Build a launch request with the default fee.
    pub fn build(
        token_a: u64,
        token_b: u64,
        initial_a: u128,
        initial_b: u128,
    ) -> Result<LaunchRequest, LaunchError> {
        Self::build_with_fee(token_a, token_b, initial_a, initial_b, DEFAULT_FEE_BPS)
    }

    /// Build a launch request with an explicit fee.
    pub fn build_with_fee(
        token_a: u64,
        token_b: u64,
        initial_a: u128,
        initial_b: u128,
        fee_bps: u32,
    ) -> Result<LaunchRequest, LaunchError> {
        if token_a == token_b {
            return Err(LaunchError::SameToken);
        }
        if initial_a == 0 || initial_b == 0 {
            return Err(LaunchError::ZeroInitialLiquidity);
        }
        if fee_bps > MAX_FEE_BPS {
            return Err(LaunchError::FeeTooHigh);
        }
        Ok(LaunchRequest {
            token_a,
            token_b,
            initial_a,
            initial_b,
            fee_bps,
        })
    }
}
