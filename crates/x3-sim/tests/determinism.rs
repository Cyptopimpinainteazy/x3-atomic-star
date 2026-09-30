//! A simulator is only useful if a failure can be replayed.
//!
//! These tests pin the property the whole design rests on: the same seed
//! produces the same trace, and the seed actually controls the schedule.

use x3_sim::{run, Scenario, SimConfig};

fn config(seed: u64, scenario: Scenario) -> SimConfig {
    SimConfig {
        seed,
        scenario,
        sessions: 6,
        steps: 150,
        nodes: 4,
    }
}

#[test]
fn same_seed_reproduces_the_same_run() {
    for scenario in Scenario::all() {
        for seed in [0u64, 1, 42, 948_218_671, u64::MAX] {
            let first = run(&config(seed, scenario));
            let second = run(&config(seed, scenario));
            let label = format!("{} seed={seed}", scenario.as_str());

            assert_eq!(first.trace_digest, second.trace_digest, "trace {label}");
            assert_eq!(first.state_digest, second.state_digest, "state {label}");
            assert_eq!(first.accepted, second.accepted, "accepted {label}");
            assert_eq!(first.rejected, second.rejected, "rejected {label}");
            assert_eq!(first.restarts, second.restarts, "restarts {label}");
            assert_eq!(first.violations, second.violations, "violations {label}");
            assert_eq!(first.trace, second.trace, "trace lines {label}");
        }
    }
}

#[test]
fn the_seed_actually_changes_the_schedule() {
    let base = config(1, Scenario::PartitionStorm);
    let other = config(2, Scenario::PartitionStorm);
    assert_ne!(
        run(&base).trace_digest,
        run(&other).trace_digest,
        "two seeds produced identical traces — the schedule is not seed-derived"
    );
}

#[test]
fn happy_path_completes_every_session_without_refusals() {
    let outcome = run(&SimConfig {
        seed: 7,
        scenario: Scenario::HappyPath,
        sessions: 5,
        steps: 200,
        nodes: 3,
    });

    assert!(outcome.is_pass(), "violations: {:?}", outcome.violations);
    assert_eq!(
        outcome.sessions_completed, 5,
        "every session should reach Complete"
    );
    assert_eq!(outcome.sessions_refunded, 0);
    assert_eq!(
        outcome.rejected, 0,
        "a compliant sequence must not be refused"
    );
}
