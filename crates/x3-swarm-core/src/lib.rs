//! X3 Swarm Core - Autonomous Agent System
//! 
//! This crate provides the core infrastructure for X3's autonomous swarm agents
//! that can scan, build, test, audit, break, fix, and prove the system.

pub mod agent;
pub mod approval;
pub mod events;
pub mod guard;
pub mod memory;
pub mod permissions;
pub mod policy;
pub mod report;
pub mod scheduler;
pub mod scoreboard;
pub mod task;

pub use agent::{AgentKind, AgentPermissionTier};
pub use approval::ApprovalGate;
pub use events::SwarmEvent;
pub use guard::{evaluate_path, ForbiddenPathGuard, GuardAction};
pub use memory::SwarmMemoryEntry;
pub use policy::{AgentPolicy, ApprovalRequirement, default_agent_policies};
pub use report::SwarmReport;
pub use scheduler::SwarmScheduler;
pub use scoreboard::SwarmScoreboard;
pub use task::{AgentResult, AgentTask, TaskStatus};
