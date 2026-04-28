use crate::proof::*;
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Verify operational / evidence-based claims:
///
///   onboarding.developer_first_value  — dev quickstart path exists and is measurable
///   funding.milestone_receipts        — funding asks map to milestones with deliverables
///   evolution.no_regression           — S0/S1 claim registry shows no new regressions
pub async fn verify_claim(workspace: &Path, claim_id: &str, verbose: bool) -> Result<ProofResult> {
    if claim_id.contains("onboarding") {
        verify_onboarding(workspace, claim_id, verbose).await
    } else if claim_id.contains("funding") {
        verify_funding(workspace, claim_id, verbose).await
    } else if claim_id.contains("evolution") {
        verify_evolution(workspace, claim_id, verbose).await
    } else {
        // Shouldn't happen, but handle gracefully
        Ok(unrecognized_claim(claim_id))
    }
}

async fn verify_onboarding(workspace: &Path, claim_id: &str, verbose: bool) -> Result<ProofResult> {
    let start = Instant::now();

    if verbose {
        println!("  → Checking developer onboarding evidence...");
    }

    let mut files_inspected = vec![];
    let mut passed_checks = vec![];
    let mut failed_checks = vec![];
    let mut missing_proofs = vec![];
    let mut evidence = HashMap::new();

    // Quick-start guide
    let quickstart_candidates = [
        "MAINNET_QUICK_START.md",
        "OPTION_D_LAUNCH_GUIDE.md",
        "docs/BLOCKCHAIN_STARTUP_GUIDE.md",
        "README.md",
    ];
    let mut found_quickstart = false;
    for candidate in &quickstart_candidates {
        if workspace.join(candidate).exists() {
            files_inspected.push(candidate.to_string());
            if !found_quickstart {
                passed_checks.push(format!("Quickstart guide found: {}", candidate));
                evidence.insert("quickstart_doc".to_string(), candidate.to_string());
                found_quickstart = true;
            }
        }
    }
    if !found_quickstart {
        failed_checks.push("No developer quickstart guide found".to_string());
        missing_proofs
            .push("Add QUICKSTART.md or README with time-to-first-value steps".to_string());
    }

    // Makefile or script with first-run target
    if workspace.join("Makefile").exists() {
        files_inspected.push("Makefile".to_string());
        passed_checks.push("Makefile present (dev entry point)".to_string());
        evidence.insert("makefile".to_string(), "present".to_string());
    } else {
        missing_proofs
            .push("No Makefile — hard for a developer to know what to run first".to_string());
    }

    // Chain node binary is buildable (check Cargo workspace references it)
    let cargo_toml = workspace.join("Cargo.toml");
    if cargo_toml.exists() {
        files_inspected.push("Cargo.toml".to_string());
        // Check if node member is listed
        if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
            if content.contains("node") || content.contains("x3-chain") {
                passed_checks.push("Node crate referenced in workspace Cargo.toml".to_string());
            } else {
                missing_proofs
                    .push("Node binary not clearly referenced in root Cargo.toml".to_string());
            }
        }
    }

    // Measure: time-to-first-value is S1 — we can only prove the DOCUMENTATION exists,
    // not the measured time. Flag as partial if we have docs but no benchmark.
    missing_proofs.push(
        "No measured time-to-first-value benchmark — \
         add a CI step that times `cargo build` + node startup + first block"
            .to_string(),
    );

    let total_checks = passed_checks.len() + failed_checks.len();
    let score = if total_checks == 0 {
        0.0
    } else {
        passed_checks.len() as f64 / total_checks as f64
    };

    let status = if !failed_checks.is_empty() {
        ProofStatus::Unverified
    } else if missing_proofs.is_empty() {
        ProofStatus::Verified
    } else {
        ProofStatus::Partial
    };

    Ok(ProofResult {
        claim_id: claim_id.to_string(),
        claim:
            "A fresh developer can deploy and test a first X3 app with measured time-to-first-value"
                .to_string(),
        status,
        proof_level: None,
        edge_case_level: None,
        hack_level: None,
        operator_level: None,
        degraded_level: None,
        files_inspected,
        commands_run: vec![],
        passed_checks,
        failed_checks,
        missing_proofs,
        blockers: vec![],
        score,
        evidence,
        timestamp: Utc::now(),
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

async fn verify_funding(workspace: &Path, claim_id: &str, verbose: bool) -> Result<ProofResult> {
    let start = Instant::now();

    if verbose {
        println!("  → Checking funding milestone evidence...");
    }

    let mut files_inspected = vec![];
    let mut passed_checks = vec![];
    let mut failed_checks = vec![];
    let mut missing_proofs = vec![];
    let mut evidence = HashMap::new();

    // Look for milestone tracking files
    let milestone_candidates = [
        "docs/ATLAS_SPHERE_ROADMAP.md",
        "docs/BUILD_PHASES.md",
        "MASTER_STATUS.md",
        "PHASE_1_2_KICKOFF.md",
    ];
    let mut milestone_count = 0;
    for candidate in &milestone_candidates {
        if workspace.join(candidate).exists() {
            files_inspected.push(candidate.to_string());
            milestone_count += 1;
        }
    }
    evidence.insert("milestone_docs".to_string(), milestone_count.to_string());
    if milestone_count > 0 {
        passed_checks.push(format!(
            "{} milestone/phase documents found",
            milestone_count
        ));
    } else {
        failed_checks.push("No milestone tracking documents found".to_string());
    }

    // Look for any ProofForge receipts — they serve as proof of deliverable completion
    let receipts_dir = workspace.join("proof/receipts/claims");
    if receipts_dir.exists() {
        let receipt_count = std::fs::read_dir(&receipts_dir)
            .map(|rd| rd.filter_map(|e| e.ok()).count())
            .unwrap_or(0);
        evidence.insert("proof_receipts".to_string(), receipt_count.to_string());
        if receipt_count > 0 {
            passed_checks.push(format!(
                "{} proof receipts exist as deliverable evidence",
                receipt_count
            ));
            files_inspected.push("proof/receipts/claims/".to_string());
        } else {
            missing_proofs
                .push("No proof receipts — no machine-verifiable deliverable evidence".to_string());
        }
    } else {
        missing_proofs.push("proof/receipts/claims/ directory missing".to_string());
    }

    // Hard gap: no explicit funding→milestone→receipt linkage file
    let link_file_exists = workspace
        .join("proof/funding/milestone-receipt-map.yml")
        .exists()
        || workspace.join("docs/funding-milestones.yml").exists();
    if link_file_exists {
        passed_checks.push("Funding→milestone→receipt linkage file found".to_string());
    } else {
        missing_proofs.push(
            "No proof/funding/milestone-receipt-map.yml — \
             create a file linking each funding ask to a milestone ID and a proof receipt"
                .to_string(),
        );
    }

    let total_checks = passed_checks.len() + failed_checks.len();
    let score = if total_checks == 0 {
        0.0
    } else {
        passed_checks.len() as f64 / total_checks as f64
    };

    let status = if !failed_checks.is_empty() {
        ProofStatus::Unverified
    } else if missing_proofs.is_empty() {
        ProofStatus::Verified
    } else {
        ProofStatus::Partial
    };

    Ok(ProofResult {
        claim_id: claim_id.to_string(),
        claim: "Every funding ask maps to a milestone, deliverable, budget, and proof receipt"
            .to_string(),
        status,
        proof_level: None,
        edge_case_level: None,
        hack_level: None,
        operator_level: None,
        degraded_level: None,
        files_inspected,
        commands_run: vec![],
        passed_checks,
        failed_checks,
        missing_proofs,
        blockers: vec![],
        score,
        evidence,
        timestamp: Utc::now(),
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

async fn verify_evolution(workspace: &Path, claim_id: &str, verbose: bool) -> Result<ProofResult> {
    let start = Instant::now();

    if verbose {
        println!("  → Checking evolution / no-regression evidence...");
    }

    let mut files_inspected = vec![];
    let mut passed_checks = vec![];
    let mut failed_checks = vec![];
    let mut missing_proofs = vec![];
    let mut evidence = HashMap::new();

    // Read the claims registry to check S0/S1 status
    let registry = workspace.join("proof/claims/registry.yml");
    if !registry.exists() {
        return Ok(unrecognized_claim(claim_id));
    }
    files_inspected.push("proof/claims/registry.yml".to_string());

    let registry_content = std::fs::read_to_string(&registry)?;

    // Count S0/S1 claims and how many are VERIFIED vs UNVERIFIED
    let mut s0_total = 0usize;
    let mut s0_verified = 0usize;
    let mut s1_total = 0usize;
    let mut s1_verified = 0usize;

    let mut current_criticality = "";
    for line in registry_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("criticality: S0") {
            current_criticality = "S0";
        } else if trimmed.starts_with("criticality: S1") {
            current_criticality = "S1";
        } else if trimmed.starts_with("status:") {
            let status_val = trimmed.trim_start_matches("status:").trim();
            match current_criticality {
                "S0" => {
                    s0_total += 1;
                    if status_val == "VERIFIED" {
                        s0_verified += 1;
                    }
                }
                "S1" => {
                    s1_total += 1;
                    if status_val == "VERIFIED" {
                        s1_verified += 1;
                    }
                }
                _ => {}
            }
            current_criticality = "";
        }
    }

    evidence.insert("s0_total".to_string(), s0_total.to_string());
    evidence.insert("s0_verified".to_string(), s0_verified.to_string());
    evidence.insert("s1_total".to_string(), s1_total.to_string());
    evidence.insert("s1_verified".to_string(), s1_verified.to_string());

    let s0_unverified = s0_total.saturating_sub(s0_verified);
    let s1_unverified = s1_total.saturating_sub(s1_verified);

    if s0_unverified == 0 {
        passed_checks.push(format!(
            "All {} S0 claims verified — no S0 regressions",
            s0_total
        ));
    } else {
        failed_checks.push(format!(
            "{}/{} S0 claims are unverified — S0 regressions present",
            s0_unverified, s0_total
        ));
    }

    if s1_unverified == 0 {
        passed_checks.push(format!(
            "All {} S1 claims verified — no S1 regressions",
            s1_total
        ));
    } else {
        missing_proofs.push(format!(
            "{}/{} S1 claims are unverified",
            s1_unverified, s1_total
        ));
    }

    // Check evolution pallet exists
    if workspace.join("pallets/evolution-core/src/lib.rs").exists() {
        files_inspected.push("pallets/evolution-core/src/lib.rs".to_string());
        passed_checks.push("evolution-core pallet exists".to_string());
    } else {
        missing_proofs.push(
            "pallets/evolution-core/ not found — no automated regression detection pallet"
                .to_string(),
        );
    }

    let total_checks = passed_checks.len() + failed_checks.len();
    let score = if total_checks == 0 {
        0.0
    } else {
        passed_checks.len() as f64 / total_checks as f64
    };

    let status = if !failed_checks.is_empty() {
        ProofStatus::Unverified
    } else if missing_proofs.is_empty() {
        ProofStatus::Verified
    } else {
        ProofStatus::Partial
    };

    Ok(ProofResult {
        claim_id: claim_id.to_string(),
        claim: "New generations must beat prior generations without S0/S1 regressions".to_string(),
        status,
        proof_level: None,
        edge_case_level: None,
        hack_level: None,
        operator_level: None,
        degraded_level: None,
        files_inspected,
        commands_run: vec![],
        passed_checks,
        failed_checks,
        missing_proofs,
        blockers: vec![],
        score,
        evidence,
        timestamp: Utc::now(),
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

fn unrecognized_claim(claim_id: &str) -> ProofResult {
    ProofResult {
        claim_id: claim_id.to_string(),
        claim: "Unrecognized operational claim".to_string(),
        status: ProofStatus::Unverified,
        proof_level: None,
        edge_case_level: None,
        hack_level: None,
        operator_level: None,
        degraded_level: None,
        files_inspected: vec![],
        commands_run: vec![],
        passed_checks: vec![],
        failed_checks: vec!["Claim ID not handled by operational runner".to_string()],
        missing_proofs: vec![],
        blockers: vec!["Operational runner does not handle this claim".to_string()],
        score: 0.0,
        evidence: HashMap::new(),
        timestamp: Utc::now(),
        duration_ms: 0,
    }
}
