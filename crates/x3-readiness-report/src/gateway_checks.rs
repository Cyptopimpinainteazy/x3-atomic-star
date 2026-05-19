//! Gateway readiness checks.

use crate::types::ReadinessCheck;

/// Aggregate gateway readiness from packet lifecycle and bridge-scope checks.
pub fn collect_gateway_health(
    packet_lifecycle: &ReadinessCheck,
    external_bridges_disabled: &ReadinessCheck,
) -> ReadinessCheck {
    if !packet_lifecycle.is_pass() {
        return ReadinessCheck::fail(format!(
            "packet lifecycle gate failing: {}",
            packet_lifecycle.reason
        ));
    }

    if !external_bridges_disabled.is_pass() {
        return ReadinessCheck::fail(format!(
            "scope-freeze gate failing: {}",
            external_bridges_disabled.reason
        ));
    }

    ReadinessCheck::pass("gateway checks healthy: packet lifecycle live and external bridges disabled")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_health_fails_if_packet_gate_fails() {
        let packet = ReadinessCheck::fail("replay guard unavailable");
        let bridges = ReadinessCheck::pass("bridges disabled");
        let check = collect_gateway_health(&packet, &bridges);
        assert_eq!(check.status, crate::types::CheckStatus::Fail);
    }

    #[test]
    fn gateway_health_passes_when_both_prereqs_pass() {
        let packet = ReadinessCheck::pass("packet lifecycle live");
        let bridges = ReadinessCheck::pass("bridges disabled");
        let check = collect_gateway_health(&packet, &bridges);
        assert!(check.is_pass());
    }
}
