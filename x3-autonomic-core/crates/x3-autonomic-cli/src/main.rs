//! X3 Autonomic CLI Binary
//!
//! Main entry point for the x3-autonomic command-line tool.

//! X3 Autonomic CLI Binary
//!
//! Every action this CLI used to advertise printed a line claiming it had happened —
//! "Setting autonomy level to: X", "Checking health status...", "Running audit..." — and
//! performed nothing: it has no RPC client, so it cannot reach a node at all. This binary now
//! parses its arguments, maps them to the library's [`Command`] type, and refuses the action
//! with a non-zero status and a message saying what is missing. It is the same rule the wallet
//! CLI follows ("not implemented: this CLI does not talk to a node yet").

use clap::{Parser, Subcommand};
use x3_autonomic_cli::{CliConfig, Command};
use x3_autonomic_types::AutonomyLevel;

#[derive(Parser)]
#[command(name = "x3-autonomic")]
#[command(about = "X3 Autonomic Core CLI", long_about = None)]
struct Cli {
    /// RPC endpoint
    #[arg(short, long, default_value = "ws://localhost:9944")]
    rpc: String,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Set the autonomy level
    SetAutonomy {
        /// Autonomy level (0-5 or manual/auto/self-improving/self-governing)
        level: String,
    },
    /// Check health status
    Health,
    /// Run audit
    Audit,
    /// List invariants
    Invariants,
    /// View metrics
    Metrics,
}

/// Parse the autonomy level argument: a number 0-5 or one of the ladder's names.
fn parse_autonomy(level: &str) -> Result<AutonomyLevel, String> {
    let parsed = match level.trim().to_ascii_lowercase().as_str() {
        "0" | "manual" => AutonomyLevel::Manual,
        "1" | "monitored" => AutonomyLevel::Monitored,
        "2" | "detected-human-approval" | "detectedhumanapproval" => {
            AutonomyLevel::DetectedHumanApproval
        }
        "3" | "staged-rollout" | "stagedrollout" => AutonomyLevel::StagedRollout,
        "4" | "canary" => AutonomyLevel::Canary,
        "5" | "fully-autonomous" | "fullyautonomous" => AutonomyLevel::FullyAutonomous,
        other => {
            return Err(format!(
                "'{other}' is not an autonomy level: use 0-5 or manual|monitored|\
                 detected-human-approval|staged-rollout|canary|fully-autonomous"
            ))
        }
    };
    Ok(parsed)
}

fn main() {
    let cli = Cli::parse();

    let config = CliConfig::default()
        .with_verbose(cli.verbose)
        .with_autonomy(AutonomyLevel::Manual);

    let Some(requested) = &cli.command else {
        println!("X3 Autonomic Core CLI");
        println!("Use --help for usage information");
        return;
    };

    // Parsing is real: a bad level or an unreadable proposal is a parameter error.
    let command = match requested {
        Commands::SetAutonomy { level } => match parse_autonomy(level) {
            Ok(level) => Command::SetAutonomy(level),
            Err(message) => {
                eprintln!("x3-autonomic: {message}");
                std::process::exit(1);
            }
        },
        Commands::Health => Command::HealthCheck,
        Commands::Audit => Command::RunAudit,
        Commands::Invariants => Command::ListInvariants,
        Commands::Metrics => Command::ViewMetrics,
    };

    if config.verbose {
        eprintln!(
            "x3-autonomic: endpoint {}",
            String::from_utf8_lossy(&config.rpc_endpoint)
        );
    }

    // Then refuse, because nothing here can carry the command out.
    eprintln!(
        "x3-autonomic: {command:?} is not implemented: this binary has no node client, so it \
         cannot submit a transaction or read chain state (the RPC endpoint it was given is \
         unused). Nothing was changed."
    );
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use super::parse_autonomy;
    use x3_autonomic_types::AutonomyLevel;

    /// Parsing is the one thing this binary really does, so it is the thing under test: names and
    /// numbers map to the ladder, and anything else is a parameter error rather than a default.
    #[test]
    fn the_autonomy_argument_parses_names_and_numbers() {
        assert_eq!(parse_autonomy("0"), Ok(AutonomyLevel::Manual));
        assert_eq!(parse_autonomy("manual"), Ok(AutonomyLevel::Manual));
        assert_eq!(
            parse_autonomy("2"),
            Ok(AutonomyLevel::DetectedHumanApproval)
        );
        assert_eq!(
            parse_autonomy("detected-human-approval"),
            Ok(AutonomyLevel::DetectedHumanApproval)
        );
        assert_eq!(
            parse_autonomy("Fully-Autonomous"),
            Ok(AutonomyLevel::FullyAutonomous),
            "the match is case-insensitive"
        );

        let error = parse_autonomy("auto").expect_err("there is no `auto` level");
        assert!(error.contains("not an autonomy level"), "{error}");
        assert!(parse_autonomy("6").is_err(), "6 is past the ladder");
        assert!(parse_autonomy("").is_err());
    }
}
