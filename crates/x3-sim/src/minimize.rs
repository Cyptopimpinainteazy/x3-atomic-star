//! Automatic failure minimization.
//!
//! A failing run of 200 sessions and 5,000 steps is a mystery; the same
//! violation in 1 session and 9 steps is a bug report. The minimizer shrinks
//! the *config* one dimension at a time — sessions, steps, nodes — keeping a
//! candidate only when re-running it reproduces the same violation, and
//! verifying the smallest candidate once more before returning it.
//!
//! Everything here is deterministic: the same starting config and predicate
//! always produce the same minimized config, because the same seeds are tried
//! in the same order.

use serde::Serialize;

use crate::packet::MinimizedReproducer;
use crate::sim::{SimConfig, SimOutcome};

/// Default ceiling on re-runs. A bounded search that stops and reports is
/// worth more than an unbounded one that never returns.
pub const DEFAULT_MAX_RUNS: usize = 160;

/// What the minimizer settled on.
#[derive(Debug, Clone, Serialize)]
pub struct Minimized {
    pub config: SimConfig,
    pub outcome: SimOutcome,
    pub runs: usize,
    /// True when the minimized config was re-run one final time and the
    /// violation fired again. False means the run budget ran out first.
    pub verified: bool,
}

impl Minimized {
    /// The same facts in the shape a packet carries.
    pub fn to_reproducer(&self) -> MinimizedReproducer {
        MinimizedReproducer {
            sessions: self.config.sessions,
            steps: self.config.steps,
            nodes: self.config.nodes,
            scenario: self.config.scenario.as_str().to_string(),
            minimizer_runs: self.runs,
            verified: self.verified,
            replay_command: replay_command(&self.config),
        }
    }
}

/// The `cargo run` line for a config, including every dimension the CLI reads.
pub fn replay_command(config: &SimConfig) -> String {
    format!(
        "cargo run -p x3-sim -- --seed {} --scenario {} --sessions {} --steps {} --nodes {}",
        config.seed,
        config.scenario.as_str(),
        config.sessions,
        config.steps,
        config.nodes
    )
}

/// Shrink `start` until no dimension can be removed and the same violation
/// still fires.
///
/// `target` names the violation code that must survive minimization; `None`
/// accepts any violation. Returns `None` when `start` does not reproduce at
/// all — the minimizer never manufactures a reproducer that the predicate
/// did not produce.
pub fn minimize<F>(
    start: &SimConfig,
    target: Option<&str>,
    max_runs: usize,
    mut runner: F,
) -> Option<Minimized>
where
    F: FnMut(&SimConfig) -> SimOutcome,
{
    let mut runs = 0usize;
    let reproduce = |config: &SimConfig, runner: &mut F, runs: &mut usize| {
        if *runs >= max_runs {
            return None;
        }
        *runs += 1;
        let outcome = runner(config);
        if outcome.is_pass() {
            return None;
        }
        if let Some(code) = target {
            if !outcome.violations.iter().any(|violation| violation.code == code) {
                return None;
            }
        }
        Some(outcome)
    };

    let mut current = start.clone();
    let first = reproduce(&current, &mut runner, &mut runs)?;
    let mut best: Option<(SimConfig, SimOutcome)> = Some((current.clone(), first));

    // Two passes: shrinking sessions and nodes changes how many steps the
    // schedule needs, so a dimension that could not shrink on the first pass
    // may shrink on the second.
    for _pass in 0..2 {
        for dimension in [Dimension::Sessions, Dimension::Steps, Dimension::Nodes] {
            let floor = dimension.floor();
            let ceiling = dimension.get(&current);
            if ceiling <= floor {
                continue;
            }
            // Binary search for the smallest reproducing value. The search
            // assumes a failure that survives at a larger value usually
            // survives near it; the verification run at the end is what
            // makes the result trustworthy either way.
            let mut low = floor;
            let mut high = ceiling;
            while low < high {
                let mid = low + (high - low) / 2;
                let mut candidate = current.clone();
                dimension.set(&mut candidate, mid);
                match reproduce(&candidate, &mut runner, &mut runs) {
                    Some(outcome) => {
                        high = mid;
                        best = Some((candidate, outcome));
                    }
                    None => low = mid + 1,
                }
            }
            if let Some((config, _)) = &best {
                current = config.clone();
            }
        }
    }

    let (config, outcome) = best?;
    let mut result = Minimized {
        config,
        outcome,
        runs,
        verified: false,
    };
    // Final verification: re-run the minimized config once more and confirm
    // the violation is really there. Skipped — and reported as unverified —
    // when the budget is already spent.
    if result.runs < max_runs {
        let candidate = result.config.clone();
        if let Some(outcome) = reproduce(&candidate, &mut runner, &mut runs) {
            result.outcome = outcome;
            result.verified = true;
        }
    }
    result.runs = runs;
    Some(result)
}

