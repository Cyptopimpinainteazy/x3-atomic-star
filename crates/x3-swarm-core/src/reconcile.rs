//! Settling two replicas of one task queue after a partition.
//!
//! Two [`SwarmScheduler`] replicas that could not see each other can both hand
//! out the same task. Neither replica can tell which holder is right, so this
//! module does not guess: every conflicting claim is put to a [`ClaimArbiter`],
//! and the chain is that arbiter. `pallet-northern-swarm`'s `claim_task` lets up
//! to `MaxExecutorsPerTask` executors claim one task (a quorum), and records the
//! **first** of them in the task's `claimed_by`. That first claimer is the holder
//! both replicas converge on; every other local holder is reported as evicted so
//! an operator can stop the work it is doing.
//!
//! What [`reconcile`] does, per task id in either replica:
//!
//! - **Only one replica has the task**: it is copied, with its claim, into the
//!   other.
//! - **One side finished it** (`Passed`/`Failed`) and the other did not: the
//!   finished record wins, and a different holder on the unfinished side is
//!   evicted. Two *different* finished records are left alone and reported.
//! - **One side claimed it, the other still has it `Pending`**: the claim is
//!   copied across, so the other replica cannot hand the task out a second time.
//! - **Both sides claimed it for different holders**: the arbiter decides.
//!   [`ClaimVerdict::HeldBy`] stamps that holder on both sides;
//!   [`ClaimVerdict::Unclaimed`] means neither holder claimed it on chain, so
//!   both are evicted and the task goes back to `Pending`;
//!   [`ClaimVerdict::Unavailable`] leaves both replicas untouched and reports the
//!   task, because a reconciler that cannot ask must not pick.
//! - **The records disagree about the work itself** (anything other than status
//!   and holder): left alone and reported. Neither side is more authoritative
//!   about a task's definition than the other.
//!
//! A task reported in [`ReconcileReport::unresolved`] is exactly as it was on
//! both sides; every other task is identical on both sides afterwards.

use crate::genesis::AgentId;
use crate::scheduler::SwarmScheduler;
use crate::{AgentTask, TaskStatus};
use std::collections::BTreeSet;

/// What the arbiter knows about who holds a task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClaimVerdict {
    /// This agent holds the task: on chain, the task's first claimer.
    HeldBy(AgentId),
    /// Nobody has claimed the task.
    Unclaimed,
    /// The arbiter could not be asked (node unreachable, task not yet on chain).
    Unavailable,
}

/// The authority that settles a claim both replicas made.
pub trait ClaimArbiter {
    fn verdict(&self, task_id: &str) -> ClaimVerdict;
}

/// Which replica a line of the report is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Replica {
    A,
    B,
}

/// A local holder that lost its claim. The agent may still be working the task
/// and has to be told to stop; the replica no longer names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eviction {
    pub task_id: String,
    pub replica: Replica,
    pub agent: AgentId,
}

/// Why a task was left as it was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unresolved {
    /// Both sides claimed it for different holders and the arbiter could not
    /// answer.
    ArbiterUnavailable {
        task_id: String,
        holder_a: AgentId,
        holder_b: AgentId,
    },
    /// Both sides finished it, with different outcomes or holders.
    FinishedDifferently { task_id: String },
    /// The two records describe different work.
    RecordsDiffer { task_id: String },
}

/// Everything [`reconcile`] changed, and everything it would not.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    /// Tasks copied into a replica that did not have them.
    pub copied: Vec<(Replica, String)>,
    /// Tasks whose record or claim a replica took from the other side or from
    /// the arbiter.
    pub updated: Vec<(Replica, String)>,
    /// Holders that lost their claim.
    pub evicted: Vec<Eviction>,
    /// Tasks left untouched on both sides.
    pub unresolved: Vec<Unresolved>,
}

impl ReconcileReport {
    /// Whether the two replicas now agree on every task.
    pub fn converged(&self) -> bool {
        self.unresolved.is_empty()
    }
}

fn finished(status: TaskStatus) -> bool {
    matches!(status, TaskStatus::Passed | TaskStatus::Failed)
}

