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

pub mod config;
pub mod cpu_validator;
pub mod crypto;
pub mod deterministic;
pub mod error;
#[cfg(feature = "gpu-validators")]
pub mod gpu_bytecode;
#[cfg(feature = "gpu-validators")]
pub mod gpu_fallback_chain;
#[cfg(feature = "gpu-validators")]
pub mod gpu_memory_pool;
#[cfg(feature = "gpu-validators")]
pub mod gpu_receipt;
#[cfg(feature = "gpu-validators")]
pub mod health;
pub mod metrics;
#[cfg(feature = "gpu-validators")]
pub mod multi_gpu_dispatcher;
#[cfg(feature = "gpu-validators")]
pub mod network;
#[cfg(feature = "gpu-validators")]
pub mod orchestrator;
#[cfg(feature = "gpu-validators")]
pub mod payment;
#[cfg(feature = "gpu-validators")]
pub mod proof_aggregator;
#[cfg(feature = "gpu-validators")]
pub mod proof_integration;
#[cfg(feature = "gpu-validators")]
pub mod protocol;
#[cfg(feature = "gpu-validators")]
pub mod quarantine;
#[cfg(feature = "gpu-validators")]
pub mod state_merkle_proof;
#[cfg(feature = "gpu-validators")]
pub mod telemetry;
#[cfg(feature = "gpu-validators")]
pub mod unified_proof;
#[cfg(feature = "gpu-validators")]
pub mod validator;
#[cfg(feature = "gpu-validators")]
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
#[cfg(feature = "gpu-validators")]
pub use gpu_fallback_chain::{DegradationStrategy, FallbackChain, FallbackStats};
#[cfg(feature = "gpu-validators")]
pub use gpu_memory_pool::{GpuMemoryManager, GpuMemoryPool, MemoryPoolStats, SlabHandle};
#[cfg(feature = "gpu-validators")]
pub use gpu_receipt::{GpuClass, GpuReceipt, GpuReceiptValidator, ProofType};
pub use metrics::{HealthCheck, HealthStatus, MetricsCollector, SwarmMetrics, ValidatorHealth};
#[cfg(feature = "gpu-validators")]
pub use multi_gpu_dispatcher::{GpuDeviceInfo, JobResult, MultiGpuDispatcher, PerformanceStats};
#[cfg(feature = "gpu-validators")]
pub use network::{
    Network, NetworkConfig, NetworkEvent, NetworkManager, NetworkMessage, NetworkPeer,
};
#[cfg(feature = "gpu-validators")]
pub use orchestrator::{OrchestratorEvent, SwarmOrchestrator};
#[cfg(feature = "gpu-validators")]
pub use payment::{PaymentSystem, ProviderAccount, ProviderStatus, WorkRecord, WorkType};
#[cfg(feature = "gpu-validators")]
pub use proof_aggregator::{AggregationState, AggregatorStats, ProofAggregator};
#[cfg(feature = "gpu-validators")]
pub use protocol::{SwarmMessage, TaskAssignment, TaskResult, ValidatorMessage, ValidatorProof};
#[cfg(feature = "gpu-validators")]
pub use quarantine::{DivergenceRecord, QuarantineManager, QuarantineReason};
#[cfg(feature = "gpu-validators")]
pub use state_merkle_proof::{
    compute_merkle_root, generate_merkle_proof, MerkleNode, MerkleProofPath, StateMerkleProof,
    StateRootVerification,
};
#[cfg(feature = "gpu-validators")]
pub use telemetry::{TelemetryConfig, TelemetrySink};
#[cfg(feature = "gpu-validators")]
pub use unified_proof::{
    AtomicVmProof, ByzantineConsensus, GpuValidatorAttestation, ProofHeader, ProofValidationResult,
    UnifiedProof,
};
#[cfg(feature = "gpu-validators")]
pub use validator::{Validator, ValidatorEvent, ValidatorState};
#[cfg(feature = "gpu-validators")]
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
