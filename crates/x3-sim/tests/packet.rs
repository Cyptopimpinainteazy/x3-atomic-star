//! Failure-packet and minimizer behaviour, against real runs of the real
//! coordinator.
//!
//! The simulator's positive control must not manufacture a packet, and the
//! minimizer must shrink real runs — with the final candidate verified by
//! another real run. The only synthetic ingredient here is the predicate: the
//! coordinator no longer has a reachable violation (that is what #558 fixed),
//! so the tests minimize a real, reproducible *near-failure* (a refused
//! operation) to exercise the machinery end to end.

use x3_sim::invariants::Violation;
use x3_sim::{minimize, run, FailurePacket, Scenario, SimConfig, DEFAULT_MAX_RUNS};

#[test]
fn a_passing_real_run_produces_no_packet() {
    let config = SimConfig {
        seed: 5,
        scenario: Scenario::HappyPath,
        ..SimConfig::default()
    };
    let outcome = run(&config);
    assert!(outcome.is_pass(), "the positive control must pass");
    assert!(
        FailurePacket::from_outcome(&config, &outcome).is_none(),
        "a passing run must never produce a failure packet"
    );
    assert!(outcome.state_before.is_none());
    assert!(outcome.first_bad_op.is_none());
}

#[test]
fn the_minimizer_drives_real_runs_and_verifies_its_result() {
    let start = SimConfig {
        seed: 11,
        scenario: Scenario::ClaimRefundRace,
        sessions: 24,
        steps: 400,
        nodes: 8,
    };

    // A real predicate over real outcomes: the run contains at least one
    // refused operation, which is the observable shape a violation would have
    // had. The synthetic violation carries a predicate code, never a real
    // invariant name, so nothing here can be mistaken for production evidence.
    let runner = |config: &SimConfig| {
        let mut outcome = run(config);
        if outcome.rejected >= 1 {
            outcome.violations.push(Violation {
                code: "TEST_REFUSAL_PRESENT",
                session_id: "<predicate>".to_string(),
                detail: format!("rejected = {}", outcome.rejected),
            });
        }
        outcome
    };

    let start_outcome = runner(&start);
    assert!(
        !start_outcome.is_pass(),
        "the starting config must reproduce the predicate"
    );

    let minimized = minimize(&start, None, DEFAULT_MAX_RUNS, runner)
        .expect("the predicate survives minimization");
    assert!(minimized.verified, "the minimized run was re-run and reproduced");
    assert!(
        minimized.config.sessions <= start.sessions
            && minimized.config.steps <= start.steps
            && minimized.config.nodes <= start.nodes,
        "minimization never grows the config"
    );
    assert!(
        minimized.config.sessions < start.sessions
            || minimized.config.steps < start.steps
            || minimized.config.nodes < start.nodes,
        "at least one dimension must shrink on a reducible predicate"
    );
    assert!(
        minimized.outcome.rejected >= 1,
        "the minimized run really reproduces the predicate"
    );
}

#[test]
fn the_cli_replays_and_minimizes_a_recorded_packet_shape() {
    // The packet's replay command must be the one the CLI accepts: run the
    // binary with exactly those arguments and require the same verdict.
    let config = SimConfig {
        seed: 7,
        scenario: Scenario::PartitionStorm,
        sessions: 3,
        steps: 40,
        nodes: 3,
    };
    let outcome = run(&config);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_x3-sim"))
        .args([
            "--seed",
            "7",
            "--scenario",
            "partition-storm",
            "--sessions",
            "3",
            "--steps",
            "40",
            "--nodes",
            "3",
        ])
        .output()
        .expect("run the simulator binary");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&outcome.trace_digest),
        "the CLI replay must produce the same trace digest as the library run: {stdout}"
    );
}
