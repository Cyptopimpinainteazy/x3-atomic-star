//! Type definitions for readiness reports

use serde::{Deserialize, Serialize};

/// Comprehensive readiness report for X3 Atomic Star v0.4
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessReport {
    /// ISO 8601 timestamp when report was generated
    pub timestamp: String,
    /// Version string (e.g., "0.4.0")
    pub version: String,
    /// Current kernel status snapshot
    pub kernel_status: KernelStatus,
    /// Canonical supply invariant verified
    pub supply_invariant: bool,
    /// Emergency halt mechanism working
    pub halt_functional: bool,
    /// Permission controls enforced
    pub permissions_enforced: bool,
    /// Cross-domain balance reconciliation OK
    pub balance_reconciliation: bool,
    /// Computed overall readiness flag
    pub overall_ready: bool,
}

/// Snapshot of kernel status at report time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelStatus {
    /// Total canonical supply
    pub supply: u128,
    /// Number of active accounts
    pub account_count: usize,
    /// System halted state
    pub halted: bool,
    /// Total amount locked across all accounts
    pub total_locked: u128,
}

impl Default for ReadinessReport {
    fn default() -> Self {
        Self::new()
    }
}

impl ReadinessReport {
    /// Create a new readiness report with default/current values
    pub fn new() -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: "0.4.0".to_string(),
            kernel_status: KernelStatus {
                supply: 0,
                account_count: 0,
                halted: false,
                total_locked: 0,
            },
            supply_invariant: false,
            halt_functional: false,
            permissions_enforced: false,
            balance_reconciliation: false,
            overall_ready: false,
        }
    }

    /// Check if all readiness criteria are met
    pub fn is_ready(&self) -> bool {
        self.supply_invariant
            && self.halt_functional
            && self.permissions_enforced
            && self.balance_reconciliation
    }

    /// Set all readiness flags to true (fully ready state)
    pub fn mark_ready(&mut self) {
        self.supply_invariant = true;
        self.halt_functional = true;
        self.permissions_enforced = true;
        self.balance_reconciliation = true;
        self.overall_ready = self.is_ready();
    }

    /// Return readiness percentage (0-100)
    pub fn readiness_percentage(&self) -> u32 {
        let checks = [
            self.supply_invariant,
            self.halt_functional,
            self.permissions_enforced,
            self.balance_reconciliation,
        ];
        let passed = checks.iter().filter(|&&x| x).count() as u32;
        (passed * 100) / 4
    }
}
