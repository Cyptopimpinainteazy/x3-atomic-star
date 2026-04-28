use crate::proof::*;
use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// Verify asset kernel supply conservation claim
pub async fn verify_claim(workspace: &Path, claim_id: &str, verbose: bool) -> Result<ProofResult> {
    let start = Instant::now();

    if verbose {
        println!("  → Verifying asset kernel supply conservation...");
    }

    let mut files_inspected = vec![];
    let mut commands_run = vec![];
    let mut passed_checks = vec![];
    let mut failed_checks = vec![];
    let mut missing_proofs = vec![];
    let mut blockers = vec![];
    let mut evidence = HashMap::new();

    // Search for canonical_supply tests
    let search_pattern = "canonical_supply";
    let grep_output = Command::new("grep")
        .args(["-r", search_pattern, "pallets/x3-kernel/", "--include=*.rs"])
        .current_dir(workspace)
        .output();

    match grep_output {
        Ok(output) => {
            let matches = String::from_utf8_lossy(&output.stdout);
            if matches.is_empty() {
                missing_proofs.push("canonical_supply test not found".to_string());
                blockers.push("Missing canonical_supply test implementation".to_string());
            } else {
                passed_checks.push("canonical_supply test found".to_string());
                evidence.insert("canonical_supply_test".to_string(), "found".to_string());
            }
        }
        Err(e) => {
            if verbose {
                println!("    Warning: grep search failed: {}", e);
            }
        }
    }

    // Check for test files
    let find_output = Command::new("find")
        .args(["pallets/x3-kernel/src/", "-name", "*test*.rs"])
        .current_dir(workspace)
        .output();

    match find_output {
        Ok(output) => {
            let test_files = String::from_utf8_lossy(&output.stdout);
            let test_file_count = test_files.lines().count();

            for test_file in test_files.lines() {
                files_inspected.push(test_file.to_string());
            }

            evidence.insert("test_files".to_string(), test_file_count.to_string());

            if test_file_count == 0 {
                missing_proofs.push("No test files found for x3-kernel".to_string());
                blockers.push("Missing test coverage for asset kernel".to_string());
            } else {
                passed_checks.push(format!("Found {} test files", test_file_count));
            }
        }
        Err(e) => {
            if verbose {
                println!("    Warning: find command failed: {}", e);
            }
        }
    }

    // Try to run tests
    if verbose {
        println!("    Running tests...");
    }

    let test_cmd = "cargo test --package x3-kernel canonical_supply";
    commands_run.push(test_cmd.to_string());

    let test_output = Command::new("cargo")
        .args(["test", "--package", "x3-kernel", "canonical_supply"])
        .current_dir(workspace)
        .output();

    let mut test_passed = false;
    match test_output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if verbose {
                println!("    Test output (last 10 lines):");
                let lines: Vec<&str> = stdout.lines().collect();
                for line in lines.iter().rev().take(10).rev() {
                    println!("      {}", line);
                }
            }

            if output.status.success() {
                test_passed = true;
                passed_checks.push("canonical_supply tests PASSED".to_string());
                evidence.insert("test_result".to_string(), "PASSED".to_string());
            } else {
                failed_checks.push("canonical_supply tests FAILED".to_string());
                evidence.insert("test_result".to_string(), "FAILED".to_string());
                blockers.push("canonical_supply tests failing".to_string());
            }

            // Count test cases
            let test_count = stdout.matches("test result").count();
            evidence.insert("test_cases_run".to_string(), test_count.to_string());
        }
        Err(e) => {
            failed_checks.push(format!("Failed to run tests: {}", e));
            blockers.push("Cannot execute tests".to_string());
        }
    }

    // Search for invariant proofs in code
    let invariant_search = Command::new("grep")
        .args(["-r", "invariant:", "pallets/x3-kernel/", "--include=*.rs"])
        .current_dir(workspace)
        .output();

    if let Ok(output) = invariant_search {
        let matches = String::from_utf8_lossy(&output.stdout);
        let invariant_count = matches.lines().count();

        evidence.insert(
            "invariants_declared".to_string(),
            invariant_count.to_string(),
        );

        if invariant_count > 0 {
            passed_checks.push(format!("Found {} invariant declarations", invariant_count));
        } else {
            missing_proofs.push("No invariant declarations found".to_string());
        }
    }

    // Check for supply monitoring
    let monitor_search = Command::new("grep")
        .args([
            "-r",
            "supply_monitor",
            "pallets/x3-kernel/",
            "--include=*.rs",
        ])
        .current_dir(workspace)
        .output();

    if let Ok(output) = monitor_search {
        let matches = String::from_utf8_lossy(&output.stdout);
        if !matches.is_empty() {
            passed_checks.push("Supply monitoring code found".to_string());
            evidence.insert("supply_monitor".to_string(), "found".to_string());
        } else {
            missing_proofs.push("Supply monitoring not implemented".to_string());
        }
    }

    // Determine status
    let status = if !blockers.is_empty() {
        ProofStatus::Failed
    } else if test_passed && !missing_proofs.is_empty() {
        ProofStatus::Partial
    } else if test_passed {
        ProofStatus::Verified
    } else {
        ProofStatus::Unverified
    };

    // Calculate score (0.0 - 1.0)
    let score = if test_passed {
        let base_score = 0.5;
        let has_invariants = evidence
            .get("invariants_declared")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0)
            > 0;
        let has_monitor = evidence.contains_key("supply_monitor");

        base_score
            + (if has_invariants { 0.25 } else { 0.0 })
            + (if has_monitor { 0.25 } else { 0.0 })
    } else {
        0.0
    };

    let result = ProofResult {
        claim_id: claim_id.to_string(),
        claim: "Asset kernel maintains canonical supply invariant across all operations"
            .to_string(),
        status,
        proof_level: Some(ProofLevel::P2), // L2: unit + integration tests
        edge_case_level: Some(EdgeCaseLevel::E1),
        hack_level: Some(HackLevel::H0),
        operator_level: Some(OperatorLevel::I1),
        degraded_level: Some(DegradedLevel::D1),
        files_inspected,
        commands_run,
        passed_checks,
        failed_checks,
        missing_proofs,
        blockers,
        score,
        evidence,
        timestamp: Utc::now(),
        duration_ms: start.elapsed().as_millis() as u64,
    };

    if verbose {
        println!("    Status: {:?}", result.status);
        println!("    Score: {:.2}", result.score);
        println!(
            "    Passed: {} | Failed: {}",
            result.passed_checks.len(),
            result.failed_checks.len()
        );
    }

    Ok(result)
}

pub async fn run_proofs(workspace: &Path, verbose: bool) -> Result<ProofResult> {
    verify_claim(workspace, "x3.asset_kernel.supply_conservation", verbose).await
}
