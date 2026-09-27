use crate::{AgentKind, AgentPermissionTier};

/// Permissions management for swarm agents.
pub struct Permissions {
    /// Reserved for operation-specific permission rules and audit attribution.
    _agent: AgentKind,
    tier: AgentPermissionTier,
}

impl Permissions {
    pub fn new(agent: AgentKind, tier: AgentPermissionTier) -> Self {
        Self {
            _agent: agent,
            tier,
        }
    }

    /// Check if agent can edit a path.
    pub fn can_edit_path(&self, path: &str) -> bool {
        self.tier.allows_path(path)
    }

    /// The approval level the agent's tier alone implies.
    pub fn tier_requirement(&self) -> crate::policy::ApprovalRequirement {
        match self.tier {
            AgentPermissionTier::ReadOnly => crate::policy::ApprovalRequirement::HumanReview,
            AgentPermissionTier::DocsTestsReports => {
                crate::policy::ApprovalRequirement::HumanReview
            }
            AgentPermissionTier::TauriServiceWiring => {
                crate::policy::ApprovalRequirement::HumanReview
            }
            AgentPermissionTier::RuntimeProposalOnly => {
                crate::policy::ApprovalRequirement::SecurityReview
            }
            AgentPermissionTier::BridgeEconomicsProposalOnly => {
                crate::policy::ApprovalRequirement::SecurityReview
            }
            AgentPermissionTier::MainnetBlocked => crate::policy::ApprovalRequirement::Blocked,
        }
    }

    /// Approval level required for a sensitive production action.
    ///
    /// The agent's tier is a floor and the action's bar is the other floor; the
    /// stronger wins. A permissive tier can therefore never lower the bar for a
    /// runtime upgrade, a supply change, a key replacement, a genesis change or
    /// a settlement-rule change.
    pub fn required_approval_for(
        &self,
        action: crate::sensitive::SensitiveAction,
    ) -> crate::policy::ApprovalRequirement {
        let action_requirement = crate::sensitive::required_requirement(action);
        if rank(&action_requirement) >= rank(&self.tier_requirement()) {
            action_requirement
        } else {
            self.tier_requirement()
        }
    }

    /// Approval level required for a named operation.
    ///
    /// This used to ignore `operation` entirely and answer from the tier alone,
    /// so naming a runtime upgrade was indistinguishable from naming a docs
    /// edit. A name this crate cannot classify is `Blocked`: an operation must
    /// be one of the documented ones to be authorized at all, otherwise an
    /// agent could dodge the mapping by choosing its own label.
    pub fn required_approval(&self, operation: &str) -> crate::policy::ApprovalRequirement {
        match crate::sensitive::SensitiveAction::from_operation(operation) {
            Some(action) => self.required_approval_for(action),
            None => crate::policy::ApprovalRequirement::Blocked,
        }
    }
}

/// Ordering of the approval ladder, strongest last. `Blocked` is the top:
/// nothing satisfies it.
fn rank(requirement: &crate::policy::ApprovalRequirement) -> u8 {
    use crate::policy::ApprovalRequirement as R;
    match requirement {
        R::None => 0,
        R::HumanReview => 1,
        R::SecurityReview => 2,
        R::GovernanceReview => 3,
        R::Blocked => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::ApprovalRequirement as R;
    use crate::sensitive::SensitiveAction;

    #[test]
    fn a_permissive_tier_cannot_lower_a_sensitive_action() {
        let permissive = Permissions::new(AgentKind::Marketing, AgentPermissionTier::ReadOnly);
        assert_eq!(permissive.tier_requirement(), R::HumanReview);
        assert_eq!(
            permissive.required_approval_for(SensitiveAction::RuntimeUpgrade),
            R::SecurityReview,
            "read-only tier must not drop the bar for a runtime upgrade"
        );
        assert_eq!(
            permissive.required_approval_for(SensitiveAction::TokenSupplyChange),
            R::GovernanceReview
        );
    }

    #[test]
    fn an_unknown_operation_name_is_blocked() {
        let agent = Permissions::new(AgentKind::Fixer, AgentPermissionTier::DocsTestsReports);
        assert_eq!(
            agent.required_approval("runtime-upgrade"),
            R::SecurityReview,
            "a documented sensitive name resolves to its bar"
        );
        assert_eq!(
            agent.required_approval("runtime_upgrade_v2"),
            R::Blocked,
            "a near-miss name must not be authorized at a weaker level"
        );
    }

    #[test]
    fn a_blocked_tier_stays_blocked() {
        let blocked = Permissions::new(AgentKind::Fixer, AgentPermissionTier::MainnetBlocked);
        assert_eq!(
            blocked.required_approval_for(SensitiveAction::GenesisModification),
            R::Blocked
        );
    }
}
