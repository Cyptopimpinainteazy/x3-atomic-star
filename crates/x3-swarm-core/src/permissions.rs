use crate::{AgentKind, AgentPermissionTier};

/// Permissions management for swarm agents.
pub struct Permissions {
    _agent: AgentKind,
    tier: AgentPermissionTier,
}

impl Permissions {
    pub fn new(agent: AgentKind, tier: AgentPermissionTier) -> Self {
        Self { _agent: agent, tier }
    }

    /// Check if agent can edit a path.
    pub fn can_edit_path(&self, path: &str) -> bool {
        self.tier.allows_path(path)
    }

    /// Get required approval for operation.
    pub fn required_approval(&self, _operation: &str) -> crate::policy::ApprovalRequirement {
        match self.tier {
            AgentPermissionTier::MainnetBlocked => crate::policy::ApprovalRequirement::Blocked,
            _ => crate::policy::ApprovalRequirement::HumanReview,
        }
    }
}
