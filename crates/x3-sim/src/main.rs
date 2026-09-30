//! `x3-sim` command line entry point.
//!
//! Exit codes follow the repository's fail-closed convention: `0` when every
//! invariant held, `1` when a run produced a violation, `2` for a usage error.

use std::path::PathBuf;
use std::process::ExitCode;

use x3_sim::{run, Scenario, SimConfig, SimOutcome};

const USAGE: &str = "\
x3-sim — deterministic fault-injection simulator for X3 atomic swaps

USAGE:
    cargo run -p x3-sim -- [OPTIONS]

OPTIONS:
    --seed <u64>          Seed for the run (default 0). Replay uses this.
    --scenario <name>     happy-path | claim-refund-race | partition-storm | crash-recovery
    --sessions <n>        Atomic swap sessions in the ledger (default 8)
    --steps <n>           Scheduler steps (default 120)
    --nodes <n>           Node 0 is the coordinator, the rest are clients (default 4)
    --out <dir>           Write the evidence bundle (JSON + trace) here
    --json                Print the evidence bundle to stdout instead of a summary
    -h, --help            Show this help
";

fn main() -> ExitCode {
    let mut config = SimConfig::default();
    let mut out: Option<PathBuf> = None;
    let mut json = false;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        let mut value = || -> Option<String> {
            index += 1;
            args.get(index).cloned()
        };
        match arg {
            "-h" | "--help" => {
                print!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--json" => json = true,
            "--seed" => match value().and_then(|raw| raw.parse::<u64>().ok()) {
                Some(parsed) => config.seed = parsed,
                None => return usage_error("--seed requires an unsigned integer"),
            },
            "--sessions" => match value().and_then(|raw| raw.parse::<usize>().ok()) {
                Some(parsed) => config.sessions = parsed,
                None => return usage_error("--sessions requires a positive integer"),
            },
            "--steps" => match value().and_then(|raw| raw.parse::<usize>().ok()) {
                Some(parsed) => config.steps = parsed,
                None => return usage_error("--steps requires a positive integer"),
            },
            "--nodes" => match value().and_then(|raw| raw.parse::<usize>().ok()) {
                Some(parsed) => config.nodes = parsed,
                None => return usage_error("--nodes requires an integer of at least 2"),
            },
            "--scenario" => match value().and_then(|raw| Scenario::parse(&raw)) {
                Some(parsed) => config.scenario = parsed,
                None => return usage_error("--scenario must be one of the documented names"),
            },
            "--out" => match value() {
                Some(path) => out = Some(PathBuf::from(path)),
                None => return usage_error("--out requires a directory"),
            },
            other => return usage_error(&format!("unknown argument '{other}'")),
        }
        index += 1;
    }

    if let Some(dir) = &out {
        if let Err(error) = std::fs::create_dir_all(dir) {
            eprintln!("x3-sim: cannot create evidence directory: {error}");
            return ExitCode::from(2);
        }
    }

    let outcome = run(&config);

    if json {
        println!("{}", outcome.to_evidence_json());
    } else {
        print_summary(&outcome);
    }

    if let Some(dir) = &out {
        if let Err(error) = write_evidence(dir, &outcome) {
            eprintln!("x3-sim: cannot write evidence bundle: {error}");
            return ExitCode::from(2);
        }
        if !json {
            println!("evidence: {}", dir.display());
        }
    }

    if outcome.is_pass() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn usage_error(message: &str) -> ExitCode {
    eprintln!("x3-sim: {message}\n\n{USAGE}");
    ExitCode::from(2)
}

fn print_summary(outcome: &SimOutcome) {
    println!("seed = {}", outcome.seed);
    println!("scenario = {}", outcome.scenario);
    println!(
        "steps = {}  accepted = {}  rejected = {}",
        outcome.steps, outcome.accepted, outcome.rejected
    );
    println!(
        "restarts = {}  stale_writes = {}  partitions = {}",
        outcome.restarts, outcome.stale_writes, outcome.partitions
    );
    println!(
        "network: sent = {}  delivered = {}  dropped = {}  partitioned = {}  reordered = {}",
        outcome.network.sent,
        outcome.network.delivered,
        outcome.network.dropped,
        outcome.network.partitioned,
        outcome.network.reordered
    );
    println!(
        "sessions: completed = {}  refunded = {}",
        outcome.sessions_completed, outcome.sessions_refunded
    );
    println!("trace_digest = {}", outcome.trace_digest);
    println!("state_digest = {}", outcome.state_digest);

    if outcome.is_pass() {
        println!("PASS");
    } else {
        println!("FAIL");
        println!("violations:");
        for violation in &outcome.violations {
            println!(
                "  {} session={} {}",
                violation.code, violation.session_id, violation.detail
            );
        }
        println!("Replay:");
        println!("  {}", outcome.replay_command());
    }
}

fn write_evidence(dir: &std::path::Path, outcome: &SimOutcome) -> std::io::Result<()> {
    let stem = format!("x3-sim-{}-{}", outcome.scenario, outcome.seed);
    std::fs::write(dir.join(format!("{stem}.json")), outcome.to_evidence_json())?;
    std::fs::write(dir.join(format!("{stem}.trace.log")), outcome.trace.join("\n"))?;
    Ok(())
}
