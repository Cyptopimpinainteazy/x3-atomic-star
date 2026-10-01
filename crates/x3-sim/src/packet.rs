//! Failure packets: everything a root-cause agent needs, without the search.
//!
//! A bare `FAIL` line costs an agent a long investigation before it can even
//! start. A packet pins the exact run (seed, scenario, config, digests), the
//! first step whose execution broke an invariant, the state immediately before
//! and after that step, the faults that were active, and the coordinator
//! symbols the violated invariant implicates. It is the difference between
//! "something is wrong" and "work here".
//!
//! The packet is derived, never invented: every field comes from the outcome
//! of a real run of the real coordinator. When a field cannot be known (the
//! commit, for instance, outside a checkout) it says so instead of guessing.

use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::sim::{SimConfig, SimOutcome};

/// Schema id so a consumer can refuse a packet it does not understand.
pub const PACKET_SCHEMA: &str = "x3-failure-packet-v1";

/// One place in the coordinator the violated invariant points at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuspectedSymbol {
    pub file: String,
    pub symbol: String,
    pub reason: String,
}

/// The minimized run that still reproduces the violation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinimizedReproducer {
    pub sessions: usize,
    pub steps: usize,
    pub nodes: usize,
    pub scenario: String,
    /// Runs the minimizer spent, including the final verification run.
    pub minimizer_runs: usize,
    /// True when the minimized config was re-run and failed again.
    pub verified: bool,
    pub replay_command: String,
}

/// The config, in the shape a consumer can re-enter into the CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacketConfig {
    pub sessions: usize,
    pub steps: usize,
    pub nodes: usize,
}

/// What one run broke, and where it first broke it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacketViolation {
    pub code: String,
    pub session: String,
    pub detail: String,
}

/// A structured failure report for one failing simulation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePacket {
    pub schema: String,
    /// Stable identity: the same failure always hashes to the same id, so a
    /// re-run of the same seed files one packet, not a second one.
    pub failure_id: String,
    pub producer: String,
    /// `X3_COMMIT` when the caller provides it, otherwise `"unknown"`.
    pub commit: String,
    pub seed: u64,
    pub scenario: String,
    pub config: PacketConfig,
    pub invariant: String,
    pub session: String,
    pub detail: String,
    /// Scheduler step whose execution introduced the first violation, or
    /// `None` when it appeared during the drain (after the last step).
    pub first_bad_step: Option<u64>,
    /// `"step N"` or `"drain"`, for humans.
    pub first_bad_step_label: String,
    pub first_bad_op: Option<String>,
    pub active_faults: Vec<String>,
    pub state_before: Option<serde_json::Value>,
    pub state_after: Option<serde_json::Value>,
    pub all_violations: Vec<PacketViolation>,
    pub suspected_code: Vec<SuspectedSymbol>,
    pub trace_digest: String,
    pub state_digest: String,
    pub replay_command: String,
    pub minimized: Option<MinimizedReproducer>,
    pub required_regression_test: String,
}

