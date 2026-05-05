use crate::{AgentTask, AgentKind, TaskStatus};
use std::collections::HashMap;

/// Swarm task scheduler.
pub struct SwarmScheduler {
    tasks: HashMap<String, AgentTask>,
    _active_agents: Vec<AgentKind>,
}

impl SwarmScheduler {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            _active_agents: vec![],
        }
    }

    pub fn enqueue(&mut self, task: AgentTask) {
        self.tasks.insert(task.id.clone(), task);
    }

    pub fn next_task(&self, agent: AgentKind) -> Option<&AgentTask> {
        for task in self.tasks.values() {
            if task.agent == agent && task.status == TaskStatus::Pending {
                return Some(task);
            }
        }
        None
    }

    pub fn update_status(&mut self, task_id: &str, status: TaskStatus) {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.status = status;
        }
    }

    pub fn count_tasks(&self) -> usize {
        self.tasks.len()
    }
}
