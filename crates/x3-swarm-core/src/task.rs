use serde::{Deserialize, Serialize};
use crate::{AgentKind, AgentPermissionTier, policy::ApprovalRequirement};

/// Core task structure for swarm agents.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentTask {
    pub id: String,
    pub title: String,
    pub feature: String,
    pub agent: AgentKind,
    pub permission_tier: AgentPermissionTier,
    pub allowed_paths: Vec<String>,
    pub forbidden_paths: Vec<String>,
    pub required_commands: Vec<String>,
    pub approval_required: ApprovalRequirement,
    pub status: TaskStatus,
    pub risk: String,
}

impl AgentTask {
    pub fn new(id: String, title: String, feature: String, agent: AgentKind) -> Self {
        Self {
            id,
            title,
            feature,
            agent,
            permission_tier: AgentPermissionTier::ReadOnly,
            allowed_paths: vec![],
            forbidden_paths: vec![],
            required_commands: vec![],
            approval_required: ApprovalRequirement::None,
            status: TaskStatus::Pending,
            risk: "low".to_string(),
        }
    }
}

/// Result from agent task execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentResult {
    pub task_id: String,
    pub status: TaskStatus,
    pub files_changed: Vec<String>,
    pub commands_run: Vec<String>,
    pub summary: String,
    pub blockers: Vec<String>,
}

/// Task execution status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Passed,
    Failed,
    Blocked,
    NeedsApproval,
}

impl Default for TaskStatus {
    fn default() -> Self {
        TaskStatus::Pending
    }
}
