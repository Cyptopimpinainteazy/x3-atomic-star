use crate::types::{NorthernSwarmError, TaskKind, TaskPayload};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};
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

/// Per-execution counters for [`AutoBackend`].
///
/// `accelerator_*` counters describe what happened when the accelerator was
/// consulted; `cpu_executions` counts every execution served by the CPU
/// reference, including the ones whose result was ultimately returned.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BackendTelemetry {
    pub executions: u64,
    pub accelerator_agreements: u64,
    pub accelerator_divergences: u64,
    pub accelerator_refusals: u64,
    pub cpu_executions: u64,
}

/// A recorded accelerator divergence.
///
/// The diverging output is discarded, but the fact that it happened is kept so
/// the event is auditable after the fact rather than only in a log line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    pub task_id: String,
    pub accelerator: &'static str,
    /// SHA-256 hex of the accelerator's output (the one that was rejected).
    pub accelerator_hash: String,
    /// SHA-256 hex of the CPU reference output (the one that was returned).
    pub reference_hash: String,
}

/// Hardware-detecting backend that never trusts an accelerator on its own.
///
/// The CPU backend is the deterministic reference. When an accelerator is
/// present, not quarantined and claims support for the task kind, its output is
/// compared byte-for-byte against a freshly computed CPU reference:
///
/// * **agreement** — the accelerator output is returned;
/// * **divergence** — the accelerator is quarantined, the divergence is
///   recorded, and the *CPU* result is returned;
/// * **refusal/error** — the accelerator is skipped and the CPU result is
///   returned (an explicit fallback, not a silent substitution).
///
/// There is no path that returns an accelerator result without the CPU
/// reference having been computed and compared. A quarantined accelerator stays
/// quarantined until [`AutoBackend::clear_quarantine`] is called by an operator.
pub struct AutoBackend {
    cpu: CpuBackend,
    accelerator: Option<Box<dyn ComputeBackend>>,
    quarantine: Mutex<Option<Divergence>>,
    telemetry: Mutex<BackendTelemetry>,
}

impl AutoBackend {
    /// Detect hardware and select the accelerator, if any.
    pub fn new() -> Self {
        Self::with_accelerator(Some(Box::new(GpuBackend::new())))
    }

    /// Build with an explicit accelerator (used to inject a test double).
    pub fn with_accelerator(accelerator: Option<Box<dyn ComputeBackend>>) -> Self {
        Self {
            cpu: CpuBackend,
            accelerator,
            quarantine: Mutex::new(None),
            telemetry: Mutex::new(BackendTelemetry::default()),
        }
    }

    /// Snapshot of the execution counters.
    pub fn telemetry(&self) -> BackendTelemetry {
        lock(&self.telemetry).clone()
    }

    /// The recorded divergence, if the accelerator has been quarantined.
    pub fn quarantine(&self) -> Option<Divergence> {
        lock(&self.quarantine).clone()
    }

    /// Operator action: allow a quarantined accelerator to be tried again.
    pub fn clear_quarantine(&self) {
        *lock(&self.quarantine) = None;
    }
}

impl Default for AutoBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputeBackend for AutoBackend {
    fn name(&self) -> &'static str {
        "auto"
    }

    fn supports(&self, kind: &TaskKind) -> bool {
        self.cpu.supports(kind)
    }

    fn execute(&self, payload: &TaskPayload) -> Result<Vec<u8>, NorthernSwarmError> {
        lock(&self.telemetry).executions += 1;

        // Fail closed on a kind this backend cannot serve, even if a caller
        // ignored `supports`.
        if !self.cpu.supports(&payload.kind) {
            return Err(NorthernSwarmError::ExecutionFailed {
                task_id: payload.task_id.clone(),
                reason: format!(
                    "AutoBackend does not support {:?}; refusing rather than hashing it",
                    payload.kind,
                ),
            });
        }

        // The reference is always computed: it is what any accepted output is
        // measured against.
        let reference = self.cpu.execute(payload)?;

        let accelerator = match self.accelerator.as_deref() {
            Some(accel) if lock(&self.quarantine).is_none() && accel.supports(&payload.kind) => {
                accel
            }
            _ => {
                lock(&self.telemetry).cpu_executions += 1;
                return Ok(reference);
            }
        };

        match accelerator.execute(payload) {
            Ok(candidate) if candidate == reference => {
                lock(&self.telemetry).accelerator_agreements += 1;
                Ok(candidate)
            }
            Ok(candidate) => {
                let divergence = Divergence {
                    task_id: payload.task_id.clone(),
                    accelerator: accelerator.name(),
                    accelerator_hash: hash_hex(&candidate),
                    reference_hash: hash_hex(&reference),
                };
                {
                    let mut telemetry = lock(&self.telemetry);
                    telemetry.accelerator_divergences += 1;
                    telemetry.cpu_executions += 1;
                }
                *lock(&self.quarantine) = Some(divergence);
                Ok(reference)
            }
            Err(_) => {
                let mut telemetry = lock(&self.telemetry);
                telemetry.accelerator_refusals += 1;
                telemetry.cpu_executions += 1;
                Ok(reference)
            }
        }
    }
}

