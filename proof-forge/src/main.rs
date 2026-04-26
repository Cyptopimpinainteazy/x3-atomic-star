mod runners;
mod scoring;
mod registry;
mod dashboard;
mod proof;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "x3-proof")]
#[command(about = "X3 ProofForge - Executable Truth Layer for X3")]
#[command(version = "1.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to X3 codebase
    #[arg(global = true, long, default_value = ".")]
    workspace: PathBuf,

    /// Enable verbose output
    #[arg(global = true, short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Verify a claim with required proof
    Verify {
        /// Claim ID (e.g., x3.bridge.replay_protection)
        #[arg(value_name = "CLAIM_ID")]
        claim: String,

        /// Run with strict mode (require all proofs)
        #[arg(short, long)]
        strict: bool,
    },

    /// Run proof for a specific area
    Prove {
        /// Area to prove (asset-kernel, bridge, consensus, etc.)
        #[arg(value_name = "AREA")]
        area: String,

        /// Strict mode
        #[arg(short, long)]
        strict: bool,

        /// Dry run (show what would execute)
        #[arg(long)]
        dry_run: bool,
    },

    /// Run all proofs
    ProveAll {
        #[arg(short, long)]
        strict: bool,

        #[arg(long)]
        dry_run: bool,

        /// Run in parallel
        #[arg(long)]
        parallel: bool,
    },

    /// Check security gates (S0/S1 blockers)
    SecurityGate {
        #[arg(short, long)]
        fail_hard: bool,
    },

    /// Test hack resistance
    Hack {
        /// Specific area to test
        #[arg(value_name = "AREA")]
        area: Option<String>,

        #[arg(short, long)]
        strict: bool,
    },

    /// Test edge cases
    EdgeCase {
        /// Area to test
        #[arg(value_name = "AREA")]
        area: Option<String>,

        #[arg(short, long)]
        strict: bool,
    },

    /// Test degraded operation (limp to finish)
    Limp {
        /// Area to test
        #[arg(value_name = "AREA")]
        area: Option<String>,

        #[arg(short, long)]
        strict: bool,
    },

    /// Test operator safety (idiot-proof mode)
    Idiot {
        /// Command to test
        #[arg(value_name = "COMMAND")]
        command: String,

        #[arg(long)]
        dry_run: bool,
    },

    /// Check formal proofs
    Formal {
        /// Area to check
        #[arg(value_name = "AREA")]
        area: Option<String>,
    },

    /// Generate proof receipt
    Receipt {
        /// Receipt type (mainnet, testnet, upgrade, etc.)
        #[arg(value_name = "TYPE")]
        receipt_type: String,

        /// Areas to include in receipt
        #[arg(value_name = "AREAS")]
        areas: Vec<String>,
    },

    /// Check mainnet readiness
    MainnetGate {
        #[arg(short, long)]
        fail_hard: bool,

        #[arg(long)]
        strict: bool,
    },

    /// Check testnet readiness
    TestnetGate {
        #[arg(short, long)]
        fail_hard: bool,
    },

    /// Generate proof dashboard export
    Dashboard {
        /// Output file
        #[arg(short, long, default_value = "proof-score.json")]
        output: PathBuf,

        /// Include detailed reports
        #[arg(long)]
        detailed: bool,
    },

    /// Scan for unproven claims
    ScanClaims {
        /// File to scan (markdown/code)
        #[arg(value_name = "FILE")]
        file: Option<PathBuf>,

        #[arg(long)]
        fail_on_unproven: bool,
    },

    /// Check AI patch safety
    AiPatchFirewall {
        /// Git diff to check
        #[arg(value_name = "DIFF")]
        diff: Option<String>,

        #[arg(long)]
        fail_hard: bool,
    },

    /// Explain blockers for an area
    ExplainBlockers {
        /// Area
        #[arg(value_name = "AREA")]
        area: String,
    },

    /// List all claims and their status
    Claims,

    /// Show help for all proof areas
    Help,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.verbose {
        println!("{}", "X3 ProofForge v1.0.0 - Executable Truth Layer".bold().cyan());
        println!("Workspace: {}", cli.workspace.display());
        println!();
    }

    match cli.command {
        Commands::Verify { claim, strict } => {
            runners::verify_claim(&cli.workspace, &claim, strict, cli.verbose).await?
        }

        Commands::Prove { area, strict, dry_run } => {
            runners::prove_area(&cli.workspace, &area, strict, dry_run, cli.verbose).await?
        }

        Commands::ProveAll { strict, dry_run, parallel } => {
            runners::prove_all(&cli.workspace, strict, dry_run, parallel, cli.verbose).await?
        }

        Commands::SecurityGate { fail_hard } => {
            runners::check_security_gate(&cli.workspace, fail_hard, cli.verbose).await?
        }

        Commands::Hack { area, strict } => {
            runners::test_hack_resistance(&cli.workspace, area, strict, cli.verbose).await?
        }

        Commands::EdgeCase { area, strict } => {
            runners::test_edge_cases(&cli.workspace, area, strict, cli.verbose).await?
        }

        Commands::Limp { area, strict } => {
            runners::test_limp_mode(&cli.workspace, area, strict, cli.verbose).await?
        }

        Commands::Idiot { command, dry_run } => {
            runners::test_idiot_proof(&cli.workspace, &command, dry_run, cli.verbose).await?
        }

        Commands::Formal { area } => {
            runners::check_formal_proofs(&cli.workspace, area, cli.verbose).await?
        }

        Commands::Receipt { receipt_type, areas } => {
            runners::generate_receipt(&cli.workspace, &receipt_type, &areas, cli.verbose).await?
        }

        Commands::MainnetGate { fail_hard, strict } => {
            runners::check_mainnet_readiness(&cli.workspace, fail_hard, strict, cli.verbose)
                .await?
        }

        Commands::TestnetGate { fail_hard } => {
            runners::check_testnet_readiness(&cli.workspace, fail_hard, cli.verbose).await?
        }

        Commands::Dashboard { output, detailed } => {
            dashboard::generate_dashboard(&cli.workspace, &output, detailed, cli.verbose).await?
        }

        Commands::ScanClaims { file, fail_on_unproven } => {
            runners::scan_claims(&cli.workspace, file, fail_on_unproven, cli.verbose).await?
        }

        Commands::AiPatchFirewall { diff, fail_hard } => {
            runners::check_ai_patch(&cli.workspace, diff, fail_hard, cli.verbose).await?
        }

        Commands::ExplainBlockers { area } => {
            runners::explain_blockers(&cli.workspace, &area, cli.verbose).await?
        }

        Commands::Claims => {
            runners::list_all_claims(&cli.workspace, cli.verbose).await?
        }

        Commands::Help => {
            print_help();
        }
    }

    Ok(())
}

