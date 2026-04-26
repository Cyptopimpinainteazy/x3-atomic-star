//! Integration tests for readiness report infrastructure

#[cfg(test)]
mod tests {
    use crate::{Collector, JsonFormatter, ReadinessReport, TextFormatter};

    #[test]
    fn test_collector_creates_report() {
        let report = Collector::collect();
        assert_eq!(report.version, "0.4.0");
        assert!(!report.timestamp.is_empty());
        assert!(report.timestamp.len() > 10); // ISO 8601 format
    }

    #[test]
    fn test_readiness_report_creation() {
        let report = ReadinessReport::new();
        assert_eq!(report.version, "0.4.0");
        assert_eq!(report.kernel_status.supply, 0);
        assert_eq!(report.kernel_status.account_count, 0);
        assert!(!report.kernel_status.halted);
    }

    #[test]
    fn test_readiness_percentage_calculation() {
        let mut report = ReadinessReport::new();
        assert_eq!(report.readiness_percentage(), 0); // No checks passed

        report.supply_invariant = true;
        assert_eq!(report.readiness_percentage(), 25); // 1/4

        report.halt_functional = true;
        assert_eq!(report.readiness_percentage(), 50); // 2/4

        report.permissions_enforced = true;
        assert_eq!(report.readiness_percentage(), 75); // 3/4

        report.balance_reconciliation = true;
        assert_eq!(report.readiness_percentage(), 100); // 4/4
    }

    #[test]
    fn test_readiness_flag_logic() {
        let mut report = ReadinessReport::new();
        assert!(!report.is_ready()); // All false

        report.mark_ready();
        assert!(report.is_ready()); // All true
        assert_eq!(report.readiness_percentage(), 100);
    }

    #[test]
    fn test_text_formatter_output() {
        let mut report = ReadinessReport::new();
        report.mark_ready();

        let text = TextFormatter::format(&report);
        assert!(text.contains("X3 ATOMIC STAR V0.4 READINESS REPORT"));
        assert!(text.contains("READINESS CHECKLIST"));
        assert!(text.contains("✓")); // Checkmarks
        assert!(text.contains("🟢")); // Ready indicator
        assert!(text.contains("100%")); // Readiness percentage
    }

    #[test]
    fn test_text_formatter_not_ready() {
        let report = ReadinessReport::new();
        let text = TextFormatter::format(&report);
        assert!(text.contains("🔴")); // Not ready indicator
        assert!(text.contains("0%")); // Zero readiness
        assert!(text.contains("✗")); // X marks
    }

    #[test]
    fn test_text_formatter_compact() {
        let mut report = ReadinessReport::new();
        report.mark_ready();

        let compact = TextFormatter::format_compact(&report);
        assert!(compact.contains("0.4.0"));
        assert!(compact.contains("Ready: YES"));
    }

    #[test]
    fn test_json_formatter_output() {
        let mut report = ReadinessReport::new();
        report.mark_ready();

        let json_str = JsonFormatter::format(&report);
        assert!(json_str.contains("version"));
        assert!(json_str.contains("0.4.0"));
        assert!(json_str.contains("timestamp"));
        assert!(json_str.contains("kernel_status"));
        assert!(json_str.contains("supply_invariant"));
        assert!(json_str.contains("overall_ready"));

        // Verify it's valid JSON
        let parsed: serde_json::Result<serde_json::Value> = serde_json::from_str(&json_str);
        assert!(parsed.is_ok(), "JSON formatter should produce valid JSON");
    }

    #[test]
    fn test_json_formatter_compact() {
        let report = ReadinessReport::new();
        let compact = JsonFormatter::format_compact(&report);
        assert!(!compact.contains("\n")); // No newlines in compact format
        assert!(compact.contains("version"));

        // Verify it's still valid JSON
        let parsed: serde_json::Result<serde_json::Value> = serde_json::from_str(&compact);
        assert!(parsed.is_ok(), "Compact JSON should be valid");
    }

    #[test]
    fn test_collector_supply_methods() {
        let supply = Collector::collect_supply();
        assert_eq!(supply, 0); // Placeholder returns 0

        let account_count = Collector::collect_account_count();
        assert_eq!(account_count, 0); // Placeholder returns 0

        let halted = Collector::is_halted();
        assert!(!halted); // Placeholder returns false
    }

    #[test]
    fn test_report_serialization_roundtrip() {
        let mut original = ReadinessReport::new();
        original.mark_ready();
        original.kernel_status.supply = 1_000_000_000;
        original.kernel_status.account_count = 42;

        // Serialize to JSON
        let json_str = JsonFormatter::format(&original);

        // Deserialize back
        let deserialized: ReadinessReport =
            serde_json::from_str(&json_str).expect("Should deserialize successfully");

        // Verify fields match
        assert_eq!(deserialized.version, original.version);
        assert_eq!(
            deserialized.kernel_status.supply,
            original.kernel_status.supply
        );
        assert_eq!(
            deserialized.kernel_status.account_count,
            original.kernel_status.account_count
        );
        assert_eq!(deserialized.supply_invariant, original.supply_invariant);
        assert_eq!(deserialized.overall_ready, original.overall_ready);
    }
}
