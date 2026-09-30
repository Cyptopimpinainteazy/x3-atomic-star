//! Fault schedule.
//!
//! Faults are chosen from the same seeded stream as the workload and stored as
//! a sorted plan, so "when does the node crash" is part of what `--seed` pins
//! down.

use crate::network::NodeId;
use crate::rng::SimRng;
use crate::Scenario;

/// One injected fault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaultKind {
    /// Cut all links between two groups of nodes.
    Partition {
        group_a: Vec<NodeId>,
        group_b: Vec<NodeId>,
    },
    /// Restore all links.
    Heal,
    /// The coordinator process dies and restarts from persistence.
    CrashAndRestart { duration_ms: u64 },
    /// Degrade every link.
    SlowLink {
        latency_ms: u64,
        extra_drop_percent: u32,
    },
    /// The most recent persisted write for one session disappears.
    StaleWrite { session: usize },
}

#[derive(Debug, Clone)]
pub struct FaultEvent {
    pub at_ms: u64,
    pub kind: FaultKind,
    pub fired: bool,
}

/// A time-ordered fault plan.
#[derive(Debug, Clone)]
pub struct FaultPlan {
    events: Vec<FaultEvent>,
}

impl FaultPlan {
    /// Build the plan for a scenario.
    ///
    /// `horizon_ms` is the expected length of the run; faults land inside it so
    /// that every scenario actually experiences what it advertises.
    pub fn generate(
        scenario: Scenario,
        rng: &mut SimRng,
        clients: usize,
        sessions: usize,
        start_ms: u64,
        horizon_ms: u64,
    ) -> Self {
        let mut events: Vec<FaultEvent> = Vec::new();
        let mut push = |at_ms: u64, kind: FaultKind| {
            events.push(FaultEvent {
                at_ms,
                kind,
                fired: false,
            });
        };

        let window = |frac_percent: u64| start_ms + (horizon_ms * frac_percent) / 100;

        match scenario {
            Scenario::HappyPath => {}
            Scenario::ClaimRefundRace => {
                let cut = window(40);
                push(
                    cut,
                    FaultKind::Partition {
                        group_a: vec![0],
                        group_b: (1..=clients).collect(),
                    },
                );
                push(cut + horizon_ms / 10, FaultKind::Heal);
            }
            Scenario::PartitionStorm => {
                for (i, frac) in [12u64, 30, 52, 74].iter().enumerate() {
                    let at = window(*frac);
                    push(
                        at,
                        FaultKind::Partition {
                            group_a: vec![0],
                            group_b: (1..=clients).collect(),
                        },
                    );
                    push(at + horizon_ms / 20 + (i as u64) * 7, FaultKind::Heal);
                }
                push(
                    window(20),
                    FaultKind::SlowLink {
                        latency_ms: 2_500,
                        extra_drop_percent: 20,
                    },
                );
                push(window(65), FaultKind::SlowLink {
                    latency_ms: 25,
                    extra_drop_percent: 0,
                });
            }
            Scenario::CrashRecovery => {
                push(
                    window(25),
                    FaultKind::CrashAndRestart {
                        duration_ms: 4_000,
                    },
                );
                push(
                    window(60),
                    FaultKind::CrashAndRestart {
                        duration_ms: 2_000,
                    },
                );
                if sessions > 0 {
                    push(
                        window(45),
                        FaultKind::StaleWrite {
                            session: rng.below(sessions),
                        },
                    );
                }
            }
        }

        events.sort_by_key(|e| e.at_ms);
        Self { events }
    }

    /// Take every event due at or before `now_ms`, marking them fired.
    pub fn due(&mut self, now_ms: u64) -> Vec<FaultKind> {
        let mut out = Vec::new();
        for event in self.events.iter_mut() {
            if !event.fired && event.at_ms <= now_ms {
                event.fired = true;
                out.push(event.kind.clone());
            }
        }
        out
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}
