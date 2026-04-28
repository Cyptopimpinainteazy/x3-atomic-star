use crate::proof::*;
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

/// Verify GPU validator parity claim.
///
/// Checks:
///   cpu_gpu_parity — GPU-accelerated execution matches CPU reference results
pub async fn verify_claim(
    workspace: &PathBuf,
    claim_id: &str,
    verbose: bool,
) -> Result<ProofResult> {
    let start = Instant::now();

    if verbose {
        println!("  → Verifying GPU parity claim: {}", claim_id);
    }

    let mut files_inspected = vec![];
    let mut commands_run = vec![];
    let mut passed_checks = vec![];
    let mut failed_checks = vec![];
    let mut missing_proofs = vec![];
    let mut blockers = vec![];
    let mut evidence = HashMap::new();

    // ── 1. Check key GPU source files exist ──────────────────────────────────
    let key_files = [
        "crates/x3-gpu-validator-swarm/src/lib.rs",
        "crates/x3-gpu-validator-swarm/src/deterministic.rs",
        "crates/x3-gpu-validator-swarm/src/cpu_validator.rs",
    ];
    for path in &key_files {
        if workspace.join(path).exists() {
            files_inspected.push(path.to_string());
            passed_checks.push(format!("{} exists", path));
        } else {
            failed_checks.push(format!("{} missing", path));
        }
    }

    // ── 2. Grep for CPU/GPU parity evidence ──────────────────────────────────
    let parity_grep = Command::new("grep")
        .args([
            "-rn",
            "cpu_result\\|cpu_fallback\\|deterministic\\|parity",
            "crates/x3-gpu-validator-swarm/src/",
        ])
        .current_dir(workspace)
        .output();

    match parity_grep {
        Ok(out) => {
            let hits = String::from_utf8_lossy(&out.stdout);
            let count = hits.lines().count();
            evidence.insert("parity_references".to_string(), count.to_string());
            if count > 0 {
                passed_checks.push(format!("{} CPU/GPU parity references found", count));
            } else {
                failed_checks.push("No CPU/GPU parity references found".to_string());
                blockers.push("GPU parity logic not found in codebase".to_string());
            }
        }
        Err(e) => {
            if verbose {
                eprintln!("    Warning: grep failed: {}", e);
            }
        }
    }

    // ── 3. Run the GPU validator swarm tests ─────────────────────────────────
    // Scope to deterministic module only; payment/versioning failures are unrelated
    let cmd_str = "cargo test -p x3-gpu-validator-swarm -- deterministic --quiet";
    commands_run.push(cmd_str.to_string());

    if verbose {
        println!("    Running: {}", cmd_str);
    }

    let test_out = Command::new("cargo")
        .args([
            "test",
            "-p",
            "x3-gpu-validator-swarm",
            "--",
            "deterministic",
            "--quiet",
        ])
        .current_dir(workspace)
        .output();

    match test_out {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            if out.status.success() {
                let test_count = stdout
                    .lines()
                    .chain(stderr.lines())
                    .filter(|l| l.contains("test ") && l.contains("ok"))
                    .count();
                passed_checks.push(format!("GPU validator tests passed ({} tests)", test_count));
                evidence.insert("tests_passed".to_string(), test_count.to_string());

                // Look specifically for parity/deterministic test names in output
                let has_parity_test = stdout.contains("deterministic")
                    || stdout.contains("parity")
                    || stderr.contains("deterministic")
                    || stderr.contains("parity");
                if has_parity_test {
                    passed_checks.push("Deterministic/parity test confirmed in output".to_string());
                } else {
                    missing_proofs.push(
                        "No test explicitly named 'parity' or 'deterministic' in output; \
                         add test_cpu_gpu_parity that asserts CPU and GPU results match"
                            .to_string(),
                    );
                }
            } else {
                let err_snippet = stderr.lines().take(5).collect::<Vec<_>>().join(" | ");
                failed_checks.push(format!("GPU validator tests failed: {}", err_snippet));
                blockers.push("x3-gpu-validator-swarm tests are failing".to_string());
            }
        }
        Err(e) => {
            failed_checks.push(format!("Could not run cargo test: {}", e));
            missing_proofs.push("cargo could not execute GPU validator tests".to_string());
        }
    }

    // ── 4. Check that CPU fallback path is compiled in ───────────────────────
    let fallback_grep = Command::new("grep")
        .args([
            "-n",
            "cpu_fallback\\|CpuFallback\\|FallbackToCpu",
            "crates/x3-gpu-validator-swarm/src/deterministic.rs",
        ])
        .current_dir(workspace)
        .output();

    if let Ok(out) = fallback_grep {
        let hits = String::from_utf8_lossy(&out.stdout);
        if !hits.is_empty() {
            passed_checks.push("CPU fallback path found in deterministic.rs".to_string());
            evidence.insert("cpu_fallback".to_string(), "present".to_string());
        } else {
            missing_proofs.push(
                "No explicit CPU fallback path in deterministic.rs — \
                 GPU parity requires the CPU path to be the reference"
                    .to_string(),
            );
        }
    }

    // ── 5. Score and status ──────────────────────────────────────────────────
    let total_checks = passed_checks.len() + failed_checks.len();
    let score = if total_checks == 0 {
        0.0
    } else {
        passed_checks.len() as f64 / total_checks as f64
    };

    let status = if !blockers.is_empty() {
        ProofStatus::Failed
    } else if failed_checks.is_empty() && missing_proofs.is_empty() {
        ProofStatus::Verified
    } else if score >= 0.5 {
        ProofStatus::Partial
    } else {
        ProofStatus::Unverified
    };

    Ok(ProofResult {
        claim_id: claim_id.to_string(),
        claim: "GPU-accelerated execution/verification matches CPU reference results".to_string(),
        status,
        proof_level: None,
        edge_case_level: None,
        hack_level: None,
        operator_level: None,
        degraded_level: None,
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
    })
}