fn print_help() {
    println!("{}", "X3 ProofForge - Available Commands".bold().green());
    println!();
    println!("{}", "Core Verification:".bold());
    println!("  x3-proof verify CLAIM_ID            - Verify a specific claim");
    println!("  x3-proof prove AREA                 - Run proofs for an area");
    println!("  x3-proof prove-all                  - Run all proofs");
    println!();
    println!("{}", "Security & Safety:".bold());
    println!("  x3-proof security-gate              - Check S0/S1 blockers");
    println!("  x3-proof hack [AREA]                - Test hack resistance");
    println!("  x3-proof edge-case [AREA]           - Test edge cases");
    println!("  x3-proof limp [AREA]                - Test degraded operation");
    println!("  x3-proof idiot COMMAND              - Test operator safety");
    println!();
    println!("{}", "Readiness & Gates:".bold());
    println!("  x3-proof mainnet-gate               - Check mainnet readiness");
    println!("  x3-proof testnet-gate               - Check testnet readiness");
    println!("  x3-proof formal [AREA]              - Check formal proofs");
    println!();
    println!("{}", "Reporting & Dashboards:".bold());
    println!("  x3-proof dashboard                  - Generate proof score dashboard");
    println!("  x3-proof receipt TYPE [AREAS]       - Generate proof receipt");
    println!("  x3-proof claims                     - List all claims");
    println!("  x3-proof scan-claims [FILE]         - Scan for unproven claims");
    println!();
    println!("{}", "Development:".bold());
    println!("  x3-proof ai-patch-firewall [DIFF]   - Check AI patch safety");
    println!("  x3-proof explain-blockers AREA      - Show blockers for area");
    println!();
    println!("{}", "Global Flags:".bold());
    println!("  --workspace PATH                    - X3 codebase path (default: current)");
    println!("  --strict                            - Strict mode (require all proofs)");
    println!("  --fail-hard                         - Fail on any error");
    println!("  --dry-run                           - Show what would execute");
    println!("  -v, --verbose                       - Verbose output");
}
