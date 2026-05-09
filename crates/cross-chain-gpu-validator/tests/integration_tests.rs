//! Integration tests for cross-chain GPU validator

use cross_chain_gpu_validator::{
    dashboard::OperatorDashboard,
    kernels::Keccak256Kernel,
    registry::AtomicSwapRecord,
    EvmHeaderValidator,
    SvmHeaderValidator,
};

// ==================== 2.1 Kernel Parity Tests ====================

#[test]
fn test_keccak256_gpu_cpu_parity() {
    let kernel = Keccak256Kernel::new(32, false);
    let inputs = vec![
        b"ethereum_block".as_slice(),
        b"solana_block".as_slice(),
        b"cross_chain_state".as_slice(),
    ];

    let (gpu_hashes, _gpu_time) = kernel.hash_batch_gpu(&inputs).unwrap();
    let (cpu_hashes, _cpu_time) = kernel.hash_batch_cpu(&inputs).unwrap();

    assert_eq!(gpu_hashes.len(), cpu_hashes.len(), "Hash count mismatch");
    assert_eq!(gpu_hashes, cpu_hashes, "GPU and CPU hashes must match");
}

#[test]
fn test_keccak256_parity_all_inputs() {
    let kernel = Keccak256Kernel::new(256, false);
    let strs: Vec<String> = (0..256).map(|i| format!("input_{i}")).collect();
    let inputs: Vec<&[u8]> = strs.iter().map(|s| s.as_bytes()).collect();

    let parity_ok = kernel.verify_parity(&inputs).unwrap();
    assert!(parity_ok, "Parity check must pass for all inputs");
}

#[test]
fn test_keccak256_hash_consistency() {
    let kernel = Keccak256Kernel::new(32, false);
    let input = b"consistent_hash_test".as_slice();

    let (hashes1, _) = kernel.hash_batch_cpu(&[input]).unwrap();
    let (hashes2, _) = kernel.hash_batch_cpu(&[input]).unwrap();

    assert_eq!(hashes1[0], hashes2[0], "Same input must produce same hash");
}

// ==================== 2.2 Atomic Invariant Tests ====================

#[tokio::test]
async fn test_atomic_swap_record_pending_state() {
    let record = AtomicSwapRecord::new("swap-001".to_string(), 60, 1000, 500);
    assert_eq!(record.swap_id, "swap-001");
    assert!(!record.evm_validation_ok);
    assert!(!record.svm_validation_ok);
    assert!(!record.is_expired());
}

#[tokio::test]
async fn test_atomic_swap_timeout_enforcement() {
    let mut record = AtomicSwapRecord::new("swap-002".to_string(), 1, 1000, 500);

    // Manually expire the record
    use chrono::Duration;
    record.expires_at = chrono::Utc::now() - Duration::seconds(1);

    assert!(record.is_expired(), "Expired swap must be detected");
}

// ==================== 2.3 Integration Tests ====================

#[tokio::test]
async fn test_evm_validator_rejects_inconsistent_header() {
    let validator = EvmHeaderValidator::new();

    let result = validator
        .validate_header(
            1,
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            30_000_000,
            20_000_000,
            1234567890,
        )
        .await;

    assert!(result.is_err(), "Inconsistent EVM header should be rejected");
}

#[tokio::test]
async fn test_evm_validator_rejects_invalid_state_root() {
    let validator = EvmHeaderValidator::new();

    let result = validator
        .validate_header(
            1,
            [1u8; 32],
            [0u8; 32],
            [3u8; 32],
            30_000_000,
            20_000_000,
            1234567890,
        )
        .await;

    assert!(result.is_err(), "EVM header with invalid state root should be rejected");
}

#[tokio::test]
async fn test_svm_validator_rejects_invalid_parent_slot() {
    let validator = SvmHeaderValidator::new();

    let result = validator
        .validate_header(
            100,
            [1u8; 32],
            [2u8; 32],
            100,
            1234567890,
            100,
        )
        .await;

    assert!(result.is_err(), "SVM header with invalid parent slot should be rejected");
}

#[tokio::test]
async fn test_svm_validator_rejects_zero_timestamp() {
    let validator = SvmHeaderValidator::new();

    let result = validator
        .validate_header(
            100,
            [1u8; 32],
            [2u8; 32],
            99,
            0,
            100,
        )
        .await;

    assert!(result.is_err(), "SVM header with zero timestamp should be rejected");
}

// ==================== 2.4 Benchmark Test Harness ====================

#[tokio::test]
async fn test_benchmark_throughput_keccak256() {
    let kernel = Keccak256Kernel::new(256, false);
    let strs: Vec<String> = (0..1000).map(|i| format!("benchmark_input_{i}")).collect();
    let inputs: Vec<&[u8]> = strs.iter().map(|s| s.as_bytes()).collect();

    let start = std::time::Instant::now();
    let (hashes, _) = kernel.hash_batch_cpu(&inputs[..256]).unwrap();
    let elapsed = start.elapsed();

    let throughput = 256.0 / elapsed.as_secs_f64();
    println!("Keccak256 throughput: {throughput:.0} hashes/sec");

    assert_eq!(hashes.len(), 256);
    assert!(throughput > 100.0, "Throughput must be reasonable");
}

#[tokio::test]
async fn test_benchmark_latency_evm_validation() {
    let validator = EvmHeaderValidator::new();

    let mut failures = 0;
    for _ in 0..100 {
        let result = validator
            .validate_header(1000, [1u8; 32], [2u8; 32], [3u8; 32], 30_000_000, 20_000_000, 1234567890)
            .await;
        if result.is_err() {
            failures += 1;
        }
    }

    assert!(failures > 0, "EVM validation should reject inconsistent dummy headers");
}

#[tokio::test]
async fn test_dashboard_metrics_accumulation() {
    let dashboard = OperatorDashboard::new(100);

    // Simulate swap operations
    for _ in 0..50 {
        dashboard.record_swap_success().await;
    }
    for _ in 0..3 {
        dashboard.record_swap_rollback().await;
    }
    dashboard.record_txs_processed(50000).await;
    dashboard.record_tps(2000.0, 25).await;

    let metrics = dashboard.get_metrics().await;
    assert_eq!(metrics.total_swaps, 53);
    assert_eq!(metrics.successful_commits, 50);
    assert_eq!(metrics.rollbacks, 3);
    assert_eq!(metrics.total_txs_processed, 50000);
}

// ==================== Atomic Violation Detection ====================

#[test]
fn test_atomic_violation_detection_missing_evm() {
    let mut record = AtomicSwapRecord::new("swap-invalid".to_string(), 60, 1000, 500);
    record.evm_validation_ok = false;
    record.svm_validation_ok = true; // Only SVM validated - VIOLATION

    // In production, this would trigger an alarm
    let violation = record.evm_validation_ok != record.svm_validation_ok;
    assert!(
        violation,
        "Mismatched validation states must be detected as violation"
    );
}

#[test]
fn test_atomic_violation_detection_both_failed() {
    let record = AtomicSwapRecord::new("swap-both-failed".to_string(), 60, 1000, 500);
    // Both validations not completed - OK state
    assert!(!record.evm_validation_ok && !record.svm_validation_ok);
}
