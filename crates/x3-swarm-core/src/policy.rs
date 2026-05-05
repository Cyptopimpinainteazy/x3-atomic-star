use serde::{Deserialize, Serialize};

/// Approval levels required for task execution or file changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalRequirement {
    None,
    HumanReview,
    SecurityReview,
    GovernanceReview,
    Blocked,
}

impl ApprovalRequirement {
    /// Check if approval is satisfied (stub for now).
    pub fn is_satisfied(&self) -> bool {
        matches!(self, ApprovalRequirement::None)
    }
}

/// Agent policy structure for swarm control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPolicy {
    pub kind: crate::agent::AgentKind,
    pub permission_tier: crate::agent::AgentPermissionTier,
    pub auto_edit_allowed: Vec<String>,
    pub approval_required: Vec<String>,
    pub forbidden_paths: Vec<String>,
}

impl AgentPolicy {
    pub fn allows_path(&self, path: &str) -> bool {
        self.auto_edit_allowed.iter().any(|prefix| path.starts_with(prefix))
    }
}

pub fn default_agent_policies() -> Vec<AgentPolicy> {
    vec![
        AgentPolicy {
            kind: crate::agent::AgentKind::RepoScanner,
            permission_tier: crate::agent::AgentPermissionTier::ReadOnly,
            auto_edit_allowed: vec![],
            approval_required: vec![],
            forbidden_paths: vec![".env".into(), "private_keys".into(), "validator_keys".into()],
        },
        AgentPolicy {
            kind: crate::agent::AgentKind::TestBuilder,
            permission_tier: crate::agent::AgentPermissionTier::DocsTestsReports,
            auto_edit_allowed: vec!["tests/".into(), "docs/".into(), "reports/".into()],
            approval_required: vec!["runtime/".into(), "pallets/".into()],
            forbidden_paths: vec![".env".into(), "private_keys".into()],
        },
    ]
}