/// Whether two records describe the same work, ignoring the status that
/// claiming and finishing move.
fn same_work(a: &AgentTask, b: &AgentTask) -> bool {
    let mut b = b.clone();
    b.status = a.status;
    *a == b
}

/// The state one task converges to.
struct Settled {
    task: AgentTask,
    holder: Option<AgentId>,
}

/// Settle replica `a` and replica `b` against each other. See the module
/// documentation for the rules.
pub fn reconcile(
    a: &mut SwarmScheduler,
    b: &mut SwarmScheduler,
    arbiter: &impl ClaimArbiter,
) -> ReconcileReport {
    let mut report = ReconcileReport::default();

    // Every id once, A's queue order first, then what only B has, so two runs
    // over the same replicas produce the same report.
    let mut seen = BTreeSet::new();
    let ids: Vec<String> = a
        .tasks()
        .chain(b.tasks())
        .filter(|task| seen.insert(task.id.clone()))
        .map(|task| task.id.clone())
        .collect();

    for id in ids {
        let side_a = a.get(&id).cloned().map(|t| (t, a.claim_of(&id).copied()));
        let side_b = b.get(&id).cloned().map(|t| (t, b.claim_of(&id).copied()));

        let (task_a, holder_a, task_b, holder_b) = match (side_a, side_b) {
            (Some((task, holder)), None) => {
                b.put(task, holder);
                report.copied.push((Replica::B, id));
                continue;
            }
            (None, Some((task, holder))) => {
                a.put(task, holder);
                report.copied.push((Replica::A, id));
                continue;
            }
            (Some((ta, ha)), Some((tb, hb))) => (ta, ha, tb, hb),
            (None, None) => continue,
        };

        if task_a == task_b && holder_a == holder_b {
            continue;
        }
        if !same_work(&task_a, &task_b) {
            report
                .unresolved
                .push(Unresolved::RecordsDiffer { task_id: id });
            continue;
        }

        let settled = match (finished(task_a.status), finished(task_b.status)) {
            (true, true) => {
                report
                    .unresolved
                    .push(Unresolved::FinishedDifferently { task_id: id });
                continue;
            }
            (true, false) => Settled {
                task: task_a.clone(),
                holder: holder_a,
            },
            (false, true) => Settled {
                task: task_b.clone(),
                holder: holder_b,
            },
            (false, false) => match (holder_a, holder_b) {
                (Some(_), None) => Settled {
                    task: task_a.clone(),
                    holder: holder_a,
                },
                (None, Some(_)) => Settled {
                    task: task_b.clone(),
                    holder: holder_b,
                },
                (Some(ha), Some(hb)) if ha != hb => match arbiter.verdict(&id) {
                    ClaimVerdict::HeldBy(holder) => {
                        let mut task = task_a.clone();
                        task.status = TaskStatus::Running;
                        Settled {
                            task,
                            holder: Some(holder),
                        }
                    }
                    ClaimVerdict::Unclaimed => {
                        let mut task = task_a.clone();
                        task.status = TaskStatus::Pending;
                        Settled { task, holder: None }
                    }
                    ClaimVerdict::Unavailable => {
                        report.unresolved.push(Unresolved::ArbiterUnavailable {
                            task_id: id,
                            holder_a: ha,
                            holder_b: hb,
                        });
                        continue;
                    }
                },
                // Same holder (or none) on both sides, and neither finished:
                // only the status differs, e.g. `Pending` against `Blocked`.
                // Neither side is the authority on that.
                _ => {
                    report
                        .unresolved
                        .push(Unresolved::RecordsDiffer { task_id: id });
                    continue;
                }
            },
        };

        for (replica, scheduler, task, holder) in [
            (Replica::A, &mut *a, &task_a, holder_a),
            (Replica::B, &mut *b, &task_b, holder_b),
        ] {
            if let Some(agent) = holder {
                if settled.holder != Some(agent) {
                    report.evicted.push(Eviction {
                        task_id: id.clone(),
                        replica,
                        agent,
                    });
                }
            }
            if *task != settled.task || holder != settled.holder {
                scheduler.put(settled.task.clone(), settled.holder);
                report.updated.push((replica, id.clone()));
            }
        }
    }
    report
}
