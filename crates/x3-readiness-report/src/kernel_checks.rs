//! Kernel readiness checks used by the readiness collector.

use crate::types::{KernelStatus, ReadinessCheck};

/// Collect high-signal kernel health from the runtime snapshot.
pub fn collect_kernel_health(status: &KernelStatus) -> ReadinessCheck {
    match (status.supply, status.halted, status.total_locked) {
        (Some(supply), Some(halted), Some(locked)) if supply > 0 && locked <= supply => {
            ReadinessCheck::pass(format!(
                "kernel snapshot valid: supply={}, locked={}, halted={}",
                supply, locked, halted
            ))
        }
        (Some(supply), Some(_), Some(locked)) if locked > supply => ReadinessCheck::fail(format!(
            "kernel snapshot invalid: locked={} exceeds supply={}",
            locked, supply
        )),
        _ => ReadinessCheck::unknown("kernel snapshot incomplete: missing supply/halted/locked"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_health_passes_with_complete_snapshot() {
        let status = KernelStatus {
            supply: Some(1_000),
            total_locked: Some(300),
            halted: Some(false),
            account_count: Some(4),
        };
        let check = collect_kernel_health(&status);
        assert!(check.is_pass());
    }

    #[test]
    fn kernel_health_fails_when_locked_exceeds_supply() {
        let status = KernelStatus {
            supply: Some(100),
            total_locked: Some(101),
            halted: Some(false),
            account_count: Some(2),
        };
        let check = collect_kernel_health(&status);
        assert_eq!(check.status, crate::types::CheckStatus::Fail);
    }
}