/// The coordinator symbols each invariant implicates, with the reason.
///
/// These are pointers for an investigator, not verdicts: the simulator reports
/// violated properties, and these are the code paths that own those properties.
fn suspected_symbols(code: &str) -> Vec<SuspectedSymbol> {
    let symbol = |file: &str, name: &str, reason: &str| SuspectedSymbol {
        file: file.to_string(),
        symbol: name.to_string(),
        reason: reason.to_string(),
    };
    let state_machine = "crates/cross-vm-coordinator/src/state_machine.rs";
    match code {
        "CLAIM_REFUND_MIX" | "REFUNDED_WITH_A_CLAIM" => vec![
            symbol(
                state_machine,
                "SwapCoordinator::record_refunds",
                "the refund path must refuse any session whose leg was claimed",
            ),
            symbol(
                state_machine,
                "SwapCoordinator::record_fast_claim",
                "the claim path must terminalize the session before a refund can run",
            ),
        ],
        "REFUND_AFTER_CLAIM" => vec![
            symbol(
                state_machine,
                "SwapCoordinator::abort",
                "abort() moves the session towards refundable; it must honour the terminal-phase table",
            ),
            symbol(
                state_machine,
                "SwapCoordinator::validate_phase_transition",
                "the phase table is the single gate every mutator must pass through",
            ),
        ],
        "DOUBLE_SETTLE" => vec![symbol(
            state_machine,
            "SwapCoordinator::begin_settlement",
            "settlement must be single-shot per session; a second call is a double spend",
        )],
        "COMPLETE_WITHOUT_BOTH_CLAIMS" => vec![
            symbol(
                state_machine,
                "SwapCoordinator::begin_settlement",
                "phase must not advance to Complete before both legs are claimed",
            ),
            symbol(
                state_machine,
                "SwapCoordinator::record_slow_claim",
                "the slow leg's claim is what closes the lifecycle",
            ),
        ],
        "DUPLICATE_JOURNAL_ENTRY" => vec![symbol(
            state_machine,
            "SwapCoordinator::record_operation",
            "the operation journal must collapse duplicate deliveries; a duplicate entry means the idempotency check and the journal write disagree",
        )],
        "PHASE_WITHOUT_FAST_HTLC" => vec![
            symbol(
                state_machine,
                "SwapCoordinator::validate_phase_transition",
                "a phase that requires the fast HTLC must be refused while it is absent",
            ),
            symbol(
                state_machine,
                "SwapCoordinator::record_htlc_fast",
                "the fast HTLC record is what makes those phases reachable at all",
            ),
        ],
        "TIMELOCK_ORDER_INVERTED" => vec![
            symbol(
                state_machine,
                "SwapCoordinator::record_htlc_fast",
                "the fast-chain timelock must be checked against the slow-chain one",
            ),
            symbol(
                state_machine,
                "SwapCoordinator::record_htlc_slow",
                "the slow-chain timelock must exceed the fast one or the refund race is winnable",
            ),
        ],
        _ => Vec::new(),
    }
}

/// The regression test the fix must land with.
fn required_regression_test(code: &str, failure_id: &str) -> String {
    format!(
        "tests/regression_{}.rs — reproduces {} (failure {}), asserts it stays fixed",
        code.to_lowercase(),
        code,
        failure_id
    )
}

impl FailurePacket {
    /// Build a packet from a failing outcome, or `None` when nothing failed.
    ///
    /// The before/after states come from the run itself: the run loop captures
    /// the violating session around the first bad step.
    pub fn from_outcome(config: &SimConfig, outcome: &SimOutcome) -> Option<Self> {
        let first = outcome.violations.first()?.clone();
        let code = first.code.to_string();
        let violations: Vec<PacketViolation> = outcome
            .violations
            .iter()
            .map(|violation| PacketViolation {
                code: violation.code.to_string(),
                session: violation.session_id.clone(),
                detail: violation.detail.clone(),
            })
            .collect();

        // Identity: what was run and what broke. Deliberately excludes the
        // trace, so a packet is stable across re-runs of the same failure.
        let mut identity = String::new();
        let _ = write!(
            identity,
            "{}|{}|{}|{}|{}|{}",
            config.seed,
            outcome.scenario,
            config.sessions,
            config.steps,
            config.nodes,
            violations
                .iter()
                .map(|violation| format!("{}:{}:{}", violation.code, violation.session, violation.detail))
                .collect::<Vec<_>>()
                .join("+")
        );
        let failure_id = blake3::hash(identity.as_bytes()).to_hex().to_string()[..16].to_string();

        Some(Self {
            schema: PACKET_SCHEMA.to_string(),
            failure_id: failure_id.clone(),
            producer: "x3-sim".to_string(),
            commit: std::env::var("X3_COMMIT").unwrap_or_else(|_| "unknown".to_string()),
            seed: outcome.seed,
            scenario: outcome.scenario.clone(),
            config: PacketConfig {
                sessions: config.sessions,
                steps: config.steps,
                nodes: config.nodes,
            },
            invariant: code.clone(),
            session: first.session_id.clone(),
            detail: first.detail.clone(),
            first_bad_step: outcome.first_bad_step,
            first_bad_step_label: outcome.first_bad_step_label.clone(),
            first_bad_op: outcome.first_bad_op.clone(),
            active_faults: outcome.active_faults.clone(),
            state_before: outcome.state_before.clone(),
            state_after: outcome.state_after.clone(),
            all_violations: violations,
            suspected_code: suspected_symbols(&code),
            trace_digest: outcome.trace_digest.clone(),
            state_digest: outcome.state_digest.clone(),
            replay_command: outcome.replay_command(),
            minimized: None,
            required_regression_test: required_regression_test(&code, &failure_id),
        })
    }

