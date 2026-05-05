use serde::Serialize;
use crate::memory::AgentMemory;
use crate::scoreboard::SwarmScoreboard;
use crate::scheduler::SwarmScheduler;

/// Report types supported by the swarm.
#[derive(Debug, Clone, Serialize)]
pub enum ReportType {
    Health,
    Tasks,
    Memory,
}

/// Swarm readiness report generator.
#[derive(Serialize)]
pub struct SwarmReport {
    pub health_status: String,
    pub task_count: usize,
    pub success_rate: f64,
    pub top_findings: Vec<String>,
}

impl SwarmReport {
    pub fn generate(scoreboard: &SwarmScoreboard, memory: &AgentMemory, scheduler: &SwarmScheduler) -> Self {
        Self {
            health_status: "GUARDED_TESTNET".to_string(),
            task_count: scheduler.count_tasks(),
            success_rate: scoreboard.success_rate(),
            top_findings: memory.entries().iter().map(|e| e.finding.clone()).collect(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn to_markdown(&self) -> String {
        format!("# X3 Swarm Report\n\nSuccess Rate: {:.2}%\nTasks: {}", self.success_rate * 100.0, self.task_count)
    }
}
