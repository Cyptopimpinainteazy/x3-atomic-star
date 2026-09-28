//! The two behaviours `FEATURE_REGISTRY.toml` lists as missing for this crate:
//! multi-agent race conditions and partition tolerance.
//!
//! The scheduler is a control plane, not a consensus system, so the honest
//! questions are narrow and answerable:
//!
//! 1. **Race** — many agents asking for work at once must not be handed the same
//!    task twice, and the set of claims has to match a sequential reference run
//!    over the same queue. `claim_next_for` is one `&mut self` operation that
//!    marks the task and stamps the holder, so the answer is checkable rather
//!    than assumed.
//! 2. **Partition** — two replicas of the same queue *can* claim the same task;
//!    no single-process data structure can prevent that. What the crate owes an
//!    operator is that the conflict is **visible** (`claims()`) and that the
//!    loser can be put back to work (`release_claim`). Settling which replica
//!    wins is `reconcile`'s job, against the chain's record of the task's first
//!    claimer (`pallet-northern-swarm`'s `claimed_by`); see `replica_reconcile.rs`.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use x3_swarm_core::{
    AgentKind, AgentPermissionTier, AgentTask, DispatchRefusal, GenesisRecord, SwarmAuthority,
    SwarmScheduler, TaskStatus,
};

const TASKS: usize = 32;
const WORKERS: usize = 8;

