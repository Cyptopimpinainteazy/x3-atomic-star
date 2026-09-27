//! The reactor: deciding *where* a task runs, deterministically.
//!
//! The crate's backends answer "can this host run the task, and does its output
//! match the canonical CPU reference". This module answers the question before
//! that one: given the work's requirements and the backends a node is
//! advertising, which backend should take it — and why.
//!
//! The policy is deliberately boring and total:
//!
//! 1. A candidate must serve the task's [`TaskKind`], be available, meet the
//!    reputation floor, and be inside the cost and latency ceilings when the
//!    caller set them. Anything else is disqualified **with the reason
//!    recorded**, so "no backend" is never an unexplained answer.
//! 2. Work that `must_accelerate` disqualifies every CPU candidate. If that
//!    leaves nothing, the reactor **refuses** ([`ScheduleRefusal::AcceleratorUnavailable`])
//!    rather than quietly running it on the reference CPU — the optional GPU
//!    sidecar is a dependency, and a missing dependency is a refusal, not a
//!    silent substitution.
//! 3. Survivors are ordered by [`Preference`]: `FidelityFirst` puts the
//!    canonical CPU first (the reference is the truth; acceleration is an
//!    optimisation), `ThroughputFirst` puts accelerators first. Ties break by
//!    reputation descending, cost ascending, latency ascending, then backend id
//!    ascending — a total order, so two runs over the same inputs agree.
//!
//! Every non-chosen candidate is reported as `NotPreferred` or
//! `Disqualified(..)`, so a decision is evidence rather than an assertion.
//! Nothing here decides *results*: [`crate::backend::AutoBackend`] still
//! compares any accelerator output against the CPU reference, and consensus
//! still accepts only what quorum verifies.

use crate::types::TaskKind;
use serde::{Deserialize, Serialize};

/// Where a task can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accelerator {
    Cpu,
    Gpu,
    Npu,
    Fpga,
}

impl Accelerator {
    /// The canonical reference executes on the CPU.
    pub fn is_canonical(self) -> bool {
        matches!(self, Accelerator::Cpu)
    }
}

/// What the caller optimises for. A policy knob, not a heuristic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preference {
    /// Prefer the canonical CPU; use an accelerator only when CPU cannot serve.
    FidelityFirst,
    /// Prefer the fastest accelerator; parity checking is still enforced at run time.
    ThroughputFirst,
}

/// What a node advertises about one of its backends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendDescriptor {
    /// Stable identity of this backend on this node (`cpu-0`, `gpu-0`, ..., or a
    /// worker account id when the reactor is scheduling across nodes).
    pub id: String,
    pub accelerator: Accelerator,
    /// Task kinds this backend can serve.
    pub kinds: Vec<TaskKind>,
    /// Whether the backend is usable *right now* (worker online, sidecar present).
    pub available: bool,
    /// Reputation 0-100, as the on-chain record reports it.
    pub reputation: u8,
    /// Quoted price per unit of work; unit defined by the caller's market.
    pub cost_per_unit: u64,
    /// Expected latency for this backend, milliseconds.
    pub latency_ms: u32,
}

impl BackendDescriptor {
    /// A descriptor that serves `kinds` on `accelerator`.
    pub fn new(
        id: impl Into<String>,
        accelerator: Accelerator,
        kinds: Vec<TaskKind>,
        available: bool,
        reputation: u8,
        cost_per_unit: u64,
        latency_ms: u32,
    ) -> Self {
        Self {
            id: id.into(),
            accelerator,
            kinds,
            available,
            reputation,
            cost_per_unit,
            latency_ms,
        }
    }

    /// The canonical CPU backend, which serves everything the executor can serve.
    pub fn cpu(id: impl Into<String>, reputation: u8) -> Self {
        Self::new(
            id,
            Accelerator::Cpu,
            vec![
                TaskKind::Compute,
                TaskKind::DataFetch,
                TaskKind::X3LangAgent,
                TaskKind::Other(0),
            ],
            true,
            reputation,
            0,
            0,
        )
    }

