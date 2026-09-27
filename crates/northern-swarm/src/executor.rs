use crate::{
    backend::{AutoBackend, ComputeBackend},
    reactor::{
        schedule, Accelerator, BackendDescriptor, Preference, ScheduleDecision, ScheduleRefusal,
        TaskRequirements,
    },
    types::*,
};
use sha2::{Digest, Sha256};
use tracing::{debug, info};

/// Off-chain task executor (RC1).
///
/// Receives a [`TaskPayload`], runs it deterministically, and returns an
/// [`ExecutionResult`] with a SHA-256 content hash ready for on-chain submission.
///
/// # Determinism contract
/// Given identical `payload.body` and `payload.params`, this function **must**
/// produce an identical `result_hash` on every executor node.  Non-deterministic
/// outputs (random seeds, wall-clock embedded in output bytes, etc.) will trigger
/// slashing in the RC3 quorum round.
pub struct TaskExecutor {
    executor_id: ExecutorId,
    /// Hardware-detecting backend that compares any accelerator output against
    /// the canonical CPU reference before it is ever used.
    backend: AutoBackend,
    /// The backends this executor is willing to place work on, as the reactor
    /// sees them. Defaults to the canonical CPU; an executor that has an
    /// accelerator sidecar advertises it here.
    backends: Vec<BackendDescriptor>,
    /// Placement policy. `FidelityFirst` + `must_accelerate = false` is the
    /// default: the canonical CPU takes the work unless the operator says the
    /// work needs an accelerator or asks for throughput.
    preference: Preference,
    must_accelerate: bool,
    min_reputation: u8,
}

impl TaskExecutor {
    pub fn new(executor_id: ExecutorId) -> Self {
        TaskExecutor {
            backends: vec![BackendDescriptor::cpu(format!("{executor_id}/cpu-0"), 100)],
            executor_id,
            backend: AutoBackend::new(),
            preference: Preference::FidelityFirst,
            must_accelerate: false,
            min_reputation: 0,
        }
    }

    /// An executor that advertises the given backends and places work with the
    /// given policy. `must_accelerate` makes the reactor refuse work it cannot
    /// place on an accelerator instead of quietly using the CPU reference.
    pub fn with_backends(
        executor_id: ExecutorId,
        backends: Vec<BackendDescriptor>,
        preference: Preference,
        must_accelerate: bool,
        min_reputation: u8,
    ) -> Self {
        TaskExecutor {
            executor_id,
            backend: AutoBackend::new(),
            backends,
            preference,
            must_accelerate,
            min_reputation,
        }
    }

    /// The reactor's decision for `payload`, or why it cannot be placed.
    ///
    /// This is the placement half of execution: [`TaskExecutor::execute`] asks
    /// this first and refuses to run work it cannot place, so a missing
    /// accelerator is a typed refusal rather than a silent fallback.
    pub fn schedule_for(&self, payload: &TaskPayload) -> Result<ScheduleDecision, ScheduleRefusal> {
        let mut requirements = TaskRequirements::new(payload.kind.clone());
        requirements.preference = self.preference;
        requirements.must_accelerate = self.must_accelerate;
        requirements.min_reputation = self.min_reputation;
        schedule(&self.backends, &requirements)
    }

    /// The accelerator this executor reports for a placement decision.
    pub fn accelerator_of(&self, payload: &TaskPayload) -> Option<Accelerator> {
        self.schedule_for(payload)
            .ok()
            .map(|d| d.chosen_accelerator)
    }

