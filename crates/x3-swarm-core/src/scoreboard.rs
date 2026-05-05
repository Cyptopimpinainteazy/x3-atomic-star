use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmScoreboard {
    pub agent_scores: HashMap<String, f64>,
    pub task_success_rate: f64,
    pub total_tasks_completed: u64,
    pub total_tasks_failed: u64,
}

impl Default for SwarmScoreboard {
    fn default() -> Self {
        Self {
            agent_scores: HashMap::new(),
            task_success_rate: 0.0,
            total_tasks_completed: 0,
            total_tasks_failed: 0,
        }
    }
}

impl SwarmScoreboard {
    pub fn record_result(&mut self, agent: &str, success: bool) {
        let score = self.agent_scores.entry(agent.to_string()).or_insert(0.0);
        if success {
            *score += 1.0;
            self.total_tasks_completed += 1;
        } else {
            *score -= 1.0;
            self.total_tasks_failed += 1;
        }
        self.task_success_rate = if self.total_tasks_completed + self.total_tasks_failed == 0 {
            0.0
        } else {
            self.total_tasks_completed as f64 / (self.total_tasks_completed + self.total_tasks_failed) as f64
        };
    }

    pub fn success_rate(&self) -> f64 {
        self.task_success_rate
    }
}
