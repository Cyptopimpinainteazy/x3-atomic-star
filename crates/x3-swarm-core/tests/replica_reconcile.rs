//! `reconcile` settles two partitioned replicas of one queue.
//!
//! The arbiter here is a table of verdicts, standing in for the chain's
//! `claimed_by` record; everything else is the real scheduler and the real
//! dispatch path (`claim_next_for`, which consults the real `SwarmAuthority`).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use x3_swarm_core::{
    reconcile, AgentKind, AgentPermissionTier, AgentTask, ClaimArbiter, ClaimVerdict,
    DispatchRefusal, Eviction, GenesisRecord, Replica, SwarmAuthority, SwarmScheduler, TaskStatus,
    Unresolved,
};

type AgentId = [u8; 32];

fn agent_id(byte: u8) -> AgentId {
    [byte; 32]
}

const CLASS: AgentKind = AgentKind::TestBuilder;

fn authority_with(ids: impl IntoIterator<Item = u8>) -> SwarmAuthority {
    let mut authority = SwarmAuthority::new();
    for byte in ids {
        let record = GenesisRecord::new(
            agent_id(byte),
            agent_id(0),
            "reconcile test agent",
            CLASS,
            AgentPermissionTier::DocsTestsReports,
            vec![],
            1,
        );
        authority.genesis_mut().create(record).unwrap();
    }
    authority
}

fn task(id: &str) -> AgentTask {
    AgentTask::new(
        id.to_string(),
        format!("task {id}"),
        "x3_swarm_core".to_string(),
        CLASS,
    )
}

/// Two replicas of the same queue, as they were before the partition.
fn replicas(ids: &[&str]) -> (SwarmScheduler, SwarmScheduler) {
    let mut a = SwarmScheduler::new();
    for id in ids {
        a.enqueue(task(id));
    }
    (a.clone(), a)
}

fn claim(scheduler: &mut SwarmScheduler, holder: u8) -> String {
    scheduler
        .claim_next_for(CLASS, &agent_id(holder), &authority_with([holder]), 10)
        .expect("a registered agent can claim on its own side")
        .id
}

type State = BTreeMap<String, (AgentTask, Option<AgentId>)>;

/// A replica's content, independent of queue order.
fn state(scheduler: &SwarmScheduler) -> State {
    scheduler
        .tasks()
        .map(|t| {
            (
                t.id.clone(),
                (t.clone(), scheduler.claim_of(&t.id).copied()),
            )
        })
        .collect()
}

/// A table of verdicts that records which task ids it was asked about.
#[derive(Default)]
struct Table {
    verdicts: BTreeMap<String, ClaimVerdict>,
    asked: RefCell<Vec<String>>,
}

impl Table {
    fn with(entries: &[(&str, ClaimVerdict)]) -> Self {
        Self {
            verdicts: entries
                .iter()
                .map(|(id, v)| (id.to_string(), v.clone()))
                .collect(),
            asked: RefCell::default(),
        }
    }
}

impl ClaimArbiter for Table {
    fn verdict(&self, task_id: &str) -> ClaimVerdict {
        self.asked.borrow_mut().push(task_id.to_string());
        self.verdicts
            .get(task_id)
            .cloned()
            .unwrap_or(ClaimVerdict::Unavailable)
    }
}

/// The partition from `race_and_partition.rs`, settled: both sides took `t-1`
/// for different agents, the chain says agent 2 claimed first, and afterwards
/// both replicas name agent 2 while agent 1 is reported for eviction.
#[test]
fn partitioned_replicas_converge_on_the_chains_first_claimer() {
    let (mut a, mut b) = replicas(&["t-1", "t-2"]);
    assert_eq!(claim(&mut a, 1), "t-1");
    assert_eq!(claim(&mut b, 2), "t-1");
    // Each side also learned something the other did not.
    a.enqueue(task("only-a"));
    b.enqueue(task("only-b"));

    let arbiter = Table::with(&[("t-1", ClaimVerdict::HeldBy(agent_id(2)))]);
    let report = reconcile(&mut a, &mut b, &arbiter);

    assert!(report.converged(), "{report:?}");
    assert_eq!(state(&a), state(&b), "both replicas hold the same queue");
    assert_eq!(a.claim_of("t-1"), Some(&agent_id(2)));
    assert_eq!(a.get("t-1").unwrap().status, TaskStatus::Running);
    assert_eq!(
        report.evicted,
        vec![Eviction {
            task_id: "t-1".to_string(),
            replica: Replica::A,
            agent: agent_id(1),
        }]
    );
    assert_eq!(report.updated, vec![(Replica::A, "t-1".to_string())]);
    assert_eq!(
        report.copied,
        vec![
            (Replica::B, "only-a".to_string()),
            (Replica::A, "only-b".to_string()),
        ]
    );
    assert_eq!(
        *arbiter.asked.borrow(),
        vec!["t-1".to_string()],
        "the arbiter is asked about the conflict and nothing else"
    );

    // The settled queue hands out the untouched task next, on either side.
    assert_eq!(claim(&mut a, 3), "t-2");
}

