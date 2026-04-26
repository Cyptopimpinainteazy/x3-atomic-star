//! Report formatters for different output styles

use crate::types::ReadinessReport;

/// Text formatter produces human-readable reports
pub struct TextFormatter;

impl TextFormatter {
    /// Format report as human-readable text with Unicode box drawing
    pub fn format(report: &ReadinessReport) -> String {
        let ready_status = if report.overall_ready {
            "🟢 READY FOR PRODUCTION"
        } else {
            "🔴 NOT READY"
        };

        let readiness_pct = report.readiness_percentage();

        format!(
            r#"
╔══════════════════════════════════════════════════════╗
║        X3 ATOMIC STAR V0.4 READINESS REPORT         ║
╚══════════════════════════════════════════════════════╝

📅 Timestamp:         {}
🔧 Version:           {}
⏱️  Generated at:       {}/100%

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

🔍 KERNEL STATUS
  • Total Supply:       {} units
  • Active Accounts:    {}
  • System Halted:      {}
  • Total Locked:       {} units

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✅ READINESS CHECKLIST
  [{}] Supply Invariant Maintained
  [{}] Emergency Halt / Pause Working
  [{}] Permissions Properly Enforced
  [{}] Balance Reconciliation Verified

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

🎯 OVERALL ASSESSMENT

  Status:               {}
  Readiness Score:      {}%
  
  Ready for Phase 1:    {}

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

"#,
            report.timestamp,
            report.version,
            readiness_pct,
            report.kernel_status.supply,
            report.kernel_status.account_count,
            if report.kernel_status.halted { "YES" } else { "NO" },
            report.kernel_status.total_locked,
            if report.supply_invariant { "✓" } else { "✗" },
            if report.halt_functional { "✓" } else { "✗" },
            if report.permissions_enforced { "✓" } else { "✗" },
            if report.balance_reconciliation { "✓" } else { "✗" },
            ready_status,
            readiness_pct,
            if report.overall_ready { "✓ YES" } else { "✗ NO" },
        )
    }

    /// Format report as compact single-line text
    pub fn format_compact(report: &ReadinessReport) -> String {
        format!(
            "X3 v0.4 Readiness: {} | Supply: {} | Halted: {} | Ready: {}",
            report.version,
            report.kernel_status.supply,
            report.kernel_status.halted,
            if report.overall_ready { "YES" } else { "NO" }
        )
    }
}

/// JSON formatter produces machine-readable reports
pub struct JsonFormatter;

impl JsonFormatter {
    /// Format report as pretty-printed JSON
    pub fn format(report: &ReadinessReport) -> String {
        serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
    }

    /// Format report as compact JSON (single line)
    pub fn format_compact(report: &ReadinessReport) -> String {
        serde_json::to_string(report).unwrap_or_else(|_| "{}".to_string())
    }
}
