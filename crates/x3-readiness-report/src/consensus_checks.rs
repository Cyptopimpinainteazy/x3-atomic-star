//! Consensus and validator liveness checks.

use crate::types::{KernelStatus, ReadinessCheck};

/// Conservative consensus health check.
///
/// We only mark PASS when we can observe both account_count and halted state.
pub fn collect_consensus_health(status: &KernelStatus) -> ReadinessCheck {
    match (status.account_count, status.halted) {
        (Some(n), Some(false)) if n > 0 => {
            ReadinessCheck::pass(format!("consensus-visible state healthy: account_count={}", n))
        }
        (Some(_), Some(true)) => {
            ReadinessCheck::fail("runtime currently halted; consensus not healthy for user traffic")
        }
        _ => ReadinessCheck::unknown("insufficient consensus telemetry in kernel snapshot"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consensus_health_passes_when_live() {
        let status = KernelStatus {
            account_count: Some(12),
            halted: Some(false),
            ..KernelStatus::default()
        };
        let check = collect_consensus_health(&status);
        assert!(check.is_pass());
    }
}
