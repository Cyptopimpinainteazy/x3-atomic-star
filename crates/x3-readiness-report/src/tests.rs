//! Integration tests for readiness reporting.
//!
//! These tests verify the *contract* of the readiness pipeline:
//!   * the offline collector never claims readiness;
//!   * Unknown checks block overall_ready;
//!   * formatters round-trip JSON.
//!
//! Live RPC tests live in collector.rs as unit tests.

#[cfg(test)]
mod tests {
    use crate::types::{CheckStatus, ReadinessCheck, ReadinessReport};
    use crate::{Collector, JsonFormatter, TextFormatter};

    #[test]
    fn collector_offline_emits_unknown_only() {
        let r = Collector::collect_offline();
        assert!(!r.overall_ready);
        assert_eq!(r.supply_invariant.status, CheckStatus::Unknown);
        assert_eq!(r.balance_reconciliation.status, CheckStatus::Unknown);
        assert!(r.kernel_status.supply.is_none());
        assert!(r.kernel_status.halted.is_none());
    }

    #[test]
    fn unknown_blocks_ready_even_if_others_pass() {
        let mut r = ReadinessReport::new();
        r.supply_invariant = ReadinessCheck::pass("ok");
        r.halt_functional = ReadinessCheck::pass("ok");
        r.permissions_enforced = ReadinessCheck::pass("ok");
        r.balance_reconciliation = ReadinessCheck::unknown("not wired");
        r.recompute_overall();
        assert!(!r.is_ready());
        assert_eq!(r.readiness_percentage(), 75);
    }

    #[test]
    fn fail_blocks_ready() {
        let mut r = ReadinessReport::new();
        r.supply_invariant = ReadinessCheck::fail("locked > supply");
        r.halt_functional = ReadinessCheck::pass("ok");
        r.permissions_enforced = ReadinessCheck::pass("ok");
        r.balance_reconciliation = ReadinessCheck::pass("ok");
        r.recompute_overall();
        assert!(!r.is_ready());
    }

    #[test]
    fn json_roundtrip_preserves_check_state() {
        let mut original = ReadinessReport::new();
        original.supply_invariant = ReadinessCheck::pass("supply=10, locked=5");
        original.kernel_status.supply = Some(10);
        original.kernel_status.account_count = Some(42);
        original.recompute_overall();

        let json = JsonFormatter::format(&original);
        let back: ReadinessReport = serde_json::from_str(&json).expect("valid json");
        assert_eq!(back.supply_invariant.status, CheckStatus::Pass);
        assert_eq!(back.supply_invariant.reason, "supply=10, locked=5");
        assert_eq!(back.kernel_status.supply, Some(10));
        assert_eq!(back.kernel_status.account_count, Some(42));
    }

    #[test]
    fn text_formatter_marks_unknown_distinctly() {
        let mut r = ReadinessReport::new();
        r.supply_invariant = ReadinessCheck::pass("ok");
        r.halt_functional = ReadinessCheck::fail("bad");
        // permissions + balance left as default (Unknown)
        r.recompute_overall();
        let t = TextFormatter::format(&r);
        assert!(t.contains("PASS"), "must show PASS marker:\n{}", t);
        assert!(t.contains("FAIL"));
        assert!(t.contains("UNK"));
        assert!(t.contains("NOT READY"));
    }
}
