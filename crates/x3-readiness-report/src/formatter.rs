//! Report formatters.

use crate::types::{CheckStatus, ReadinessCheck, ReadinessReport};

pub struct TextFormatter;

impl TextFormatter {
    fn mark(c: &ReadinessCheck) -> &'static str {
        match c.status {
            CheckStatus::Pass => "PASS",
            CheckStatus::Fail => "FAIL",
            CheckStatus::Unknown => "UNK ",
        }
    }

    fn opt_u128(v: Option<u128>) -> String {
        v.map(|n| n.to_string()).unwrap_or_else(|| "?".into())
    }

    fn opt_usize(v: Option<usize>) -> String {
        v.map(|n| n.to_string()).unwrap_or_else(|| "?".into())
    }

    fn opt_bool(v: Option<bool>) -> &'static str {
        match v {
            Some(true) => "YES",
            Some(false) => "NO",
            None => "?",
        }
    }

    pub fn format(report: &ReadinessReport) -> String {
        let header = if report.overall_ready {
            "READY"
        } else {
            "NOT READY"
        };
        format!(
            "X3 Atomic Star v0.4 Readiness Report
=====================================
timestamp:    {ts}
version:      {ver}
score:        {pct}%
status:       {hdr}

Kernel snapshot
  supply:         {supply}
  account_count:  {accounts}
  halted:         {halted}
  total_locked:   {locked}

Checks
  [{m_si}] supply_invariant         — {r_si}
  [{m_hf}] halt_functional          — {r_hf}
  [{m_pe}] permissions_enforced     — {r_pe}
  [{m_br}] balance_reconciliation   — {r_br}
",
            ts = report.timestamp,
            ver = report.version,
            pct = report.readiness_percentage(),
            hdr = header,
            supply = Self::opt_u128(report.kernel_status.supply),
            accounts = Self::opt_usize(report.kernel_status.account_count),
            halted = Self::opt_bool(report.kernel_status.halted),
            locked = Self::opt_u128(report.kernel_status.total_locked),
            m_si = Self::mark(&report.supply_invariant),
            r_si = report.supply_invariant.reason,
            m_hf = Self::mark(&report.halt_functional),
            r_hf = report.halt_functional.reason,
            m_pe = Self::mark(&report.permissions_enforced),
            r_pe = report.permissions_enforced.reason,
            m_br = Self::mark(&report.balance_reconciliation),
            r_br = report.balance_reconciliation.reason,
        )
    }

    pub fn format_compact(report: &ReadinessReport) -> String {
        format!(
            "X3 v{} readiness={}% ready={} supply={} halted={}",
            report.version,
            report.readiness_percentage(),
            report.overall_ready,
            TextFormatter::opt_u128(report.kernel_status.supply),
            TextFormatter::opt_bool(report.kernel_status.halted),
        )
    }
}

pub struct JsonFormatter;

impl JsonFormatter {
    pub fn format(report: &ReadinessReport) -> String {
        serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
    }
    pub fn format_compact(report: &ReadinessReport) -> String {
        serde_json::to_string(report).unwrap_or_else(|_| "{}".to_string())
    }
}