    /// Attach a minimized reproducer, after verifying it still fails.
    pub fn attach_minimized(&mut self, minimized: MinimizedReproducer) {
        self.minimized = Some(minimized);
    }

    /// The file stem, for evidence bundles.
    pub fn file_stem(&self) -> String {
        format!("x3-failure-{}-{}", self.failure_id, self.invariant.to_lowercase())
    }

    /// Human-readable packet: the same facts, in the order an investigator
    /// needs them.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# X3 failure packet {}", self.failure_id);
        let _ = writeln!(out);
        let _ = writeln!(out, "- invariant: `{}`", self.invariant);
        let _ = writeln!(out, "- session: `{}`", self.session);
        let _ = writeln!(out, "- detail: {}", self.detail);
        let _ = writeln!(out, "- seed: `{}`", self.seed);
        let _ = writeln!(out, "- scenario: `{}`", self.scenario);
        let _ = writeln!(
            out,
            "- config: sessions={} steps={} nodes={}",
            self.config.sessions, self.config.steps, self.config.nodes
        );
        let _ = writeln!(
            out,
            "- first bad step: {} {}",
            self.first_bad_step_label,
            self.first_bad_op
                .as_deref()
                .map(|op| format!("(op `{op}`)"))
                .unwrap_or_default()
        );
        let _ = writeln!(out, "- commit: `{}`", self.commit);
        let _ = writeln!(out, "- trace digest: `{}`", self.trace_digest);
        let _ = writeln!(out, "- state digest: `{}`", self.state_digest);
        if !self.active_faults.is_empty() {
            let _ = writeln!(out, "\n## Active faults\n");
            for fault in &self.active_faults {
                let _ = writeln!(out, "- {fault}");
            }
        }
        if let Some(before) = &self.state_before {
            let _ = writeln!(out, "\n## State before the bad step\n");
            let _ = writeln!(out, "```json\n{}\n```", serde_json::to_string_pretty(before).unwrap_or_default());
        }
        if let Some(after) = &self.state_after {
            let _ = writeln!(out, "\n## State after the bad step\n");
            let _ = writeln!(out, "```json\n{}\n```", serde_json::to_string_pretty(after).unwrap_or_default());
        }
        let _ = writeln!(out, "\n## Suspected code\n");
        if self.suspected_code.is_empty() {
            let _ = writeln!(out, "No registered mapping for `{}`; start from the invariant and the state diff.", self.invariant);
        }
        for suspect in &self.suspected_code {
            let _ = writeln!(out, "- `{}` in `{}` — {}", suspect.symbol, suspect.file, suspect.reason);
        }
        let _ = writeln!(out, "\n## Repro\n");
        let _ = writeln!(out, "```bash\n{}\n```", self.replay_command);
        if let Some(minimized) = &self.minimized {
            let _ = writeln!(out, "\n## Minimized repro (verified: {})\n", minimized.verified);
            let _ = writeln!(out, "```bash\n{}\n```", minimized.replay_command);
        }
        let _ = writeln!(out, "\n## Required regression test\n");
        let _ = writeln!(out, "{}", self.required_regression_test);
        out
    }

    /// Write `<stem>.json` and `<stem>.md` into `dir`, creating it if needed.
    pub fn write(&self, dir: &Path) -> io::Result<(PathBuf, PathBuf)> {
        std::fs::create_dir_all(dir)?;
        let stem = self.file_stem();
        let json_path = dir.join(format!("{stem}.json"));
        let md_path = dir.join(format!("{stem}.md"));
        std::fs::write(
            &json_path,
            serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string()),
        )?;
        std::fs::write(&md_path, self.to_markdown())?;
        Ok((json_path, md_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariants::Violation;
    use crate::network::NetworkStats;

    fn outcome(seed: u64, code: &'static str) -> SimOutcome {
        SimOutcome {
            seed,
            scenario: "claim-refund-race".to_string(),
            steps: 40,
            accepted: 10,
            rejected: 2,
            restarts: 0,
            stale_writes: 0,
            partitions: 1,
            sessions_completed: 0,
            sessions_refunded: 1,
            network: NetworkStats::default(),
            trace_digest: "aa".to_string(),
            state_digest: "bb".to_string(),
            first_bad_step: Some(7),
            first_bad_step_label: "step 7".to_string(),
            first_bad_op: Some("refund".to_string()),
            active_faults: vec!["0007 partition [0]|[1, 2, 3]".to_string()],
            state_before: None,
            state_after: None,
            violations: vec![Violation {
                code,
                session_id: "sim-0".to_string(),
                detail: "phase=Refunded but fast=Some(Claimed)".to_string(),
            }],
            trace: vec!["0007 INVARIANT".to_string()],
        }
    }

    #[test]
    fn packet_identity_is_stable_and_config_sensitive() {
        let config = SimConfig { seed: 41, sessions: 4, steps: 40, nodes: 4, ..SimConfig::default() };
        let first = FailurePacket::from_outcome(&config, &outcome(41, "CLAIM_REFUND_MIX"))
            .expect("a failing outcome produces a packet");
        let second = FailurePacket::from_outcome(&config, &outcome(41, "CLAIM_REFUND_MIX"))
            .expect("packet");
        assert_eq!(first.failure_id, second.failure_id);

        let smaller = SimConfig { seed: 41, sessions: 2, steps: 40, nodes: 4, ..SimConfig::default() };
        let third = FailurePacket::from_outcome(&smaller, &outcome(41, "CLAIM_REFUND_MIX"))
            .expect("packet");
        assert_ne!(first.failure_id, third.failure_id, "a different config is a different failure");
    }

    #[test]
    fn a_passing_outcome_has_no_packet() {
        let mut clean = outcome(3, "CLAIM_REFUND_MIX");
        clean.violations.clear();
        assert!(FailurePacket::from_outcome(&SimConfig::default(), &clean).is_none());
    }

    #[test]
    fn known_invariants_name_real_symbols_and_knowledge_gaps_do_not() {
        let with_symbols = suspected_symbols("REFUND_AFTER_CLAIM");
        assert!(with_symbols.iter().any(|s| s.symbol.contains("abort")));
        assert!(with_symbols.iter().any(|s| s.file.contains("state_machine.rs")));

        assert!(suspected_symbols("NOT_A_REAL_INVARIANT").is_empty());
    }

    #[test]
    fn markdown_carries_replay_and_minimized_repro() {
        let config = SimConfig::default();
        let mut packet =
            FailurePacket::from_outcome(&config, &outcome(41, "REFUND_AFTER_CLAIM"))
                .expect("packet");
        packet.attach_minimized(MinimizedReproducer {
            sessions: 1,
            steps: 11,
            nodes: 3,
            scenario: "claim-refund-race".to_string(),
            minimizer_runs: 9,
            verified: true,
            replay_command: "cargo run -p x3-sim -- --seed 41 --scenario claim-refund-race --sessions 1 --steps 11 --nodes 3".to_string(),
        });
        let markdown = packet.to_markdown();
        assert!(markdown.contains("REFUND_AFTER_CLAIM"));
        assert!(markdown.contains("--seed 41"));
        assert!(markdown.contains("Minimized repro (verified: true)"));
        assert!(markdown.contains("Required regression test"));
    }
}