/// The arbiter may name neither local holder (a third executor claimed first on
/// chain): both local holders are evicted.
#[test]
fn a_third_holder_on_chain_evicts_both_local_holders() {
    let (mut a, mut b) = replicas(&["t-1"]);
    claim(&mut a, 1);
    claim(&mut b, 2);

    let arbiter = Table::with(&[("t-1", ClaimVerdict::HeldBy(agent_id(9)))]);
    let report = reconcile(&mut a, &mut b, &arbiter);

    assert_eq!(state(&a), state(&b));
    assert_eq!(b.claim_of("t-1"), Some(&agent_id(9)));
    let evicted: Vec<_> = report
        .evicted
        .iter()
        .map(|e| (e.replica, e.agent))
        .collect();
    assert_eq!(
        evicted,
        vec![(Replica::A, agent_id(1)), (Replica::B, agent_id(2))]
    );
}

/// Neither holder claimed on chain: neither may finish the task, so both are
/// evicted and the task is workable again on both sides.
#[test]
fn a_conflict_nobody_claimed_on_chain_goes_back_to_the_queue() {
    let (mut a, mut b) = replicas(&["t-1"]);
    claim(&mut a, 1);
    claim(&mut b, 2);

    let report = reconcile(
        &mut a,
        &mut b,
        &Table::with(&[("t-1", ClaimVerdict::Unclaimed)]),
    );

    assert!(report.converged());
    assert_eq!(state(&a), state(&b));
    for side in [&a, &b] {
        assert_eq!(side.claim_of("t-1"), None);
        assert_eq!(side.get("t-1").unwrap().status, TaskStatus::Pending);
    }
    assert_eq!(report.evicted.len(), 2);
    assert_eq!(claim(&mut b, 3), "t-1", "the work can be taken again");
}

/// A reconciler that cannot reach the arbiter must not pick a winner: both
/// replicas stay exactly as they were, and the conflict is reported.
#[test]
fn an_unreachable_arbiter_changes_nothing() {
    let (mut a, mut b) = replicas(&["t-1"]);
    claim(&mut a, 1);
    claim(&mut b, 2);
    let (before_a, before_b) = (state(&a), state(&b));

    let report = reconcile(&mut a, &mut b, &Table::default());

    assert!(!report.converged());
    assert_eq!(
        report.unresolved,
        vec![Unresolved::ArbiterUnavailable {
            task_id: "t-1".to_string(),
            holder_a: agent_id(1),
            holder_b: agent_id(2),
        }]
    );
    assert!(report.evicted.is_empty() && report.updated.is_empty());
    assert_eq!((state(&a), state(&b)), (before_a, before_b));
}

/// The split brain the scheduler could not prevent: a claim made on one side
/// only. Copying it across is what stops the other side handing the task out.
#[test]
fn a_one_sided_claim_is_copied_so_the_task_is_not_handed_out_twice() {
    let (mut a, mut b) = replicas(&["t-1"]);
    claim(&mut a, 1);

    let arbiter = Table::default();
    let report = reconcile(&mut a, &mut b, &arbiter);

    assert!(report.converged());
    assert_eq!(state(&a), state(&b));
    assert_eq!(b.claim_of("t-1"), Some(&agent_id(1)));
    assert!(report.evicted.is_empty());
    assert!(
        arbiter.asked.borrow().is_empty(),
        "nothing conflicted, so nothing was put to the arbiter"
    );
    assert_eq!(
        b.claim_next_for(CLASS, &agent_id(2), &authority_with([2]), 10),
        Err(DispatchRefusal::NoTask),
        "the other replica no longer hands out work that is already held"
    );
}