#[derive(Debug, Clone, Copy)]
enum Dimension {
    Sessions,
    Steps,
    Nodes,
}

impl Dimension {
    fn floor(self) -> usize {
        match self {
            // The coordinator needs at least one session to violate anything
            // about a session, and at least one step to apply one operation.
            Dimension::Sessions => 1,
            Dimension::Steps => 1,
            Dimension::Nodes => 2,
        }
    }

    fn get(self, config: &SimConfig) -> usize {
        match self {
            Dimension::Sessions => config.sessions,
            Dimension::Steps => config.steps,
            Dimension::Nodes => config.nodes,
        }
    }

    fn set(self, config: &mut SimConfig, value: usize) {
        match self {
            Dimension::Sessions => config.sessions = value,
            Dimension::Steps => config.steps = value,
            Dimension::Nodes => config.nodes = value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariants::Violation;
    use crate::network::NetworkStats;
    use crate::sim::Scenario;

    fn outcome(config: &SimConfig, code: &'static str) -> SimOutcome {
        SimOutcome {
            seed: config.seed,
            scenario: config.scenario.as_str().to_string(),
            steps: config.steps,
            accepted: 1,
            rejected: 0,
            restarts: 0,
            stale_writes: 0,
            partitions: 0,
            sessions_completed: 0,
            sessions_refunded: 0,
            network: NetworkStats::default(),
            trace_digest: "00".to_string(),
            state_digest: "00".to_string(),
            first_bad_step: Some(0),
            first_bad_step_label: "step 0".to_string(),
            first_bad_op: Some("refund".to_string()),
            active_faults: Vec::new(),
            state_before: None,
            state_after: None,
            violations: vec![Violation {
                code,
                session_id: "s".to_string(),
                detail: "detail".to_string(),
            }],
            trace: Vec::new(),
        }
    }

    fn clean(config: &SimConfig) -> SimOutcome {
        let mut out = outcome(config, "CLAIM_REFUND_MIX");
        out.violations.clear();
        out
    }

    #[test]
    fn minimization_shrinks_every_dimension_to_its_boundary() {
        let start = SimConfig {
            seed: 9,
            scenario: Scenario::ClaimRefundRace,
            sessions: 64,
            steps: 512,
            nodes: 16,
        };
        // The synthetic property: the violation needs 3 sessions, 20 steps
        // and 4 nodes to occur at all.
        let minimized = minimize(&start, Some("CLAIM_REFUND_MIX"), DEFAULT_MAX_RUNS, |config| {
            if config.sessions >= 3 && config.steps >= 20 && config.nodes >= 4 {
                outcome(config, "CLAIM_REFUND_MIX")
            } else {
                clean(config)
            }
        })
        .expect("a reproducing start minimizes");

        assert_eq!(minimized.config.sessions, 3);
        assert_eq!(minimized.config.steps, 20);
        assert_eq!(minimized.config.nodes, 4);
        assert!(!minimized.outcome.is_pass(), "the minimized run still fails");
        assert!(minimized.verified, "the minimized reproducer was re-run and failed again");
        assert!(minimized.runs <= DEFAULT_MAX_RUNS);
    }

    #[test]
    fn a_different_violation_does_not_satisfy_the_target() {
        let start = SimConfig::default();
        let minimized = minimize(&start, Some("REFUND_AFTER_CLAIM"), 40, |config| {
            outcome(config, "CLAIM_REFUND_MIX")
        });
        assert!(minimized.is_none(), "the target invariant must be the one that survives");
    }

    #[test]
    fn a_passing_start_is_not_minimized_into_a_failure() {
        let start = SimConfig::default();
        let minimized = minimize(&start, None, 40, clean);
        assert!(minimized.is_none());
    }
}
