use serde::{Deserialize, Serialize};
use crate::{AgentKind, TaskStatus};
use chrono::Utc;

/// Swarm events for logging and reactivity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SwarmEvent {
    TaskEnqueued { task_id: String, agent: AgentKind },
    TaskStarted { task_id: String },
    TaskCompleted { task_id: String, status: TaskStatus },
    ApprovalRequired { task_id: String },
    BlockerDetected { task_id: String, blocker: String },
    MemoryRecorded { entry_id: String },
}

impl SwarmEvent {
    pub fn timestamp(&self) -> String {
        Utc::now().to_rfc3339()
    }
}

/// Event bus (channel-based stub).
pub struct EventBus;

impl EventBus {
    pub fn emit(event: SwarmEvent) {
        // Stub: log/print in real impl
        println!("Event: {:?}", event);
    }

    pub fn subscribe() -> Vec<SwarmEvent> {
        vec![]
    }
}