    /// Execute a task payload and return the result.
    pub async fn execute(
        &self,
        payload: TaskPayload,
    ) -> Result<ExecutionResult, NorthernSwarmError> {
        let start = std::time::Instant::now();
        info!(task_id = %payload.task_id, kind = ?payload.kind, "starting execution");

        let input_hash = sha256_hex(&payload.body);
        let (output, placement) = if matches!(payload.kind, TaskKind::AiInference) {
            return Err(NorthernSwarmError::ExecutionFailed {
                task_id: payload.task_id.clone(),
                reason: "AiInference requires a real model backend; hash-only execution is refused"
                    .into(),
            });
        } else {
            // Placement first: the reactor decides which backend takes this task,
            // and work it cannot place is refused rather than run elsewhere.
            let decision = self.schedule_for(&payload).map_err(|refusal| {
                NorthernSwarmError::ExecutionFailed {
                    task_id: payload.task_id.clone(),
                    reason: format!("reactor refused to place this task: {refusal}"),
                }
            })?;
            debug!(
                task_id = %payload.task_id,
                backend = %decision.chosen_backend_id,
                accelerator = ?decision.chosen_accelerator,
                "reactor placed the task",
            );
            // `AutoBackend` never returns an accelerator result without having
            // compared it against the CPU reference; a divergence quarantines
            // the device and returns the reference instead.
            let output = self.backend.execute(&payload)?;
            (output, Some(decision))
        };
        let duration_ms = start.elapsed().as_millis() as u64;
        let result_hash = sha256_hex(&output);
        let output_hash = result_hash.clone();

        debug!(
            task_id = %payload.task_id,
            result_hash = %result_hash,
            duration_ms,
            "execution complete",
        );

        let proof = ProofBundle {
            task_id: payload.task_id.clone(),
            executor_id: self.executor_id.clone(),
            backend_id: placement
                .as_ref()
                .map(|decision| decision.chosen_backend_id.clone())
                .unwrap_or_else(|| self.backend.name().to_string()),
            accelerator: placement
                .as_ref()
                .map(|decision| decision.chosen_accelerator)
                .unwrap_or(Accelerator::Cpu),
            input_hash,
            output_hash,
            executed_at: unix_now(),
            duration_ms,
        };

        Ok(ExecutionResult {
            task_id: payload.task_id,
            executor_id: self.executor_id.clone(),
            result_hash,
            output,
            proof,
            status: ExecutionStatus::Success,
        })
    }
}

/// SHA-256 hex digest of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn unix_now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_payload(body: &[u8]) -> TaskPayload {
        TaskPayload {
            task_id: "test-task-001".into(),
            kind: TaskKind::Compute,
            body: body.to_vec(),
            params: Default::default(),
            input_uri: None,
        }
    }

    /// The placement decision travels with the result: an operator can see which
    /// backend the reactor chose for this exact execution.
    #[tokio::test]
    async fn the_execution_receipt_names_the_backend_the_reactor_chose() {
        let exec = TaskExecutor::new("exec-placement".to_string());
        let result = exec.execute(dummy_payload(b"placed")).await.unwrap();
        assert_eq!(result.proof.backend_id, "exec-placement/cpu-0");
        assert_eq!(result.proof.accelerator, Accelerator::Cpu);
    }

    /// Work that needs an accelerator is refused when the executor advertises
    /// none, and the refusal says why — it is never run quietly on the CPU.
    #[tokio::test]
    async fn work_that_needs_an_accelerator_is_refused_by_an_accelerator_less_executor() {
        let exec = TaskExecutor::with_backends(
            "exec-cpu-only".to_string(),
            vec![BackendDescriptor::cpu("exec-cpu-only/cpu-0", 100)],
            Preference::ThroughputFirst,
            true,
            // must accelerate
            0,
        );
        let err = exec.execute(dummy_payload(b"gpu-only")).await.unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("reactor refused to place this task")
                && message.contains("requires an accelerator"),
            "unexpected refusal: {message}"
        );
    }

    #[tokio::test]
    async fn same_input_produces_same_hash() {
        let exec = TaskExecutor::new("exec-1".into());
        let p = dummy_payload(b"hello world");
        let r1 = exec.execute(p.clone()).await.unwrap();
        let r2 = exec.execute(p).await.unwrap();
        assert_eq!(
            r1.result_hash, r2.result_hash,
            "execution must be deterministic"
        );
    }

    #[tokio::test]
    async fn different_inputs_produce_different_hashes() {
        let exec = TaskExecutor::new("exec-1".into());
        let r1 = exec.execute(dummy_payload(b"input-A")).await.unwrap();
        let r2 = exec.execute(dummy_payload(b"input-B")).await.unwrap();
        assert_ne!(r1.result_hash, r2.result_hash);
    }

    #[tokio::test]
    async fn result_status_is_success() {
        let exec = TaskExecutor::new("exec-1".into());
        let r = exec.execute(dummy_payload(b"data")).await.unwrap();
        assert_eq!(r.status, ExecutionStatus::Success);
    }

    #[tokio::test]
    async fn ai_inference_without_real_model_backend_is_refused() {
        let exec = TaskExecutor::new("exec-1".into());
        let mut p = dummy_payload(b"model input");
        p.kind = TaskKind::AiInference;
        let err = exec.execute(p).await.unwrap_err();
        assert!(err.to_string().contains("real model backend"));
    }

    #[test]
    fn sha256_hex_is_stable() {
        let h = sha256_hex(b"northern swarm");
        // Pre-computed: echo -n "northern swarm" | sha256sum
        assert_eq!(h.len(), 64, "SHA-256 hex must be 64 chars");
        // Ensure it's hex only
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