/// SHA-256 hex digest, local to this module so the backend does not depend on
/// the executor.
fn hash_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// A poisoned lock is recovered rather than panicked on: the counters carry no
/// invariant that a panic could corrupt into a wrong *result* (the result is
/// decided by the byte comparison, not by the counters).
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn payload(kind: TaskKind) -> TaskPayload {
        TaskPayload {
            task_id: "00".repeat(32),
            kind,
            body: b"deterministic-work".to_vec(),
            params: HashMap::new(),
            input_uri: None,
        }
    }

    /// An accelerator that agrees with the CPU reference.
    struct AgreeingAccelerator;

    impl ComputeBackend for AgreeingAccelerator {
        fn name(&self) -> &'static str {
            "agreeing-accel"
        }
        fn supports(&self, kind: &TaskKind) -> bool {
            matches!(kind, TaskKind::Compute)
        }
        fn execute(&self, payload: &TaskPayload) -> Result<Vec<u8>, NorthernSwarmError> {
            CpuBackend.execute(payload)
        }
    }

    /// An accelerator that returns a fixed, wrong answer and counts its calls.
    struct DivergentAccelerator {
        calls: Arc<AtomicUsize>,
    }

    impl ComputeBackend for DivergentAccelerator {
        fn name(&self) -> &'static str {
            "divergent-accel"
        }
        fn supports(&self, _kind: &TaskKind) -> bool {
            true
        }
        fn execute(&self, _payload: &TaskPayload) -> Result<Vec<u8>, NorthernSwarmError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(vec![0xDE, 0xAD, 0xBE, 0xEF])
        }
    }

    /// An accelerator that claims support but cannot execute.
    struct UnavailableAccelerator;

    impl ComputeBackend for UnavailableAccelerator {
        fn name(&self) -> &'static str {
            "unavailable-accel"
        }
        fn supports(&self, _kind: &TaskKind) -> bool {
            true
        }
        fn execute(&self, payload: &TaskPayload) -> Result<Vec<u8>, NorthernSwarmError> {
            Err(NorthernSwarmError::ExecutionFailed {
                task_id: payload.task_id.clone(),
                reason: "no accelerator device".into(),
            })
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

    #[test]
    fn auto_backend_agrees_with_the_cpu_reference_on_supported_work() {
        let auto = AutoBackend::with_accelerator(Some(Box::new(AgreeingAccelerator)));
        let p = payload(TaskKind::Compute);
        let expected = CpuBackend.execute(&p).unwrap();

        let got = auto.execute(&p).unwrap();
        assert_eq!(
            got, expected,
            "an agreeing accelerator must not change the result"
        );
        assert_eq!(auto.quarantine(), None);

        let telemetry = auto.telemetry();
        assert_eq!(telemetry.executions, 1);
        assert_eq!(telemetry.accelerator_agreements, 1);
        assert_eq!(telemetry.accelerator_divergences, 0);
    }

    #[test]
    fn auto_backend_falls_back_to_cpu_when_the_accelerator_refuses() {
        let auto = AutoBackend::with_accelerator(Some(Box::new(UnavailableAccelerator)));
        let p = payload(TaskKind::Compute);
        let expected = CpuBackend.execute(&p).unwrap();

        let got = auto.execute(&p).unwrap();
        assert_eq!(
            got, expected,
            "a refused accelerator must fall back to the reference"
        );
        assert_eq!(auto.quarantine(), None, "a refusal is not a divergence");
        assert_eq!(auto.telemetry().accelerator_refusals, 1);
    }

    #[test]
    fn auto_backend_quarantines_a_divergent_accelerator_and_returns_the_cpu_result() {
        let calls = Arc::new(AtomicUsize::new(0));
        let auto = AutoBackend::with_accelerator(Some(Box::new(DivergentAccelerator {
            calls: calls.clone(),
        })));
        let p = payload(TaskKind::Compute);
        let expected = CpuBackend.execute(&p).unwrap();

        let first = auto.execute(&p).unwrap();
        assert_eq!(
            first, expected,
            "the divergence must not change the returned result"
        );

        let recorded = auto.quarantine().expect("a divergence must be recorded");
        assert_eq!(recorded.accelerator, "divergent-accel");
        assert_eq!(
            recorded.accelerator_hash,
            hash_hex(&[0xDE, 0xAD, 0xBE, 0xEF])
        );
        assert_eq!(recorded.reference_hash, hash_hex(&expected));

        let second = auto.execute(&p).unwrap();
        assert_eq!(second, expected);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a quarantined accelerator must not be consulted again",
        );
        assert_eq!(auto.telemetry().accelerator_divergences, 1);

        auto.clear_quarantine();
        assert_eq!(auto.quarantine(), None);
    }

    #[test]
    fn auto_backend_refuses_a_kind_it_cannot_serve() {
        let auto = AutoBackend::with_accelerator(None);
        let err = auto.execute(&payload(TaskKind::AiInference)).unwrap_err();
        assert!(
            err.to_string().contains("does not support"),
            "an unsupported kind must fail closed, got: {err}",
        );
    }
}
