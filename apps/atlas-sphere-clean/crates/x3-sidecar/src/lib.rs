//! X3 Sidecar Daemon
//!
//! Off-chain swarm execution node for X3 Chain. This daemon:
//! - Connects to the swarm network
//! - Receives X3 bytecode execution jobs
//! - Executes jobs in a sandboxed VM
//! - Generates deterministic receipts with Merkle proofs
//! - Submits receipts to the on-chain verifier
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                        X3 SIDECAR DAEMON                            │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │                                                                     │
//! │  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐            │
//! │  │    RPC       │   │    Job       │   │   Receipt    │            │
//! │  │   Server     │──▶│   Queue      │──▶│  Generator   │            │
//! │  └──────────────┘   └──────────────┘   └──────────────┘            │
//! │         │                  │                  │                    │
//! │         │                  ▼                  │                    │
//! │         │         ┌──────────────┐           │                    │
//! │         │         │    X3 VM     │           │                    │
//! │         │         │  Executor    │           │                    │
//! │         │         └──────────────┘           │                    │
//! │         │                  │                  │                    │
//! │         ▼                  ▼                  ▼                    │
//! │  ┌──────────────────────────────────────────────────┐              │
//! │  │              State Manager                        │              │
//! │  │  • Merkle Tree  • Checkpoints  • Rollback        │              │
//! │  └──────────────────────────────────────────────────┘              │
//! │                           │                                        │
//! │                           ▼                                        │
//! │  ┌──────────────────────────────────────────────────┐              │
//! │  │              Chain Submitter                      │              │
//! │  │  • Receipt Submission  • Gas Estimation          │              │
//! │  └──────────────────────────────────────────────────┘              │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```

pub mod config;
pub mod executor;
pub mod job;
pub mod receipt;
pub mod rpc;
pub mod state;
pub mod submitter;
pub mod telemetry;

use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

pub use config::SidecarConfig;
pub use executor::X3Executor;
pub use job::{Job, JobQueue};
pub use receipt::{ExecutionReceipt, ReceiptGenerator};
pub use state::StateManager;
pub use submitter::ChainSubmitter;
pub use telemetry::Telemetry;

/// Tracked status for an execution job.
#[derive(Clone, Debug)]
pub struct JobStatusEntry {
    /// Human-readable status label.
    pub status: String,
    /// Last update timestamp (unix seconds).
    pub updated_at_unix: u64,
    /// Transaction hash when submitted on-chain.
    pub tx_hash: Option<String>,
    /// Last error message if any.
    pub error: Option<String>,
}

impl JobStatusEntry {
    pub fn new(status: impl Into<String>, tx_hash: Option<String>, error: Option<String>) -> Self {
        let updated_at_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            status: status.into(),
            updated_at_unix,
            tx_hash,
            error,
        }
    }
}

/// Sidecar state (shared across components)
pub struct SidecarState {
    pub start_time: Instant,
    pub jobs_completed: u64,
    pub jobs_failed: u64,
    pub registered: bool,
    pub job_statuses: std::collections::HashMap<[u8; 32], JobStatusEntry>,
}

impl Default for SidecarState {
    fn default() -> Self {
        Self {
            start_time: Instant::now(),
            jobs_completed: 0,
            jobs_failed: 0,
            registered: false,
            job_statuses: std::collections::HashMap::new(),
        }
    }
}

/// Sidecar daemon
pub struct SidecarDaemon {
    pub config: SidecarConfig,
    pub job_queue: Arc<RwLock<JobQueue>>,
    pub executor: Arc<X3Executor>,
    pub state_manager: Arc<RwLock<StateManager>>,
    pub receipt_generator: Arc<ReceiptGenerator>,
    pub submitter: Arc<ChainSubmitter>,
    pub telemetry: Arc<Telemetry>,
    pub state: Arc<RwLock<SidecarState>>,
}

impl SidecarDaemon {
    /// Create a new sidecar daemon
    pub fn new(config: SidecarConfig) -> anyhow::Result<Self> {
        let state_manager = Arc::new(RwLock::new(StateManager::new()));
        let executor = Arc::new(X3Executor::new(config.vm.clone()));
        let receipt_generator = Arc::new(ReceiptGenerator::from_hex(&config.executor_key)?);
        let submitter = Arc::new(ChainSubmitter::new(
            config.chain_rpc.clone(),
            config.executor_key.clone(),
        ));
        let job_queue = Arc::new(RwLock::new(JobQueue::new()));
        let telemetry = Telemetry::new();
        let state = Arc::new(RwLock::new(SidecarState::default()));

        Ok(Self {
            config,
            job_queue,
            executor,
            state_manager,
            receipt_generator,
            submitter,
            telemetry,
            state,
        })
    }