    /// Whether this backend can serve `kind`.
    ///
    /// `TaskKind::Other(_)` listed on a descriptor matches *every* `Other`
    /// variant: the pallet's escape hatch is one family, not a set of specific
    /// ids, so a descriptor that advertises it is not silently limited to
    /// `Other(0)`.
    pub fn serves(&self, kind: &TaskKind) -> bool {
        self.kinds.iter().any(|listed| match (listed, kind) {
            (TaskKind::Other(_), TaskKind::Other(_)) => true,
            (listed, kind) => listed == kind,
        })
    }
}

/// What the work needs before it can be placed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskRequirements {
    pub kind: TaskKind,
    /// True for work that only a real accelerator may serve.
    pub must_accelerate: bool,
    pub preference: Preference,
    pub min_reputation: u8,
    pub max_cost_per_unit: Option<u64>,
    pub max_latency_ms: Option<u32>,
}

impl TaskRequirements {
    pub fn new(kind: TaskKind) -> Self {
        Self {
            kind,
            must_accelerate: false,
            preference: Preference::FidelityFirst,
            min_reputation: 0,
            max_cost_per_unit: None,
            max_latency_ms: None,
        }
    }
}

/// Why a candidate cannot take the work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Disqualification {
    /// The backend does not serve this kind of task.
    WrongKind,
    /// The backend is offline, or the sidecar it needs is not running.
    Unavailable,
    /// The task may only run on a real accelerator.
    AcceleratorRequired,
    ReputationTooLow {
        got: u8,
        needed: u8,
    },
    TooExpensive {
        got: u64,
        limit: u64,
    },
    TooSlow {
        got: u32,
        limit: u32,
    },
}

/// Why an eligible candidate was not chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreferenceReason {
    /// The preference tier put a canonical (or accelerated) backend first.
    LowerTierUnderPreference,
    /// Same tier, but another candidate had a better reputation.
    LowerReputation,
    /// Same tier and reputation, but another candidate was cheaper.
    MoreExpensive,
    /// Same tier, reputation and price, but another candidate was faster.
    Slower,
    /// Identical on every measured axis; the lower backend id won for stability.
    HigherIdTieBreak,
}

/// What happened to one candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateOutcome {
    Chosen,
    Disqualified(Disqualification),
    NotPreferred(PreferenceReason),
}

/// One candidate's outcome, kept for the report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub backend_id: String,
    pub accelerator: Accelerator,
    pub outcome: CandidateOutcome,
}

/// The scheduling decision, as evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleDecision {
    pub kind: TaskKind,
    pub chosen_backend_id: String,
    pub chosen_accelerator: Accelerator,
    pub preference: Preference,
    /// Every candidate considered, the chosen one first.
    pub candidates: Vec<Candidate>,
}

impl ScheduleDecision {
    /// The disqualifications, for a caller that wants only the failures.
    pub fn disqualifications(&self) -> impl Iterator<Item = (&str, &Disqualification)> {
        self.candidates
            .iter()
            .filter_map(|candidate| match &candidate.outcome {
                CandidateOutcome::Disqualified(reason) => {
                    Some((candidate.backend_id.as_str(), reason))
                }
                _ => None,
            })
    }
}

/// Why no backend could take the work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleRefusal {
    /// Nothing passed the filters, or no backend was advertised at all.
    NoBackend {
        kind: TaskKind,
        candidates: Vec<Candidate>,
    },
    /// The work requires an accelerator and none is advertised as available.
    AcceleratorUnavailable {
        kind: TaskKind,
        candidates: Vec<Candidate>,
    },
}

impl ScheduleRefusal {
    /// Every candidate considered, with its reason.
    pub fn candidates(&self) -> &[Candidate] {
        match self {
            ScheduleRefusal::NoBackend { candidates, .. }
            | ScheduleRefusal::AcceleratorUnavailable { candidates, .. } => candidates,
        }
    }
}

impl core::fmt::Display for ScheduleRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ScheduleRefusal::NoBackend { kind, candidates } => {
                write!(
                    f,
                    "no backend serves {kind:?} on this node ({})",
                    summarize(candidates)
                )
            }
            ScheduleRefusal::AcceleratorUnavailable { kind, candidates } => write!(
                f,
                "{kind:?} requires an accelerator and none is available ({})",
                summarize(candidates)
            ),
        }
    }
}

