use crate::policy::ApprovalRequirement;

/// Approval gate for high-risk operations.
pub struct ApprovalGate {
    requirement: ApprovalRequirement,
}

impl ApprovalGate {
    pub fn new(req: ApprovalRequirement) -> Self {
        Self { requirement: req }
    }

    pub fn request_approval(&self, context: &str) -> bool {
        match self.requirement {
            ApprovalRequirement::None => true,
            ApprovalRequirement::HumanReview => {
                println!("Human review required for: {}", context);
                false // Stub: wait for input
            }
            ApprovalRequirement::Blocked => false,
            _ => false,
        }
    }

    pub fn grant(&mut self) {
        self.requirement = ApprovalRequirement::None;
    }
}
