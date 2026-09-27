use crate::authority::{DispatchRefusal, SwarmAuthority};
use crate::genesis::{AgentId, BlockHeight};
use crate::{AgentKind, AgentTask, TaskStatus};
use std::collections::{HashMap, VecDeque};

/// Swarm task scheduler.
#[derive(Clone, Debug, Default)]
pub struct SwarmScheduler {
    tasks: HashMap<String, AgentTask>,
    task_order: VecDeque<String>,
    /// Who took each task, by task id. Filled by
    /// [`SwarmScheduler::claim_next_for`] and cleared when the task goes back to
    /// `Pending`. A second replica of this scheduler cannot see these stamps,
    /// which is exactly why the chain-side claim — not this map — is the
    /// arbiter; the map is what lets an operator *see* the conflict.
    claims: HashMap<String, AgentId>,
    /// Reserved for future active-agent accounting.
    _active_agents: Vec<AgentKind>,
}

impl SwarmScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(&mut self, task: AgentTask) -> Option<AgentTask> {
        if !self.tasks.contains_key(&task.id) {
            self.task_order.push_back(task.id.clone());
        }
        // Re-queuing work drops the previous holder's stamp: after a rejection
        // puts a task back to `Pending`, `claim_of` must not still name the agent
        // that lost it.
        if task.status == TaskStatus::Pending {
            self.claims.remove(&task.id);
        }
        self.tasks.insert(task.id.clone(), task)
    }

    /// The first `Pending` task for `agent`, dispatchable or not.
    fn first_pending(&self, agent: AgentKind) -> Option<&AgentTask> {
        for task_id in &self.task_order {
            let Some(task) = self.tasks.get(task_id) else {
                continue;
            };
            if task.agent == agent && task.status == TaskStatus::Pending {
                return Some(task);
            }
        }
        None
    }

    /// The first `Pending` task for `agent` that may be worked **now**: it also
    /// has to be past its approval gate.
    ///
    /// `ApprovalRequirement::is_satisfied_legacy` is true only for
    /// `ApprovalRequirement::None`, so work that needs a human, security or
    /// governance review (or is `Blocked` outright) stays in the queue instead of
    /// being handed to an agent that would be acting without authorisation.
    fn next_dispatchable(&self, agent: AgentKind) -> Option<&AgentTask> {
        for task_id in &self.task_order {
            let Some(task) = self.tasks.get(task_id) else {
                continue;
            };
            if task.agent == agent
                && task.status == TaskStatus::Pending
                && task.approval_required.is_satisfied_legacy()
            {
                return Some(task);
            }
        }
        None
    }

    /// Take the next dispatchable task for `agent` **and mark it claimed**, or
    /// refuse.
    ///
    /// This is the only route that hands work out, and taking it is one
    /// operation: the task is `Running` and stamped with `agent_id` before this
    /// returns, so two callers cannot both be told they hold the same task. It
    /// asks
    /// [`SwarmAuthority::may_dispatch`] *before* looking at the queue, so an
    /// agent that has been quarantined, suspended, killed, terminated or expired
    /// is refused and leaves the queue untouched — the task stays `Pending` for
    /// an agent that is allowed to take it, rather than being consumed by one
    /// that is not.
    ///
    /// Returns an owned clone: the caller holds its own copy of the claim, and
    /// keeping it cannot block the next claim.
    pub fn claim_next_for(
        &mut self,
        agent: AgentKind,
        agent_id: &AgentId,
        authority: &SwarmAuthority,
        now: BlockHeight,
    ) -> Result<AgentTask, DispatchRefusal> {
        authority.may_dispatch(agent_id, now)?;
        let Some(task_id) = self
            .next_dispatchable(agent.clone())
            .map(|task| task.id.clone())
        else {
            // "Nothing to do" and "your work is waiting on an approval" are
            // different answers, and an agent that cannot tell them apart will
            // keep asking for work that is deliberately held back.
            return Err(match self.first_pending(agent) {
                Some(task) => DispatchRefusal::AwaitingApproval {
                    agent_id: *agent_id,
                    task_id: task.id.clone(),
                    requirement: task.approval_required.clone(),
                },
                None => DispatchRefusal::NoTask,
            });
        };
        let task = self
            .tasks
            .get_mut(&task_id)
            .expect("next_dispatchable returned an id it just found");
        task.status = TaskStatus::Running;
        let claimed = task.clone();
        self.claims.insert(task_id, *agent_id);
        Ok(claimed)
    }

    /// Who holds this task, if anyone.
    pub fn claim_of(&self, task_id: &str) -> Option<&AgentId> {
        self.claims.get(task_id)
    }

    /// Every live claim, in enqueue order so two calls agree.
    ///
    /// This is what a reconciler reads: two replicas that were partitioned can
    /// disagree about a task id, and the disagreement is only visible if the
    /// claims are readable. Settling it is the chain's job — see
    /// `pallet-northern-swarm`'s `claim_task`, which refuses a second executor.
    pub fn claims(&self) -> impl Iterator<Item = (&str, &AgentId)> {
        self.task_order.iter().filter_map(move |task_id| {
            self.claims
                .get(task_id)
                .map(|agent_id| (task_id.as_str(), agent_id))
        })
    }

    /// Put one task back in the queue and drop its holder.
    ///
    /// Returns whether there was a claim to release.
    pub fn release_claim(&mut self, task_id: &str) -> bool {
        if self.claims.remove(task_id).is_none() {
            return false;
        }
        if let Some(task) = self.tasks.get_mut(task_id) {
            if task.status == TaskStatus::Running {
                task.status = TaskStatus::Pending;
            }
        }
        true
    }

    /// Put back every task held by `agent_id`, returning how many were released.
    ///
    /// A claim outlives the agent unless something releases it: without this, work
    /// taken by an agent that is then killed or suspended stays `Running` with a
    /// stamp from an agent that can never finish it, and no other agent of its
    /// class can take it. The kill path owns the authority's view; this is the
    /// scheduler's half of that transition.
    pub fn release_claims_of(&mut self, agent_id: &AgentId) -> usize {
        let held: Vec<String> = self
            .claims()
            .filter(|(_, holder)| *holder == agent_id)
            .map(|(task_id, _)| task_id.to_string())
            .collect();
        for task_id in &held {
            self.release_claim(task_id);
        }
        held.len()
    }

    pub fn update_status(&mut self, task_id: &str, status: TaskStatus) -> bool {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.status = status;
            if status == TaskStatus::Pending {
                self.claims.remove(task_id);
            }
            return true;
        }
        false
    }

    /// The task with this id, if the scheduler is holding one.
    pub fn get(&self, task_id: &str) -> Option<&AgentTask> {
        self.tasks.get(task_id)
    }

    /// Every task the scheduler holds, in the order it was first enqueued.
    ///
    /// The order is the queue's own `task_order`, not the hash map's iteration
    /// order, so two calls over an unchanged scheduler produce the same
    /// sequence. Re-enqueuing an id updates in place and does not move it.
    pub fn tasks(&self) -> impl Iterator<Item = &AgentTask> {
        self.task_order
            .iter()
            .filter_map(move |task_id| self.tasks.get(task_id))
    }

    pub fn count_tasks(&self) -> usize {
        self.tasks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authority::{DispatchRefusal, SwarmAuthority};
    use crate::genesis::GenesisRecord;
    use crate::misconduct::ViolationClass;
    use crate::{AgentPermissionTier, ApprovalRequirement, RiskLevel};

    fn id(b: u8) -> AgentId {
        [b; 32]
    }

    /// A registered, unviolating agent of the class the task is for.
    fn authority_with_agent(agent_id: AgentId, class: AgentKind) -> SwarmAuthority {
        let mut authority = SwarmAuthority::new();
        let record = GenesisRecord::new(
            agent_id,
            [0u8; 32],
            "scheduler test agent",
            class,
            AgentPermissionTier::DocsTestsReports,
            vec![],
            1,
        );
        authority.genesis_mut().create(record).unwrap();
        authority
    }

    fn task(id: &str, class: AgentKind) -> AgentTask {
        AgentTask {
            id: id.to_string(),
            title: "a task".to_string(),
            feature: "scheduler".to_string(),
            agent: class,
            permission_tier: AgentPermissionTier::DocsTestsReports,
            allowed_paths: vec![],
            forbidden_paths: vec![],
            required_commands: vec![],
            approval_required: ApprovalRequirement::None,
            status: TaskStatus::Pending,
            risk: RiskLevel::Low,
        }
    }

    #[test]
    fn a_clean_agent_is_handed_its_task() {
        let class = AgentKind::TestBuilder;
        let authority = authority_with_agent(id(1), class.clone());
        let mut scheduler = SwarmScheduler::new();
        scheduler.enqueue(task("t-1", class.clone()));

        let taken = scheduler
            .claim_next_for(class, &id(1), &authority, 10)
            .expect("a clean, registered, unexpired agent is allowed to work");
        assert_eq!(taken.id, "t-1");
        // The handout is the claim: the task is out of the queue and stamped.
        assert_eq!(taken.status, TaskStatus::Running);
        assert_eq!(scheduler.get("t-1").unwrap().status, TaskStatus::Running);
        assert_eq!(scheduler.claim_of("t-1"), Some(&id(1)));
    }

    /// The regression this module was missing: dispatching used to be a peek, so
    /// two calls both returned the same `Pending` task. Measured before the fix:
    /// `next_task_for` twice returned `t-1` twice.
    #[test]
    fn a_claimed_task_is_never_handed_out_twice() {
        let class = AgentKind::TestBuilder;
        let authority = authority_with_agent(id(1), class.clone());
        let mut scheduler = SwarmScheduler::new();
        scheduler.enqueue(task("t-1", class.clone()));

        let first = scheduler
            .claim_next_for(class.clone(), &id(1), &authority, 10)
            .expect("the first claim must succeed");
        let second = scheduler.claim_next_for(class.clone(), &id(1), &authority, 10);
        assert_eq!(
            (first.id.as_str(), second),
            ("t-1", Err(DispatchRefusal::NoTask)),
            "a claimed task must not be handed to a second caller"
        );

        // With one more task queued, the next claim gets *that* one.
        scheduler.enqueue(task("t-2", class.clone()));
        assert_eq!(
            scheduler
                .claim_next_for(class, &id(1), &authority, 10)
                .unwrap()
                .id,
            "t-2"
        );
    }

    /// A task that carries an approval requirement is not dispatchable until the
    /// approval is satisfied; the refusal says which task and which requirement,
    /// so "no work" and "work held back" stay distinguishable.
    #[test]
    fn work_waiting_on_an_approval_is_not_dispatched() {
        let class = AgentKind::Integrator;
        let authority = authority_with_agent(id(7), class.clone());
        let mut scheduler = SwarmScheduler::new();
        let mut gated = task("t-gated", class.clone());
        gated.approval_required = ApprovalRequirement::HumanReview;
        scheduler.enqueue(gated);

        let refusal = scheduler
            .claim_next_for(class.clone(), &id(7), &authority, 10)
            .expect_err("a task awaiting human review must not be handed out");
        match refusal {
            DispatchRefusal::AwaitingApproval {
                task_id,
                requirement,
                ..
            } => {
                assert_eq!(task_id, "t-gated");
                assert_eq!(requirement, ApprovalRequirement::HumanReview);
            }
            other => panic!("expected AwaitingApproval, got {other:?}"),
        }
        // Held back means untouched: still pending, still unclaimed.
        assert_eq!(
            scheduler.get("t-gated").unwrap().status,
            TaskStatus::Pending
        );
        assert_eq!(scheduler.claim_of("t-gated"), None);
    }

    /// Re-queuing a task (the reject path) hands it back out, and the stamp names
    /// the new holder rather than the agent that lost it.
    #[test]
    fn a_requeued_task_can_be_claimed_again_and_the_stamp_moves() {
        let class = AgentKind::TestBuilder;
        let first_agent = authority_with_agent(id(1), class.clone());
        let second_agent = authority_with_agent(id(8), class.clone());
        let mut scheduler = SwarmScheduler::new();
        scheduler.enqueue(task("t-6", class.clone()));

        assert_eq!(
            scheduler
                .claim_next_for(class.clone(), &id(1), &first_agent, 10)
                .unwrap()
                .id,
            "t-6"
        );
        assert_eq!(scheduler.claim_of("t-6"), Some(&id(1)));

        assert!(scheduler.update_status("t-6", TaskStatus::Pending));
        assert_eq!(
            scheduler.claim_of("t-6"),
            None,
            "returning a task to the queue drops the previous holder's stamp"
        );
        assert_eq!(
            scheduler
                .claim_next_for(class, &id(8), &second_agent, 10)
                .unwrap()
                .id,
            "t-6"
        );
        assert_eq!(scheduler.claim_of("t-6"), Some(&id(8)));
        assert_eq!(
            scheduler.claims().collect::<Vec<_>>(),
            vec![("t-6", &id(8))],
            "claim stamps are readable in enqueue order"
        );
    }

    /// A claim must not outlive the agent that holds it: work taken by an agent
    /// that is then killed has to become available again, or the queue starves.
    #[test]
    fn releasing_a_dead_holders_claim_puts_the_work_back() {
        let class = AgentKind::TestBuilder;
        let doomed = authority_with_agent(id(1), class.clone());
        let survivor = authority_with_agent(id(2), class.clone());
        let mut scheduler = SwarmScheduler::new();
        scheduler.enqueue(task("t-8", class.clone()));
        scheduler.enqueue(task("t-9", class.clone()));

        scheduler
            .claim_next_for(class.clone(), &id(1), &doomed, 10)
            .unwrap();
        assert_eq!(
            scheduler
                .claim_next_for(class.clone(), &id(1), &doomed, 10)
                .unwrap()
                .id,
            "t-9"
        );

        // The operator kills that agent; the scheduler's half of the transition.
        assert_eq!(scheduler.release_claims_of(&id(1)), 2);
        assert_eq!(scheduler.claim_of("t-8"), None);
        assert_eq!(scheduler.get("t-8").unwrap().status, TaskStatus::Pending);
        assert_eq!(scheduler.get("t-9").unwrap().status, TaskStatus::Pending);
        assert_eq!(
            scheduler.release_claims_of(&id(1)),
            0,
            "releasing twice is a no-op"
        );

        // Another agent of the class can now take the work.
        assert_eq!(
            scheduler
                .claim_next_for(class.clone(), &id(2), &survivor, 10)
                .unwrap()
                .id,
            "t-8"
        );
        assert!(
            scheduler.release_claim("t-8"),
            "a single task can be released"
        );
        assert!(!scheduler.release_claim("t-8"), "and only once");
        assert_eq!(scheduler.claim_of("t-8"), None);
    }

    #[test]
    fn a_quarantined_agent_is_refused_and_the_task_stays_pending() {
        let class = AgentKind::TestBuilder;
        let mut authority = authority_with_agent(id(2), class.clone());
        // One D-class violation is Quarantine on this ladder.
        authority
            .enforce_violation(id(2), ViolationClass::D, "safety breach", 5)
            .unwrap();

        let mut scheduler = SwarmScheduler::new();
        scheduler.enqueue(task("t-2", class.clone()));

        let refusal = scheduler
            .claim_next_for(class.clone(), &id(2), &authority, 10)
            .expect_err("a quarantined agent must not be handed work");
        assert!(
            matches!(refusal, DispatchRefusal::Halted { .. }),
            "expected Halted, got {refusal:?}"
        );

        // The task was not consumed: an allowed agent still gets it.
        let allowed = authority_with_agent(id(3), class.clone());
        assert_eq!(
            scheduler
                .claim_next_for(class, &id(3), &allowed, 10)
                .unwrap()
                .id,
            "t-2"
        );
    }

    #[test]
    fn a_suspended_agent_is_refused() {
        let class = AgentKind::Fixer;
        let mut authority = authority_with_agent(id(4), class.clone());
        for block in 0..2u64 {
            authority
                .enforce_violation(id(4), ViolationClass::D, "critical breach", block + 1)
                .unwrap();
        }
        assert_eq!(
            authority.misconduct().current_sanction(&id(4)),
            crate::misconduct::Sanction::Suspension
        );

        let mut scheduler = SwarmScheduler::new();
        scheduler.enqueue(task("t-3", class.clone()));
        assert!(matches!(
            scheduler.claim_next_for(class, &id(4), &authority, 10),
            Err(DispatchRefusal::Halted { .. })
        ));
    }

    #[test]
    fn a_killed_agent_is_refused_even_though_a_task_waits() {
        let class = AgentKind::Integrator;
        let mut authority = authority_with_agent(id(5), class.clone());
        for block in 0..3u64 {
            // The third D-class violation kills the agent and terminates genesis.
            let _ = authority.enforce_violation(id(5), ViolationClass::D, "breach", block + 1);
        }
        assert!(authority.genesis().get(&id(5)).unwrap().terminated);

        let mut scheduler = SwarmScheduler::new();
        scheduler.enqueue(task("t-4", class.clone()));
        let refusal = scheduler
            .claim_next_for(class, &id(5), &authority, 10)
            .expect_err("a killed agent must not be handed work");
        // Whichever of the two checks fires first, it refuses: the record is
        // terminated *and* the ladder is halted at Kill.
        assert!(
            matches!(refusal, DispatchRefusal::Terminated(_)),
            "expected Terminated, got {refusal:?}"
        );
    }

    #[test]
    fn an_unregistered_agent_is_refused() {
        let class = AgentKind::Auditor;
        let authority = SwarmAuthority::new();
        let mut scheduler = SwarmScheduler::new();
        scheduler.enqueue(task("t-5", class.clone()));

        assert_eq!(
            scheduler.claim_next_for(class, &id(9), &authority, 10),
            Err(DispatchRefusal::UnknownAgent(id(9)))
        );
    }

    #[test]
    fn an_expired_agent_is_refused() {
        let class = AgentKind::Benchmark;
        let mut authority = authority_with_agent(id(6), class.clone());
        authority
            .genesis_mut()
            .set_expiry(&id(6), Some(100))
            .unwrap();

        let mut scheduler = SwarmScheduler::new();
        scheduler.enqueue(task("t-6", class.clone()));

        assert!(matches!(
            scheduler.claim_next_for(class.clone(), &id(6), &authority, 100),
            Err(DispatchRefusal::Expired { .. })
        ));
        // One block before expiry it still works.
        assert_eq!(
            scheduler
                .claim_next_for(class, &id(6), &authority, 99)
                .unwrap()
                .id,
            "t-6"
        );
    }

    #[test]
    fn a_recorded_but_not_halting_sanction_still_works() {
        let class = AgentKind::ReadinessReporter;
        let mut authority = authority_with_agent(id(7), class.clone());
        // One A-class violation is a Warning: recorded, not halting.
        authority
            .enforce_violation(id(7), ViolationClass::A, "rate limit", 1)
            .unwrap();
        assert_eq!(
            authority.misconduct().current_sanction(&id(7)),
            crate::misconduct::Sanction::Warning
        );

        let mut scheduler = SwarmScheduler::new();
        scheduler.enqueue(task("t-7", class.clone()));
        assert_eq!(
            scheduler
                .claim_next_for(class, &id(7), &authority, 10)
                .unwrap()
                .id,
            "t-7"
        );
    }

    #[test]
    fn a_cleared_agent_with_no_pending_task_is_told_so() {
        let class = AgentKind::RepoScanner;
        let authority = authority_with_agent(id(8), class.clone());
        let mut scheduler = SwarmScheduler::new();

        assert_eq!(
            scheduler.claim_next_for(class, &id(8), &authority, 10),
            Err(DispatchRefusal::NoTask)
        );
    }

    #[test]
    fn a_task_reads_back_and_lists_in_enqueue_order() {
        let class = AgentKind::RepoScanner;
        let mut scheduler = SwarmScheduler::new();
        scheduler.enqueue(task("t-b", class.clone()));
        scheduler.enqueue(task("t-a", class.clone()));

        // `get` is by id, not position.
        assert_eq!(scheduler.get("t-a").map(|t| t.id.as_str()), Some("t-a"));
        assert!(scheduler.get("t-missing").is_none());

        // Listing is the enqueue order, so it does not depend on how the
        // scheduler's map happened to hash the ids.
        let listed: Vec<&str> = scheduler.tasks().map(|t| t.id.as_str()).collect();
        assert_eq!(listed, vec!["t-b", "t-a"]);
        assert_eq!(scheduler.count_tasks(), 2);
    }

    #[test]
    fn re_enqueuing_an_id_updates_in_place_and_keeps_its_position() {
        let class = AgentKind::RepoScanner;
        let mut scheduler = SwarmScheduler::new();
        scheduler.enqueue(task("t-1", class.clone()));
        scheduler.enqueue(task("t-2", class.clone()));
        scheduler.enqueue(task("t-1", class.clone()));

        let listed: Vec<&str> = scheduler.tasks().map(|t| t.id.as_str()).collect();
        assert_eq!(
            listed,
            vec!["t-1", "t-2"],
            "a duplicate id must not appear twice or reorder the queue"
        );
    }
}