fn agent_id(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn authority_with(ids: impl IntoIterator<Item = u8>, class: AgentKind) -> SwarmAuthority {
    let mut authority = SwarmAuthority::new();
    for byte in ids {
        let record = GenesisRecord::new(
            agent_id(byte),
            agent_id(0),
            "race test agent",
            class.clone(),
            AgentPermissionTier::DocsTestsReports,
            vec![],
            1,
        );
        authority.genesis_mut().create(record).unwrap();
    }
    authority
}

fn task(index: usize, class: AgentKind) -> AgentTask {
    AgentTask::new(
        format!("t-{index:03}"),
        format!("race task {index}"),
        "x3_swarm_core".to_string(),
        class,
    )
}

fn queue(class: AgentKind) -> Vec<AgentTask> {
    (0..TASKS).map(|i| task(i, class.clone())).collect()
}

/// The sequential reference: one agent, one at a time, in enqueue order.
fn sequential_claims(tasks: &[AgentTask], class: AgentKind) -> Vec<String> {
    let authority = authority_with([1u8], class.clone());
    let mut scheduler = SwarmScheduler::new();
    for t in tasks {
        scheduler.enqueue(t.clone());
    }
    let mut claimed = Vec::new();
    loop {
        match scheduler.claim_next_for(class.clone(), &agent_id(1), &authority, 10) {
            Ok(t) => claimed.push(t.id),
            Err(DispatchRefusal::NoTask) => break,
            Err(other) => panic!("reference run was refused unexpectedly: {other:?}"),
        }
    }
    claimed
}

#[test]
fn parallel_claims_match_the_sequential_reference() {
    let class = AgentKind::TestBuilder;
    let tasks = queue(class.clone());
    let reference = sequential_claims(&tasks, class.clone());
    assert_eq!(reference.len(), TASKS, "the reference run takes every task");
    assert_eq!(
        reference.iter().collect::<BTreeSet<_>>().len(),
        TASKS,
        "the reference hands out each task exactly once"
    );

    // The same queue, claimed by eight workers through one lock. This is the
    // shape the service uses (the scheduler lives behind a mutex), and the shape
    // a single-process swarm actually runs.
    let authority = authority_with(1..=(WORKERS as u8), class.clone());
    let mut scheduler = SwarmScheduler::new();
    for t in &tasks {
        scheduler.enqueue(t.clone());
    }
    let state = Arc::new(Mutex::new((scheduler, authority)));

    let mut handles = Vec::new();
    for worker in 1..=(WORKERS as u8) {
        let state = Arc::clone(&state);
        let class = class.clone();
        handles.push(std::thread::spawn(move || {
            let mut mine = Vec::new();
            loop {
                let mut guard = state.lock().unwrap();
                let (scheduler, authority) = &mut *guard;
                match scheduler.claim_next_for(class.clone(), &agent_id(worker), authority, 10) {
                    Ok(task) => {
                        assert_eq!(
                            task.status,
                            TaskStatus::Running,
                            "a handed-out task is claimed, not left pending"
                        );
                        assert_eq!(
                            scheduler.claim_of(&task.id),
                            Some(&agent_id(worker)),
                            "the stamp must name the agent that just took it"
                        );
                        mine.push(task.id);
                    }
                    Err(DispatchRefusal::NoTask) => break,
                    Err(other) => panic!("worker {worker} was refused unexpectedly: {other:?}"),
                }
            }
            mine
        }));
    }

    let mut claimed: Vec<String> = Vec::new();
    for handle in handles {
        claimed.extend(handle.join().expect("no worker panicked"));
    }

    assert_eq!(
        claimed.len(),
        TASKS,
        "eight racing workers must claim every task exactly once: {claimed:?}"
    );
    let claimed_set: BTreeSet<&String> = claimed.iter().collect();
    assert_eq!(
        claimed_set.len(),
        TASKS,
        "a task was claimed twice under concurrency: {claimed:?}"
    );
    assert_eq!(
        claimed_set,
        reference.iter().collect::<BTreeSet<_>>(),
        "the concurrent run must cover exactly the sequential run's tasks"
    );

    // And the stamps agree with what the workers saw.
    let guard = state.lock().unwrap();
    let stamped: Vec<&str> = guard.0.claims().map(|(id, _)| id).collect();
    assert_eq!(stamped.len(), TASKS);
    for id in &claimed {
        assert_eq!(guard.0.get(id).map(|t| t.status), Some(TaskStatus::Running));
    }
}

/// A split brain is possible in this crate and is not hidden: two replicas of one
/// queue each hand out the same task. What the crate provides is the evidence
/// (`claims()`) and the recovery (`release_claim`); the arbiter is the chain.
#[test]
fn a_partitioned_replica_pair_reports_its_conflict() {
    let class = AgentKind::TestBuilder;
    let tasks = queue(class.clone());

    let replica = |holder: u8| {
        let mut scheduler = SwarmScheduler::new();
        for t in &tasks {
            scheduler.enqueue(t.clone());
        }
        let authority = authority_with([holder], class.clone());
        let claimed = scheduler
            .claim_next_for(class.clone(), &agent_id(holder), &authority, 10)
            .expect("each replica can claim on its own");
        (scheduler, authority, claimed.id)
    };
    let (side_a, _authority_a, claimed_a) = replica(1);
    let (mut side_b, _authority_b, claimed_b) = replica(2);

    assert_eq!(
        claimed_a, claimed_b,
        "both sides of a partition are handed the same task"
    );
    let stamp_a = side_a.claim_of(&claimed_a).copied().unwrap();
    let stamp_b = side_b.claim_of(&claimed_b).copied().unwrap();
    assert_ne!(stamp_a, stamp_b, "the two replicas stamp different holders");

    // What a reconciler reads: the two claim maps, diffable per task id.
    let conflicts: Vec<&str> = side_a
        .claims()
        .filter(|(id, holder)| side_b.claim_of(id).is_some_and(|other| other != *holder))
        .map(|(id, _)| id)
        .collect();
    assert_eq!(
        conflicts,
        vec![claimed_a.as_str()],
        "the conflict is exactly the task both sides took, and it is visible"
    );

    // The crate's half of the recovery: the losing side's claim is released, and
    // the task is workable again. Deciding *which* side loses is `reconcile`'s
    // job, which asks the chain who claimed first (`replica_reconcile.rs`).
    assert!(side_b.release_claim(&claimed_b));
    assert_eq!(side_b.claim_of(&claimed_b), None);
    assert_eq!(
        side_b.get(&claimed_b).unwrap().status,
        TaskStatus::Pending,
        "a released claim puts the work back where it can be taken"
    );
}

/// Every state the scheduler can reach keeps the one invariant the race depends
/// on: a stamped task is a running task, and never more claims than tasks.
#[test]
fn claim_stamps_never_outnumber_the_queue() {
    let class = AgentKind::Auditor;
    let mut scheduler = SwarmScheduler::new();
    let authority = authority_with(1..=3, class.clone());

    // A fixed, reproducible interleaving: enqueue, claim, release in a cycle.
    let mut next = 0usize;
    for step in 0..64u8 {
        if step % 3 != 0 {
            scheduler.enqueue(task(next, class.clone()));
            next += 1;
        }
        let worker = (step % 3) + 1;
        let _ = scheduler.claim_next_for(class.clone(), &agent_id(worker), &authority, 10);
        if step % 5 == 0 {
            let held = scheduler.claims().next().map(|(id, _)| id.to_string());
            if let Some(id) = held {
                scheduler.release_claim(&id);
            }
        }
        let stamps = scheduler.claims().count();
        assert!(
            stamps <= scheduler.count_tasks(),
            "step {step}: {stamps} claims over {} tasks",
            scheduler.count_tasks()
        );
        for (id, _) in scheduler.claims() {
            assert_eq!(
                scheduler.get(id).map(|t| t.status),
                Some(TaskStatus::Running),
                "step {step}: a stamped task must be running"
            );
        }
    }
    assert!(next > 0, "the cycle enqueued work");
}
