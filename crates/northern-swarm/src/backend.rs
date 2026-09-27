use crate::types::{NorthernSwarmError, TaskKind, TaskPayload};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use x3_accel::{sha256_with_parity, AccelBackend};

/// Execution backend contract for off-chain swarm work.
///
/// Consensus never trusts this interface directly: only result hashes are
/// committed on-chain and quorum/replay decides acceptance.
pub trait ComputeBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn supports(&self, kind: &TaskKind) -> bool;
    fn execute(&self, payload: &TaskPayload) -> Result<Vec<u8>, NorthernSwarmError>;
}

/// Canonical CPU backend. This is the deterministic reference/fallback.
#[derive(Default)]
pub struct CpuBackend;

impl ComputeBackend for CpuBackend {
    fn name(&self) -> &'static str {
        "cpu"
    }

    fn supports(&self, kind: &TaskKind) -> bool {
        !matches!(kind, TaskKind::AiInference)
    }

    fn execute(&self, payload: &TaskPayload) -> Result<Vec<u8>, NorthernSwarmError> {
        let input = canonical_input(payload)?;
        let mut hasher = Sha256::new();
        hasher.update(input);
        Ok(hasher.finalize().to_vec())
    }
}

/// GPU hash/compute backend using X3's vendor-neutral accelerator layer.
///
/// The accelerator layer independently recomputes SHA-256 on CPU and refuses a
/// divergent result, so a bad device/driver cannot silently alter a work hash.
pub struct GpuBackend {
    backend: Box<dyn AccelBackend>,
}

impl GpuBackend {
    pub fn new() -> Self {
        // Strict mode matters: if this build/host cannot initialize the GPU
        // backend, return BackendUnavailable rather than silently calling CPU.
        // TaskExecutor owns the explicit fallback policy.
        Self {
            backend: x3_accel::select_backend_with("wgpu", true),
        }
    }
}

impl Default for GpuBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputeBackend for GpuBackend {
    fn name(&self) -> &'static str {
        self.backend.name()
    }

    fn supports(&self, kind: &TaskKind) -> bool {
        matches!(kind, TaskKind::Compute | TaskKind::X3LangAgent)
    }

    fn execute(&self, payload: &TaskPayload) -> Result<Vec<u8>, NorthernSwarmError> {
        let input = canonical_input(payload)?;
        let output = sha256_with_parity(self.backend.as_ref(), &[input]).map_err(|error| {
            NorthernSwarmError::ExecutionFailed {
                task_id: payload.task_id.clone(),
                reason: format!("GPU backend {} unavailable/refused: {error}", self.name()),
            }
        })?;
        Ok(output[0].to_vec())
    }
}

fn canonical_input(payload: &TaskPayload) -> Result<Vec<u8>, NorthernSwarmError> {
    let canonical_params: BTreeMap<_, _> = payload.params.iter().collect();
    let encoded = serde_json::to_vec(&(
        &payload.kind,
        payload.body.as_slice(),
        canonical_params,
        payload.input_uri.as_deref(),
    ))
    .map_err(|error| NorthernSwarmError::ExecutionFailed {
        task_id: payload.task_id.clone(),
        reason: format!("canonical input serialization failed: {error}"),
    })?;

    let mut input = b"X3-NORTHERN-SWARM-DETERMINISTIC-V2\0".to_vec();
    input.extend_from_slice(&encoded);
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn payload(kind: TaskKind) -> TaskPayload {
        TaskPayload {
            task_id: "00".repeat(32),
            kind,
            body: b"deterministic-work".to_vec(),
            params: HashMap::new(),
            input_uri: None,
        }
    }

    #[test]
    fn cpu_backend_is_explicit_fallback_for_deterministic_work() {
        let cpu = CpuBackend;
        let result = cpu.execute(&payload(TaskKind::Compute)).unwrap();
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn ai_inference_is_not_faked_by_hash_backend() {
        let cpu = CpuBackend;
        assert!(!cpu.supports(&TaskKind::AiInference));
        let gpu = GpuBackend::new();
        assert!(!gpu.supports(&TaskKind::AiInference));
    }
}
