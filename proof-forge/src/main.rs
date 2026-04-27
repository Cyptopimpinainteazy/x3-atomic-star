mod runners;
mod scoring;
mod registry;
mod dashboard;
mod proof;
mod todo_proof;
mod gap_proof;
mod receipt;

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

    /// Run ALL critical proofs and gates - MUST PASS for mainnet
    ProveEverything {
        /// Strict mode (fail on any issue)
        #[arg(short, long)]
        strict: bool,

        /// Fail hard on blockers
        #[arg(long)]
        fail_hard: bool,

        /// Generate receipts
        #[arg(long)]
        receipts: bool,
    },

    /// Scan for TODO/FIXME/HACK/stub/mock/fake code
    TodoGate {
        /// Gate to check (mainnet, testnet)
        #[arg(value_name = "GATE", default_value = "mainnet")]
        gate: String,

        /// Fail on blockers
        #[arg(long)]
        fail_hard: bool,
    },

    /// Scan for missing implementations, tests, and wiring
    GapGate {
        /// Gate to check (mainnet, testnet)
        #[arg(value_name = "GATE", default_value = "mainnet")]
        gate: String,

        /// Fail on blockers
        #[arg(long)]
        fail_hard: bool,
    },
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

        Commands::ProveEverything { strict, fail_hard, receipts } => {
            prove_everything(&cli.workspace, strict, fail_hard, receipts, cli.verbose).await?
        }

        Commands::TodoGate { gate, fail_hard } => {
            run_todo_gate(&cli.workspace, &gate, fail_hard, cli.verbose).await?
        }

        Commands::GapGate { gate, fail_hard } => {
            run_gap_gate(&cli.workspace, &gate, fail_hard, cli.verbose).await?
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

/// Prove Everything - The Ultimate Gate
async fn prove_everything(
    workspace: &PathBuf,
    strict: bool,
    fail_hard: bool,
    receipts: bool,
    verbose: bool,
) -> Result<()> {
    println!("{}", "🔥 PROVE EVERYTHING - Ultimate X3 Proof Gauntlet".bold().red());
    println!();
    
    let mut all_pass = true;
    let mut failures = Vec::new();

    // 1. TodoGate
    println!("{}", "▸ Running TodoGate...".cyan());
    if let Err(e) = run_todo_gate(workspace, "mainnet", true, verbose).await {
        all_pass = false;
        failures.push(format!("TodoGate: {}", e));
    }

    // 2. GapGate
    println!("{}", "▸ Running GapGate...".cyan());
    if let Err(e) = run_gap_gate(workspace, "mainnet", true, verbose).await {
        all_pass = false;
        failures.push(format!("GapGate: {}", e));
    }

    // 3. Security Gate
    println!("{}", "▸ Running SecurityGate...".cyan());
    if let Err(e) = runners::check_security_gate(workspace, true, verbose).await {
        all_pass = false;
        failures.push(format!("SecurityGate: {}", e));
    }

    // 4. Mainnet Gate
    println!("{}", "▸ Running MainnetGate...".cyan());
    if let Err(e) = runners::check_mainnet_readiness(workspace, true, true, verbose).await {
        all_pass = false;
        failures.push(format!("MainnetGate: {}", e));
    }

    // 5. Critical Claims
    println!("{}", "▸ Verifying Critical Claims...".cyan());
    let critical_claims = vec![
        "x3.asset_kernel.supply_conservation",
        "x3.bridge.replay_protection",
        "x3.bridge.finality_verification",
        "x3.atomic.one_terminal_state",
        "x3.atomic.rollback_safety",
        "x3.flashloan.repay_or_revert",
        "x3.x3vm.determinism",
        "x3.x3lang.compiler_reproducibility",
        "x3.contracts.evm_svm_parity",
        "x3.governance.proof_gated_upgrade",
        "x3.proofforge.receipt_integrity",
    ];

    for claim in critical_claims {
        if let Err(e) = runners::verify_claim(workspace, claim, strict, false).await {
            all_pass = false;
            failures.push(format!("Claim {}: {}", claim, e));
        }
    }

    println!();
    if all_pass {
        println!("{}", "✓ PROVE EVERYTHING PASSED - X3 is proof-ready".bold().green());
        
        if receipts {
            println!("Generating master receipt...");
            runners::generate_receipt(workspace, "mainnet", &vec![], verbose).await?;
        }
        
        Ok(())
    } else {
        println!("{}", "✗ PROVE EVERYTHING FAILED".bold().red());
        println!();
        println!("{}", "Failures:".bold());
        for failure in &failures {
            println!("  - {}", failure.red());
        }
        println!();
        
        if fail_hard {
            anyhow::bail!("prove-everything failed with {} blockers", failures.len());
        }
        
        Ok(())
    }
}

/// Run TODO/FIXME/HACK scanner
async fn run_todo_gate(
    workspace: &PathBuf,
    gate: &str,
    fail_hard: bool,
    verbose: bool,
) -> Result<()> {
    use todo_proof::TodoScanner;
    
    println!("{}", format!("📋 TODO Gate: {} readiness", gate).bold().yellow());
    println!();

    let scanner = TodoScanner::new(workspace.clone());
    let report = scanner.scan(verbose)?;

    println!("Total TODOs found: {}", report.total_todos);
    println!();

    println!("By severity:");
    for (severity, count) in &report.by_severity {
        println!("  {}: {}", severity, count);
    }
    println!();

    let mainnet_blockers = report.mainnet_blockers.len();
    let testnet_blockers = report.testnet_blockers.len();

    println!("Mainnet blockers (T5+): {}", if mainnet_blockers > 0 {
        mainnet_blockers.to_string().red()
    } else {
        mainnet_blockers.to_string().green()
    });

    println!("Testnet blockers (T6+): {}", if testnet_blockers > 0 {
        testnet_blockers.to_string().red()
    } else {
        testnet_blockers.to_string().green()
    });

    println!();

    if !report.mainnet_blockers.is_empty() && verbose {
        println!("{}", "Mainnet Blockers:".bold().red());
        for item in &report.mainnet_blockers {
            println!("  {} (line {}) - {:?}", 
                item.file.display(), 
                item.line, 
                item.severity
            );
            println!("    {}", item.content);
        }
        println!();
    }

    // Check gate
    let passes = scanner.check_gates(&report, gate)?;
    
    if passes {
        println!("{}", format!("✓ {} gate PASSED", gate).bold().green());
        Ok(())
    } else {
        println!("{}", format!("✗ {} gate FAILED", gate).bold().red());
        
        if fail_hard {
            anyhow::bail!("{} gate failed: {} blockers found", gate, 
                if gate == "mainnet" { mainnet_blockers } else { testnet_blockers });
        }
        
        Ok(())
    }
}

/// Run Gap scanner
async fn run_gap_gate(
    workspace: &PathBuf,
    gate: &str,
    fail_hard: bool,
    verbose: bool,
) -> Result<()> {
    use gap_proof::GapScanner;
    
    println!("{}", format!("🔍 Gap Gate: {} readiness", gate).bold().yellow());
    println!();

    let scanner = GapScanner::new(workspace.clone());
    let report = scanner.scan(verbose)?;

    println!("Total gaps found: {}", report.total_gaps);
    println!();

    println!("By type:");
    for (gap_type, count) in &report.by_type {
        println!("  {}: {}", gap_type, count);
    }
    println!();

    let s0_gaps = report.s0_gaps.len();
    let mainnet_blockers = report.mainnet_blockers.len();
    let testnet_blockers = report.testnet_blockers.len();

    println!("S0 gaps (critical): {}", if s0_gaps > 0 {
        s0_gaps.to_string().red()
    } else {
        s0_gaps.to_string().green()
    });

    println!("Mainnet blockers: {}", if mainnet_blockers > 0 {
        mainnet_blockers.to_string().red()
    } else {
        mainnet_blockers.to_string().green()
    });

    println!("Testnet blockers: {}", if testnet_blockers > 0 {
        testnet_blockers.to_string().red()
    } else {
        testnet_blockers.to_string().green()
    });

    println!();

    if !report.s0_gaps.is_empty() && verbose {
        println!("{}", "S0 Gaps (CRITICAL):".bold().red());
        for item in &report.s0_gaps {
            println!("  [{}] {}: {}", 
                item.area,
                format!("{:?}", item.gap_type).red(),
                item.description
            );
        }
        println!();
    }

    // Check gate
    let passes = scanner.check_gates(&report, gate)?;
    
    if passes {
        println!("{}", format!("✓ {} gate PASSED", gate).bold().green());
        Ok(())
    } else {
        println!("{}", format!("✗ {} gate FAILED", gate).bold().red());
        
        if fail_hard {
            anyhow::bail!("{} gate failed: {} S0 gaps, {} blockers", 
                gate, s0_gaps, mainnet_blockers);
        }
        
        Ok(())
    }
}