/// A finished record beats an unfinished one, and a different holder still
/// working it on the other side is evicted.
#[test]
fn a_finished_record_wins_and_evicts_the_other_holder() {
    let (mut a, mut b) = replicas(&["t-1"]);
    claim(&mut a, 1);
    assert!(a.update_status("t-1", TaskStatus::Passed));
    claim(&mut b, 2);

    let report = reconcile(&mut a, &mut b, &Table::default());

    assert!(report.converged());
    assert_eq!(state(&a), state(&b));
    assert_eq!(b.get("t-1").unwrap().status, TaskStatus::Passed);
    assert_eq!(b.claim_of("t-1"), Some(&agent_id(1)));
    assert_eq!(
        report.evicted,
        vec![Eviction {
            task_id: "t-1".to_string(),
            replica: Replica::B,
            agent: agent_id(2),
        }]
    );
}

/// Two sides finished the same task differently: there is no rule that makes
/// one outcome right, so both are kept and the task is reported.
#[test]
fn two_different_outcomes_are_left_alone() {
    let (mut a, mut b) = replicas(&["t-1"]);
    claim(&mut a, 1);
    a.update_status("t-1", TaskStatus::Passed);
    claim(&mut b, 2);
    b.update_status("t-1", TaskStatus::Failed);
    let before = (state(&a), state(&b));

    let report = reconcile(&mut a, &mut b, &Table::default());

    assert_eq!(
        report.unresolved,
        vec![Unresolved::FinishedDifferently {
            task_id: "t-1".to_string()
        }]
    );
    assert_eq!((state(&a), state(&b)), before);
}

/// Records that describe different work (here, a changed approval requirement)
/// are not merged, whoever holds them.
#[test]
fn records_describing_different_work_are_left_alone() {
    let (mut a, mut b) = replicas(&["t-1"]);
    let mut gated = task("t-1");
    gated.approval_required = x3_swarm_core::ApprovalRequirement::HumanReview;
    b.enqueue(gated);
    claim(&mut a, 1);
    let before = (state(&a), state(&b));

    let report = reconcile(
        &mut a,
        &mut b,
        &Table::with(&[("t-1", ClaimVerdict::HeldBy(agent_id(1)))]),
    );

    assert_eq!(
        report.unresolved,
        vec![Unresolved::RecordsDiffer {
            task_id: "t-1".to_string()
        }]
    );
    assert_eq!((state(&a), state(&b)), before);
}

/// A status the two sides disagree on with no claim to settle it (`Pending`
/// against `Blocked`) is not decided by the reconciler either.
#[test]
fn an_unclaimed_status_disagreement_is_left_alone() {
    let (mut a, mut b) = replicas(&["t-1"]);
    b.update_status("t-1", TaskStatus::Blocked);
    let before = (state(&a), state(&b));

    let report = reconcile(&mut a, &mut b, &Table::default());

    assert_eq!(
        report.unresolved,
        vec![Unresolved::RecordsDiffer {
            task_id: "t-1".to_string()
        }]
    );
    assert_eq!((state(&a), state(&b)), before);
}

/// Settling twice changes nothing the second time, and the report is the same
/// whichever order the conflicts were created in.
#[test]
fn reconciling_is_idempotent_and_deterministic() {
    let build = || {
        let (mut a, mut b) = replicas(&["t-1", "t-2", "t-3"]);
        claim(&mut a, 1);
        claim(&mut b, 2);
        claim(&mut b, 2);
        b.enqueue(task("only-b"));
        (a, b)
    };
    let arbiter = Table::with(&[("t-1", ClaimVerdict::HeldBy(agent_id(1)))]);

    let (mut a, mut b) = build();
    let first = reconcile(&mut a, &mut b, &arbiter);
    assert!(first.converged());
    let settled = (state(&a), state(&b));
    let second = reconcile(&mut a, &mut b, &arbiter);
    assert_eq!(second, Default::default(), "nothing is left to settle");
    assert_eq!((state(&a), state(&b)), settled);

    let (mut a2, mut b2) = build();
    assert_eq!(reconcile(&mut a2, &mut b2, &arbiter), first);

    // Every task is held by at most one agent, and it is the same on both sides.
    let holders: BTreeSet<_> = a.claims().map(|(id, h)| (id.to_string(), *h)).collect();
    assert_eq!(
        holders,
        BTreeSet::from([
            ("t-1".to_string(), agent_id(1)),
            ("t-2".to_string(), agent_id(2)),
        ])
    );
}