    /// Run the daemon
    pub async fn run(self: Arc<Self>) -> anyhow::Result<()> {
        info!("Starting X3 Sidecar Daemon v{}", env!("CARGO_PKG_VERSION"));
        info!("RPC server on port {}", self.config.rpc_port);
        info!("Metrics on port {}", self.config.metrics_port);

        // Build RPC state
        let rpc_state = Arc::new(rpc::RpcState {
            job_queue: Arc::clone(&self.job_queue),
            sidecar_state: Arc::clone(&self.state),
            submitter: Arc::clone(&self.submitter),
            telemetry: Arc::clone(&self.telemetry),
        });

        // Build server routers.
        let rpc_addr: std::net::SocketAddr = format!("0.0.0.0:{}", self.config.rpc_port).parse()?;
        let metrics_addr: std::net::SocketAddr =
            format!("0.0.0.0:{}", self.config.metrics_port).parse()?;
        let router = rpc::create_router(Arc::clone(&rpc_state));
        let metrics_router = rpc::create_metrics_router(rpc_state);

        info!("RPC server listening on {}", rpc_addr);
        if self.config.metrics_port != self.config.rpc_port {
            info!("Metrics server listening on {}", metrics_addr);
        }

        // Spawn job processor
        let daemon = Arc::clone(&self);
        let processor_handle = tokio::spawn(async move {
            daemon.job_processor_loop().await;
        });

        // Run HTTP servers. Keep /metrics on RPC for backward compatibility,
        // and serve a dedicated telemetry surface on `metrics_port`.
        if self.config.metrics_port == self.config.rpc_port {
            axum::Server::bind(&rpc_addr)
                .serve(router.into_make_service())
                .await?;
        } else {
            let rpc_server = axum::Server::bind(&rpc_addr).serve(router.into_make_service());
            let metrics_server =
                axum::Server::bind(&metrics_addr).serve(metrics_router.into_make_service());

            tokio::try_join!(rpc_server, metrics_server)?;
        }

        processor_handle.abort();
        Ok(())
    }

    async fn job_processor_loop(&self) {
        loop {
            // Try to get next job
            let job = {
                let mut queue = self.job_queue.write().await;
                let popped = queue.pop();
                if popped.is_some() {
                    queue.record_started();
                }
                popped
            };

            if let Some(job) = job {
                {
                    let mut state = self.state.write().await;
                    state
                        .job_statuses
                        .insert(job.id, JobStatusEntry::new("running", None, None));
                }

                let timer = telemetry::ExecutionTimer::start(Arc::clone(&self.telemetry));
                let wait_time_ms = job.submitted_at.elapsed().as_millis() as u64;

                // Create checkpoint
                {
                    let mut sm = self.state_manager.write().await;
                    sm.checkpoint();
                }

                // Execute
                match self
                    .executor
                    .execute(&job.bytecode, &job.input, job.gas_limit)
                {
                    Ok(result) => {
                        timer.complete(result.gas_used);

                        // Get pre and post state managers
                        let pre_state = StateManager::new();
                        let post_state = self.state_manager.read().await;

                        // Generate receipt
                        let receipt = self.receipt_generator.generate(
                            job.id,
                            &job.input,
                            &result,
                            &pre_state,
                            &*post_state,
                        );

                        // Submit to chain
                        match self.submitter.submit_receipt(&receipt).await {
                            Ok(tx_hash) => {
                                info!("Receipt submitted: {}", tx_hash);
                                self.telemetry.record_receipt_submitted();
                                let mut state = self.state.write().await;
                                state.jobs_completed += 1;
                                state.job_statuses.insert(
                                    job.id,
                                    JobStatusEntry::new("submitted", Some(tx_hash), None),
                                );
                            }
                            Err(e) => {
                                tracing::warn!("Failed to submit receipt: {}", e);
                                self.telemetry.record_receipt_failure();
                                let mut state = self.state.write().await;
                                state.job_statuses.insert(
                                    job.id,
                                    JobStatusEntry::new("submit_failed", None, Some(e.to_string())),
                                );
                            }
                        }

                        let mut queue = self.job_queue.write().await;
                        queue.record_completed(wait_time_ms);
                    }
                    Err(e) => {
                        let err = e.to_string();
                        tracing::error!("Job execution failed: {}", err);
                        timer.fail();

                        // Rollback state
                        let mut sm = self.state_manager.write().await;
                        sm.rollback();

                        let mut queue = self.job_queue.write().await;
                        queue.record_failed();
                        drop(queue);

                        let mut state = self.state.write().await;
                        state.jobs_failed += 1;
                        state
                            .job_statuses
                            .insert(job.id, JobStatusEntry::new("failed", None, Some(err)));
                    }
                }
            } else {
                // No jobs, sleep
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }
}

/// Initialize logging
pub fn init_logging(level: Level) -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .finish();

    // Attempt to set the global subscriber. If one is already set (typical in tests
    // or when multiple components initialize logging), just continue.
    match tracing::subscriber::set_global_default(subscriber) {
        Ok(()) => Ok(()),
        Err(_e) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::Level;

    #[test]
    fn init_logging_is_idempotent() {
        // Should succeed even if called multiple times (no panic)
        assert!(init_logging(Level::INFO).is_ok());
        assert!(init_logging(Level::DEBUG).is_ok());
    }
}
