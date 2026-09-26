use crate::authority::{DispatchRefusal, SwarmAuthority};
use crate::genesis::{AgentId, BlockHeight};
use crate::{AgentKind, AgentTask, TaskStatus};
use std::collections::{HashMap, VecDeque};

/// Swarm task scheduler.
#[derive(Debug, Default)]
pub struct SwarmScheduler {
    tasks: HashMap<String, AgentTask>,
    task_order: VecDeque<String>,
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
        self.tasks.insert(task.id.clone(), task)
    }

    /// The pending task for `agent`, if any. Private on purpose: taking work off
    /// the queue without asking the authority is the hole this module had.
    fn pending_task(&self, agent: AgentKind) -> Option<&AgentTask> {
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

    /// Take the next `Pending` task for `agent`, or refuse.
    ///
    /// This is the only public route to a task. It asks
    /// [`SwarmAuthority::may_dispatch`] *before* looking at the queue, so an
    /// agent that has been quarantined, suspended, killed, terminated or expired
    /// is refused and leaves the queue untouched — the task stays `Pending` for
    /// an agent that is allowed to take it, rather than being consumed by one
    /// that is not.
    pub fn next_task_for(
        &self,
        agent: AgentKind,
        agent_id: &AgentId,
        authority: &SwarmAuthority,
        now: BlockHeight,
    ) -> Result<&AgentTask, DispatchRefusal> {
        authority.may_dispatch(agent_id, now)?;
        self.pending_task(agent).ok_or(DispatchRefusal::NoTask)
    }

    pub fn update_status(&mut self, task_id: &str, status: TaskStatus) -> bool {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.status = status;
            return true;
        }
        false
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
            .next_task_for(class, &id(1), &authority, 10)
            .expect("a clean, registered, unexpired agent is allowed to work");
        assert_eq!(taken.id, "t-1");
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
            .next_task_for(class.clone(), &id(2), &authority, 10)
            .expect_err("a quarantined agent must not be handed work");
        assert!(
            matches!(refusal, DispatchRefusal::Halted { .. }),
            "expected Halted, got {refusal:?}"
        );

        // The task was not consumed: an allowed agent still gets it.
        let allowed = authority_with_agent(id(3), class.clone());
        assert_eq!(
            scheduler
                .next_task_for(class, &id(3), &allowed, 10)
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
            scheduler.next_task_for(class, &id(4), &authority, 10),
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
            .next_task_for(class, &id(5), &authority, 10)
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
            scheduler.next_task_for(class, &id(9), &authority, 10),
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
            scheduler.next_task_for(class.clone(), &id(6), &authority, 100),
            Err(DispatchRefusal::Expired { .. })
        ));
        // One block before expiry it still works.
        assert_eq!(
            scheduler
                .next_task_for(class, &id(6), &authority, 99)
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
                .next_task_for(class, &id(7), &authority, 10)
                .unwrap()
                .id,
            "t-7"
        );
    }

    #[test]
    fn a_cleared_agent_with_no_pending_task_is_told_so() {
        let class = AgentKind::RepoScanner;
        let authority = authority_with_agent(id(8), class.clone());
        let scheduler = SwarmScheduler::new();

        assert_eq!(
            scheduler.next_task_for(class, &id(8), &authority, 10),
            Err(DispatchRefusal::NoTask)
        );
    }
}
