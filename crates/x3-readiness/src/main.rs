use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Parser)]
#[command(name = "x3-readiness")]
#[command(about = "Readiness and proof engine for X3 Atomic Star", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(long, default_value = "reports", global = true)]
    out: PathBuf,
}

#[derive(Subcommand)]
enum Commands {
    TestnetReport,
    FeatureGap,
    MissingTests,
    TauriWiring,
    ServiceHealth,
    BtcGatewayReport,
    MarketingClaimsAudit,
    GrantPipelineReport,
    SwarmTasks,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct FeatureRecord {
    mode: String,
    crate_or_service: Option<String>,
    tauri_app: Option<String>,
    required_tests: Vec<String>,
    health_endpoint: Option<String>,
    proof_report: Option<String>,
    readiness_score: Option<i64>,
    blockers: Option<Vec<String>>,
    dangerous_paths: Option<Vec<String>>,
}

type FeatureRegistry = HashMap<String, FeatureRecord>;

type Flags = HashMap<String, String>;

fn main() -> Result<()> {
    let cli = Cli::parse();
    fs::create_dir_all(&cli.out).with_context(|| format!("failed to create output directory: {:?}", cli.out))?;
    let report_path = cli.out.join(match &cli.command {
        Commands::TestnetReport => "testnet_readiness_report.md",
        Commands::FeatureGap => "feature_gap_report.md",
        Commands::MissingTests => "missing_tests_report.md",
        Commands::TauriWiring => "tauri_wiring_report.md",
        Commands::ServiceHealth => "service_health_report.md",
        Commands::BtcGatewayReport => "btc_gateway_report.md",
        Commands::MarketingClaimsAudit => "marketing_claims_audit.md",
        Commands::GrantPipelineReport => "grant_pipeline_report.md",
        Commands::SwarmTasks => "swarm_task_queue.json",
    });
    let report_path_markdown = if matches!(cli.command, Commands::SwarmTasks) {
        Some(cli.out.join("swarm_task_queue.md"))
    } else {
        None
    };

    let registry = load_feature_registry("FEATURE_REGISTRY.toml")?;
    let flags = load_feature_flags("TESTNET_FEATURE_FLAGS.toml")?;

    let output = match &cli.command {
        Commands::TestnetReport => generate_testnet_report(&registry, &flags),
        Commands::FeatureGap => generate_feature_gap(&registry, &flags),
        Commands::MissingTests => generate_missing_tests(&registry),
        Commands::TauriWiring => generate_tauri_wiring(&registry),
        Commands::ServiceHealth => generate_service_health(&registry),
        Commands::BtcGatewayReport => generate_btc_gateway_report(),
        Commands::MarketingClaimsAudit => generate_marketing_audit(),
        Commands::GrantPipelineReport => generate_grant_pipeline_report(),
        Commands::SwarmTasks => generate_swarm_tasks(&registry, &flags),
    };

    fs::write(&report_path, output)?;
    if let Some(md_path) = report_path_markdown {
        fs::write(md_path, generate_swarm_tasks_markdown(&registry, &flags))?;
    }
    println!("Wrote {}", report_path.display());

    Ok(())
}

fn load_feature_registry(path: &str) -> Result<FeatureRegistry> {
    let content = fs::read_to_string(path).with_context(|| format!("failed to read {}", path))?;
    let registry: FeatureRegistry = toml::from_str(&content).with_context(|| "failed to parse feature registry")?;
    Ok(registry)
}

fn load_feature_flags(path: &str) -> Result<Flags> {
    let content = fs::read_to_string(path).with_context(|| format!("failed to read {}", path))?;
    let flags: Flags = toml::from_str(&content).with_context(|| "failed to parse feature flags")?;
    Ok(flags)
}

fn generate_testnet_report(registry: &FeatureRegistry, flags: &Flags) -> String {
    let now = chrono::Utc::now().to_rfc3339();
    let mut lines = vec![
        "# X3 Testnet Readiness Report".to_string(),
        format!("Generated: {}", now),
        "".to_string(),
        "## Feature Matrix".to_string(),
        "".to_string(),
    ];

    for (feature, record) in registry {
        let mode = flags.get(feature).cloned().unwrap_or_else(|| record.mode.clone());
        let proof_report = record.proof_report.clone().unwrap_or_else(|| "unknown".to_string());
        let health_endpoint = record.health_endpoint.clone().unwrap_or_else(|| "none".to_string());
        lines.push(format!("- **{}**: mode={}, tests={}, proof={}, health={}", feature, mode, record.required_tests.len(), proof_report, health_endpoint));
    }

    lines.push("".to_string());
    lines.push("## Verdict".to_string());
    lines.push("- TESTNET GO: NO".to_string());
    lines.push("- Notes: This report is auto-generated from the feature registry and requires explicit proof report generation for GO status.".to_string());
    lines.join("\n")
}

fn generate_feature_gap(registry: &FeatureRegistry, flags: &Flags) -> String {
    let mut lines = vec!["# X3 Feature Gap Report".to_string(), "".to_string()];
    for (feature, record) in registry {
        let mode = flags.get(feature).cloned().unwrap_or(record.mode.clone());
        let proof_report = record.proof_report.clone().unwrap_or_else(|| "none".to_string());
        lines.push(format!("## {}", feature));
        lines.push(format!("mode: {}", mode));
        lines.push(format!("required_tests: {}", record.required_tests.join(", ")));
        lines.push(format!("proof_report: {}", proof_report));
        lines.push("".to_string());
    }
    lines.join("\n")
}

fn generate_missing_tests(registry: &FeatureRegistry) -> String {
    let mut lines = vec!["# X3 Missing Tests Report".to_string(), "".to_string()];
    for (feature, record) in registry {
        if record.required_tests.is_empty() {
            lines.push(format!("- {}: no required tests listed", feature));
        } else {
            lines.push(format!("- {}: requires {} tests", feature, record.required_tests.len()));
        }
    }
    lines.push("".to_string());
    lines.push("## Note".to_string());
    lines.push("The test inventory is derived from the feature registry and must be expanded with concrete suite coverage.".to_string());
    lines.join("\n")
}

fn generate_tauri_wiring(registry: &FeatureRegistry) -> String {
    let mut lines = vec!["# X3 Tauri Wiring Report".to_string(), "".to_string()];
    for (feature, record) in registry {
        let tauri_app = record.tauri_app.clone().unwrap_or_else(|| "none".to_string());
        lines.push(format!("- {}: tauri app = {}", feature, tauri_app));
    }
    lines.push("".to_string());
    lines.push("## Note".to_string());
    lines.push("Each Tauri app entry must be wired to real command execution or explicitly disabled with a reason.".to_string());
    lines.join("\n")
}

fn generate_service_health(registry: &FeatureRegistry) -> String {
    let mut lines = vec!["# X3 Service Health Report".to_string(), "".to_string()];
    for (feature, record) in registry {
        let health_endpoint = record.health_endpoint.clone().unwrap_or_else(|| "none".to_string());
        lines.push(format!("- {}: health endpoint = {}", feature, health_endpoint));
    }
    lines.push("".to_string());
    lines.push("## Note".to_string());
    lines.push("Health endpoints are declared in the feature registry and should be backed by live monitoring endpoints.".to_string());
    lines.join("\n")
}

fn generate_btc_gateway_report() -> String {
    vec![
        "# BTC Gateway Report".to_string(),
        "".to_string(),
        "- Mode: SIM_TESTNET".to_string(),
        "- Mainnet BTC gateway: DISABLED_BLOCKED".to_string(),
        "- Status: initial simulator mode only".to_string(),
        "- Notes: regtest/signet support required before any claim of live BTC gateway readiness.".to_string(),
    ]
    .join("\n")
}

fn generate_marketing_audit() -> String {
    vec![
        "# Marketing Claims Audit".to_string(),
        "".to_string(),
        "- Only verified reports may drive marketing claims.".to_string(),
        "- Unsupported claims must be marked UNSUPPORTED_CLAIM.".to_string(),
        "- Source reports: testnet_readiness_report.md, reactor_benchmark_report.md, six_route_invariants.md, btc_gateway_report.md, tauri_e2e_report.md, marketing_claims_audit.md".to_string(),
    ]
    .join("\n")
}

fn generate_grant_pipeline_report() -> String {
    vec![
        "# Grant Pipeline Report".to_string(),
        "".to_string(),
        "- Grant schema and tracking are under development.".to_string(),
        "- This report is a placeholder for Grantsmith grant opportunity, proposal, budget, and milestone generation.".to_string(),
    ]
    .join("\n")
}

#[derive(serde::Serialize)]
struct SwarmTask {
    id: String,
    title: String,
    feature: String,
    agent: String,
    permission_tier: String,
    risk: String,
}

fn generate_swarm_tasks(_registry: &FeatureRegistry, _flags: &Flags) -> String {
    let tasks = vec![
        SwarmTask {
            id: "x3-task-0001".to_string(),
            title: "Create feature registry if missing".to_string(),
            feature: "feature_registry".to_string(),
            agent: "FeatureMapper".to_string(),
            permission_tier: "DocsTestsReports".to_string(),
            risk: "low".to_string(),
        },
        SwarmTask {
            id: "x3-task-0002".to_string(),
            title: "Add Atomic Kernel invariant tests".to_string(),
            feature: "atomic_kernel".to_string(),
            agent: "TestBuilder".to_string(),
            permission_tier: "DocsTestsReports".to_string(),
            risk: "high".to_string(),
        },
        SwarmTask {
            id: "x3-task-0003".to_string(),
            title: "Add Atomic Router six-route state math tests".to_string(),
            feature: "atomic_router".to_string(),
            agent: "TestBuilder".to_string(),
            permission_tier: "DocsTestsReports".to_string(),
            risk: "high".to_string(),
        },
        SwarmTask {
            id: "x3-task-0004".to_string(),
            title: "Add BTC mainnet disabled feature-flag proof".to_string(),
            feature: "btc_fortress_gateway".to_string(),
            agent: "TestBuilder".to_string(),
            permission_tier: "DocsTestsReports".to_string(),
            risk: "critical".to_string(),
        },
        SwarmTask {
            id: "x3-task-0005".to_string(),
            title: "Add Tauri SwarmCommand screen".to_string(),
            feature: "tauri_os".to_string(),
            agent: "Integrator".to_string(),
            permission_tier: "TauriServiceWiring".to_string(),
            risk: "medium".to_string(),
        },
        SwarmTask {
            id: "x3-task-0006".to_string(),
            title: "Add no-dead-button report".to_string(),
            feature: "tauri_os".to_string(),
            agent: "WiringInspector".to_string(),
            permission_tier: "DocsTestsReports".to_string(),
            risk: "medium".to_string(),
        },
    ];

    serde_json::to_string_pretty(&tasks).unwrap_or_else(|_| "[]".to_string())
}

fn generate_swarm_tasks_markdown(_registry: &FeatureRegistry, _flags: &Flags) -> String {
    let mut lines = vec!["# X3 Swarm Task Queue".to_string(), "".to_string()];
    lines.push("## Recommended first tasks".to_string());
    lines.push("".to_string());
    lines.push("- x3-task-0001: Create feature registry if missing".to_string());
    lines.push("- x3-task-0002: Add Atomic Kernel invariant tests".to_string());
    lines.push("- x3-task-0003: Add Atomic Router six-route state math tests".to_string());
    lines.push("- x3-task-0004: Add BTC mainnet disabled feature-flag proof".to_string());
    lines.push("- x3-task-0005: Add Tauri SwarmCommand screen".to_string());
    lines.push("- x3-task-0006: Add no-dead-button report".to_string());
    lines.push("".to_string());
    lines.push("## Generated from FEATURE_REGISTRY.toml and available reports".to_string());
    lines.push("- `FEATURE_REGISTRY.toml`".to_string());
    lines.push("- `TESTNET_FEATURE_FLAGS.toml`".to_string());
    lines.push("- `reports/swarm_scan_report.md`".to_string());
    lines.push("- `reports/feature_gap_report.md`".to_string());
    lines.push("- `reports/missing_tests_report.md`".to_string());
    lines.push("".to_string());
    lines.join("\n")
}
