use serde::{Deserialize, Serialize};

/// Agent types in the X3 Swarm.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentKind {
    RepoScanner,
    FeatureMapper,
    TestBuilder,
    Integrator,
    BuildFixer,
    WiringInspector,
    Auditor,
    Breaker,
    Fixer,
    ReadinessReporter,
    Benchmark,
    Marketing,
    Grant,
    ApprovalGate,
}

/// Permission tiers controlling what agents can modify.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentPermissionTier {
    ReadOnly,
    DocsTestsReports,
    TauriServiceWiring,
    RuntimeProposalOnly,
    BridgeEconomicsProposalOnly,
    MainnetBlocked,
}

impl AgentPermissionTier {
    /// Check if tier allows path modification.
    pub fn allows_path(&self, path: &str) -> bool {
        match self {
            AgentPermissionTier::ReadOnly => false,
            AgentPermissionTier::DocsTestsReports => path.starts_with("docs/") || path.starts_with("reports/") || path.starts_with("tests/"),
            AgentPermissionTier::TauriServiceWiring => path.starts_with("apps/tauri-os/"),
            _ => false,
        }
    }
}
