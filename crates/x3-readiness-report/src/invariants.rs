//! Critical invariant checks.

use crate::types::{KernelStatus, ReadinessCheck};

/// Check critical runtime invariants that can be inferred from kernel snapshot.
pub fn check_critical_invariants(status: &KernelStatus) -> ReadinessCheck {
    match (status.supply, status.total_locked) {
        (Some(supply), Some(locked)) if locked <= supply => ReadinessCheck::pass(format!(
            "critical invariant holds: total_locked={} <= supply={}",
            locked, supply
        )),
        (Some(supply), Some(locked)) => ReadinessCheck::fail(format!(
            "critical invariant broken: total_locked={} > supply={}",
            locked, supply
        )),
        _ => ReadinessCheck::unknown("cannot verify critical invariant: missing supply/locked"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invariant_check_fails_when_locked_exceeds_supply() {
        let status = KernelStatus {
            supply: Some(10),
            total_locked: Some(11),
            ..KernelStatus::default()
        };
        let check = check_critical_invariants(&status);
        assert_eq!(check.status, crate::types::CheckStatus::Fail);
    }
}
