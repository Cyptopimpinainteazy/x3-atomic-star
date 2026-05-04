//! X3 GPU Validator Swarm
//!
//! A deterministic GPU validator swarm with CPU verification, replay mode,
//! and quarantine/fallback mechanisms for production deployments.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                    X3 GPU Validator Swarm                                │
//! │  ┌─────────────────────────────────────────────────────────────────────┐ │
//! │  │                     Swarm Orchestrator                               │ │
//! │  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────────┐  │ │
//! │  │  │ Task Queue  │  │ Scheduler   │  │ Verification Engine         │  │ │
//! │  │  └──────┬──────┘  └──────┬──────┘  └───────────┬───────────────┘  │ │
//! │  └─────────┼────────────────┼─────────────────────┼──────────────────┘ │
//! │            │                │                     │                     │
//! │      ┌─────▼─────┬──────────▼──────────┬──────────▼─────┐             │
//! │      │           │                     │                │             │
//! │  ┌───▼───┐   ┌───▼───┐            ┌────▼────┐      ┌────▼────┐        │
//! │  │Validator│  │Validator│   ...    │Validator│      │Validator│        │
//! │  │GPU:A   │  │GPU:B   │            │GPU:X    │      │GPU:Y    │        │
//! │  └───┬───┘   └───┬───┘            └────┬────┘      └────┬────┘        │
//! │      │           │                     │                │             │
//! │      └───────────┴─────────────────────┴────────────────┘             │
//! │                         │                                             │
//! │              ┌───────────▼───────────┐                                  │
//! │              │  CPU Verification    │                                  │
//! │              │  + Replay Mode       │                                  │
//! │              └─────────────────────┘                                  │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Features
//!
//! - **Deterministic GPU Execution**: Bit-for-bit deterministic outputs
//! - **CPU Verification**: Every GPU result verified by CPU
//! - **Replay Mode**: Re-run computation for divergence detection
//! - **Quarantine System**: Isolate misbehaving validators
//! - **Fallback Mechanism**: Automatic CPU fallback on divergence
//! - **Swarm Orchestration**: Coordinate multiple validators
//! - **One-Command Onboarding**: Install, run, join, and benchmark with single commands
//! - **JSON Benchmarks**: Machine-readable performance reports
//! - **Full Telemetry**: Prometheus metrics and health monitoring

#![allow(
    dead_code,
    unused_imports,
    unused_variables,
    unused_mut,
    non_snake_case,
    unexpected_cfgs,
    unused_parens,
    non_camel_case_types,
    clippy::all
)]
pub mod config;
pub mod cpu_validator;
pub mod crypto;
pub mod deterministic;
pub mod error;
pub mod gpu_fallback_chain;
pub mod gpu_memory_pool;
pub mod gpu_receipt;
pub mod health;
pub mod metrics;
pub mod multi_gpu_dispatcher;
pub mod network;
pub mod orchestrator;
pub mod payment;
pub mod protocol;
pub mod quarantine;
pub mod telemetry;
pub mod validator;
pub mod x3_kernel_versioning;

pub use config::{SwarmConfig, ValidatorConfig};
pub use cpu_validator::{
    validate_cpu, validate_cpu_batch, validate_cpu_with, CpuTaskResult, CpuValidator,
    CpuValidatorMetrics, EasyCpuValidator,
};
pub use crypto::{
    blake2b, compute_hash, keccak256, keccak256_batch, sha256, HashAlgorithm,
    HashAlgorithm as CryptoHashAlgorithm, HashOutput, SignatureOutput, VerificationResult,
};
pub use deterministic::{DeterministicEngine, ExecutionMode, VerificationLevel};
pub use error::{SwarmError, SwarmResult};
pub use gpu_fallback_chain::{DegradationStrategy, FallbackChain, FallbackStats};
pub use gpu_memory_pool::{GpuMemoryManager, GpuMemoryPool, MemoryPoolStats, SlabHandle};
pub use gpu_receipt::{GpuClass, GpuReceipt, GpuReceiptValidator, ProofType};
pub use metrics::{HealthCheck, HealthStatus, MetricsCollector, SwarmMetrics, ValidatorHealth};
pub use multi_gpu_dispatcher::{GpuDeviceInfo, JobResult, MultiGpuDispatcher, PerformanceStats};
pub use network::{
    Network, NetworkConfig, NetworkEvent, NetworkManager, NetworkMessage, NetworkPeer,
};
pub use orchestrator::{OrchestratorEvent, SwarmOrchestrator};
pub use payment::{PaymentSystem, ProviderAccount, ProviderStatus, WorkRecord, WorkType};
pub use protocol::{SwarmMessage, TaskAssignment, TaskResult, ValidatorMessage, ValidatorProof};
pub use quarantine::{DivergenceRecord, QuarantineManager, QuarantineReason};
pub use telemetry::{TelemetryConfig, TelemetrySink};
pub use validator::{Validator, ValidatorEvent, ValidatorState};
pub use x3_kernel_versioning::{X3KernelManifest, X3KernelRegistry, X3KernelRuntime};

/// Current version of the X3 GPU Validator Swarm protocol
pub const PROTOCOL_VERSION: u32 = 3;

/// Maximum task payload size (16 MB)
pub const MAX_TASK_SIZE: usize = 16 * 1024 * 1024;

/// Default task timeout (5 minutes)
pub const DEFAULT_TASK_TIMEOUT_SECS: u64 = 300;

/// Minimum stake required to participate as a validator (in X3 tokens)
pub const MIN_VALIDATOR_STAKE: u64 = 1000;

/// Maximum number of validators in the swarm
pub const MAX_VALIDATORS: usize = 256;

/// Quarantine duration for divergence (30 minutes)
pub const QUARANTINE_DURATION_SECS: u64 = 1800;

/// Maximum replay attempts before permanent quarantine
pub const MAX_REPLAY_ATTEMPTS: u32 = 3;