fn summarize(candidates: &[Candidate]) -> String {
    if candidates.is_empty() {
        return "no backends advertised".to_string();
    }
    candidates
        .iter()
        .map(|candidate| match &candidate.outcome {
            CandidateOutcome::Chosen => format!("{}: chosen", candidate.backend_id),
            CandidateOutcome::Disqualified(reason) => {
                format!("{}: {reason:?}", candidate.backend_id)
            }
            CandidateOutcome::NotPreferred(reason) => {
                format!("{}: {reason:?}", candidate.backend_id)
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Preference tier: 0 is best under the given preference.
fn tier(accelerator: Accelerator, preference: Preference) -> u8 {
    match (preference, accelerator.is_canonical()) {
        (Preference::FidelityFirst, true) => 0,
        (Preference::FidelityFirst, false) => 1,
        (Preference::ThroughputFirst, true) => 1,
        (Preference::ThroughputFirst, false) => 0,
    }
}

fn disqualify(
    descriptor: &BackendDescriptor,
    requirements: &TaskRequirements,
) -> Option<Disqualification> {
    if !descriptor.serves(&requirements.kind) {
        return Some(Disqualification::WrongKind);
    }
    if requirements.must_accelerate && descriptor.accelerator.is_canonical() {
        return Some(Disqualification::AcceleratorRequired);
    }
    if !descriptor.available {
        return Some(Disqualification::Unavailable);
    }
    if descriptor.reputation < requirements.min_reputation {
        return Some(Disqualification::ReputationTooLow {
            got: descriptor.reputation,
            needed: requirements.min_reputation,
        });
    }
    if let Some(limit) = requirements.max_cost_per_unit {
        if descriptor.cost_per_unit > limit {
            return Some(Disqualification::TooExpensive {
                got: descriptor.cost_per_unit,
                limit,
            });
        }
    }
    if let Some(limit) = requirements.max_latency_ms {
        if descriptor.latency_ms > limit {
            return Some(Disqualification::TooSlow {
                got: descriptor.latency_ms,
                limit,
            });
        }
    }
    None
}

/// Choose a backend, or explain why none can take the work.
pub fn schedule(
    backends: &[BackendDescriptor],
    requirements: &TaskRequirements,
) -> Result<ScheduleDecision, ScheduleRefusal> {
    let mut disqualified: Vec<Candidate> = Vec::new();
    let mut eligible: Vec<&BackendDescriptor> = Vec::new();

    for descriptor in backends {
        match disqualify(descriptor, requirements) {
            Some(reason) => disqualified.push(Candidate {
                backend_id: descriptor.id.clone(),
                accelerator: descriptor.accelerator,
                outcome: CandidateOutcome::Disqualified(reason),
            }),
            None => {
                eligible.push(descriptor);
            }
        }
    }

    if eligible.is_empty() {
        let candidates = disqualified;
        return Err(if requirements.must_accelerate {
            ScheduleRefusal::AcceleratorUnavailable {
                kind: requirements.kind.clone(),
                candidates,
            }
        } else {
            ScheduleRefusal::NoBackend {
                kind: requirements.kind.clone(),
                candidates,
            }
        });
    }

    eligible.sort_by(|left, right| {
        tier(left.accelerator, requirements.preference)
            .cmp(&tier(right.accelerator, requirements.preference))
            .then(right.reputation.cmp(&left.reputation))
            .then(left.cost_per_unit.cmp(&right.cost_per_unit))
            .then(left.latency_ms.cmp(&right.latency_ms))
            .then(left.id.cmp(&right.id))
    });
    let chosen = eligible[0];

    // Every eligible candidate's reason is computed against the winner, so the
    // report says which axis decided it rather than "somebody else won".
    let mut candidates: Vec<Candidate> = eligible
        .iter()
        .map(|descriptor| Candidate {
            backend_id: descriptor.id.clone(),
            accelerator: descriptor.accelerator,
            outcome: if descriptor.id == chosen.id {
                CandidateOutcome::Chosen
            } else {
                CandidateOutcome::NotPreferred(preference_reason(
                    chosen,
                    descriptor,
                    requirements.preference,
                ))
            },
        })
        .collect();
    candidates.extend(disqualified);

    // The chosen candidate leads, then the rest in the order they were considered.
    candidates.sort_by_key(|candidate| !matches!(candidate.outcome, CandidateOutcome::Chosen));

    Ok(ScheduleDecision {
        kind: requirements.kind.clone(),
        chosen_backend_id: chosen.id.clone(),
        chosen_accelerator: chosen.accelerator,
        preference: requirements.preference,
        candidates,
    })
}

fn preference_reason(
    chosen: &BackendDescriptor,
    other: &BackendDescriptor,
    preference: Preference,
) -> PreferenceReason {
    if tier(chosen.accelerator, preference) != tier(other.accelerator, preference) {
        return PreferenceReason::LowerTierUnderPreference;
    }
    if chosen.reputation != other.reputation {
        return PreferenceReason::LowerReputation;
    }
    if chosen.cost_per_unit != other.cost_per_unit {
        return PreferenceReason::MoreExpensive;
    }
    if chosen.latency_ms != other.latency_ms {
        return PreferenceReason::Slower;
    }
    PreferenceReason::HigherIdTieBreak
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(id: &str, reputation: u8, cost: u64, latency: u32) -> BackendDescriptor {
        BackendDescriptor::new(
            id,
            Accelerator::Gpu,
            vec![TaskKind::Compute, TaskKind::X3LangAgent],
            true,
            reputation,
            cost,
            latency,
        )
    }

    #[test]
    fn fidelity_first_places_work_on_the_canonical_cpu() {
        let backends = [BackendDescriptor::cpu("cpu-0", 90), gpu("gpu-0", 99, 0, 1)];
        let decision = schedule(&backends, &TaskRequirements::new(TaskKind::Compute)).unwrap();
        assert_eq!(decision.chosen_backend_id, "cpu-0");
        assert!(decision.chosen_accelerator.is_canonical());
        // The accelerator is eligible but not preferred, and the report says so.
        let gpu_outcome = decision
            .candidates
            .iter()
            .find(|candidate| candidate.backend_id == "gpu-0")
            .unwrap();
        assert_eq!(
            gpu_outcome.outcome,
            CandidateOutcome::NotPreferred(PreferenceReason::LowerTierUnderPreference)
        );
        assert_eq!(decision.candidates[0].outcome, CandidateOutcome::Chosen);
    }

    #[test]
    fn throughput_first_places_work_on_the_accelerator() {
        let backends = [BackendDescriptor::cpu("cpu-0", 90), gpu("gpu-0", 99, 5, 1)];
        let mut requirements = TaskRequirements::new(TaskKind::Compute);
        requirements.preference = Preference::ThroughputFirst;
        let decision = schedule(&backends, &requirements).unwrap();
        assert_eq!(decision.chosen_backend_id, "gpu-0");
        assert_eq!(decision.chosen_accelerator, Accelerator::Gpu);
        let cpu_outcome = decision
            .candidates
            .iter()
            .find(|candidate| candidate.backend_id == "cpu-0")
            .unwrap();
        assert_eq!(
            cpu_outcome.outcome,
            CandidateOutcome::NotPreferred(PreferenceReason::LowerTierUnderPreference)
        );
    }

    /// The optional-sidecar case: work that needs an accelerator must be refused
    /// with the reason attached, never quietly run on the CPU reference.
    #[test]
    fn accelerator_only_work_is_refused_when_no_accelerator_is_advertised() {
        let backends = [BackendDescriptor::cpu("cpu-0", 100), gpu_offline("gpu-0")];
        let mut requirements = TaskRequirements::new(TaskKind::Compute);
        requirements.must_accelerate = true;
        let refusal = schedule(&backends, &requirements).unwrap_err();
        match &refusal {
            ScheduleRefusal::AcceleratorUnavailable { candidates, .. } => {
                assert_eq!(candidates.len(), 2);
                assert_eq!(
                    candidates[0].outcome,
                    CandidateOutcome::Disqualified(Disqualification::AcceleratorRequired),
                    "the CPU is disqualified because the work demands an accelerator"
                );
                assert_eq!(
                    candidates[1].outcome,
                    CandidateOutcome::Disqualified(Disqualification::Unavailable),
                    "the GPU is present but its sidecar is not running"
                );
            }
            other => panic!("expected AcceleratorUnavailable, got {other:?}"),
        }
        assert!(refusal.to_string().contains("requires an accelerator"));
        assert!(ScheduleRefusal::NoBackend {
            kind: TaskKind::Compute,
            candidates: vec![]
        }
        .to_string()
        .contains("no backends advertised"));
    }

    fn gpu_offline(id: &str) -> BackendDescriptor {
        BackendDescriptor::new(
            id,
            Accelerator::Gpu,
            vec![TaskKind::Compute],
            false,
            100,
            0,
            1,
        )
    }

    #[test]
    fn ceilings_and_reputation_disqualify_with_the_numbers() {
        let backends = [
            gpu("gpu-expensive", 90, 500, 10),
            gpu("gpu-slow", 90, 10, 9_000),
            gpu("gpu-unproven", 20, 10, 10),
            gpu("gpu-ok", 80, 20, 20),
        ];
        let mut requirements = TaskRequirements::new(TaskKind::Compute);
        requirements.preference = Preference::ThroughputFirst;
        requirements.min_reputation = 50;
        requirements.max_cost_per_unit = Some(100);
        requirements.max_latency_ms = Some(1_000);

        let decision = schedule(&backends, &requirements).unwrap();
        assert_eq!(decision.chosen_backend_id, "gpu-ok");
        let reasons: Vec<(&str, &Disqualification)> = decision.disqualifications().collect();
        assert_eq!(
            reasons,
            vec![
                (
                    "gpu-expensive",
                    &Disqualification::TooExpensive {
                        got: 500,
                        limit: 100
                    }
                ),
                (
                    "gpu-slow",
                    &Disqualification::TooSlow {
                        got: 9_000,
                        limit: 1_000
                    }
                ),
                (
                    "gpu-unproven",
                    &Disqualification::ReputationTooLow {
                        got: 20,
                        needed: 50
                    }
                ),
            ]
        );
    }

    #[test]
    fn a_kind_no_backend_serves_is_refused_for_that_kind() {
        let backends = [BackendDescriptor::cpu("cpu-0", 90)];
        let refusal =
            schedule(&backends, &TaskRequirements::new(TaskKind::AiInference)).unwrap_err();
        match &refusal {
            ScheduleRefusal::NoBackend { kind, candidates } => {
                assert_eq!(kind, &TaskKind::AiInference);
                assert_eq!(
                    candidates[0].outcome,
                    CandidateOutcome::Disqualified(Disqualification::WrongKind)
                );
            }
            other => panic!("expected NoBackend, got {other:?}"),
        }
    }

    /// Determinism: identical descriptors on every measured axis are separated by
    /// the backend id alone, so two runs over the same inputs agree.
    #[test]
    fn a_tie_breaks_on_the_backend_id_and_the_decision_is_evidence() {
        let backends = [gpu("gpu-b", 90, 10, 5), gpu("gpu-a", 90, 10, 5)];
        let mut requirements = TaskRequirements::new(TaskKind::Compute);
        requirements.preference = Preference::ThroughputFirst;
        let first = schedule(&backends, &requirements).unwrap();
        let second = schedule(&backends, &requirements).unwrap();
        assert_eq!(first, second, "the same inputs must give the same decision");
        assert_eq!(first.chosen_backend_id, "gpu-a");
        let loser = first
            .candidates
            .iter()
            .find(|candidate| candidate.backend_id == "gpu-b")
            .unwrap();
        assert_eq!(
            loser.outcome,
            CandidateOutcome::NotPreferred(PreferenceReason::HigherIdTieBreak)
        );

        // The decision survives a round trip as JSON, which is how it travels
        // into a benchmark report or an execution receipt.
        let json = serde_json::to_string(&first).unwrap();
        let decoded: ScheduleDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, first);
        assert!(json.contains("\"throughput_first\""));
    }

    #[test]
    fn a_descriptor_advertising_other_serves_every_other_variant() {
        let backends = [BackendDescriptor::cpu("cpu-0", 90)];
        let requirements = TaskRequirements::new(TaskKind::Other(7));
        let decision = schedule(&backends, &requirements).unwrap();
        assert_eq!(decision.chosen_backend_id, "cpu-0");
    }
}
