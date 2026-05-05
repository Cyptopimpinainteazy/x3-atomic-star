use serde::{Deserialize, Serialize};
use crate::AgentKind;
use chrono::Utc;

/// Persistent memory entry for agent learnings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwarmMemoryEntry {
    pub id: String,
    pub agent: AgentKind,
    pub feature: String,
    pub finding: String,
    pub severity: String,
    pub test_added: Option<String>,
    pub fix_commit: Option<String>,
    pub result: String,
    pub timestamp: String,
}

impl SwarmMemoryEntry {
    pub fn new(id: String, agent: AgentKind, feature: String, finding: String) -> Self {
        Self {
            id,
            agent,
            feature,
            finding,
            severity: "medium".to_string(),
            test_added: None,
            fix_commit: None,
            result: "observed".to_string(),
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

/// In-memory store (append-only).
pub struct AgentMemory {
    entries: Vec<SwarmMemoryEntry>,
}

impl AgentMemory {
    pub fn new() -> Self {
        Self { entries: vec![] }
    }

    pub fn add(&mut self, entry: SwarmMemoryEntry) {
        self.entries.push(entry);
    }

    pub fn query(&self, agent: Option<AgentKind>, feature: Option<&str>) -> Vec<&SwarmMemoryEntry> {
        self.entries.iter()
            .filter(|e| match &agent {
                Some(a) => &e.agent == a,
                None => true,
            })
            .filter(|e| match feature {
                Some(f) => e.feature == f,
                None => true,
            })
            .collect()
    }

    pub fn entries(&self) -> &[SwarmMemoryEntry] {
        &self.entries
    }
}

pub fn append_memory_entry(entries: &mut Vec<SwarmMemoryEntry>, entry: SwarmMemoryEntry) {
    entries.push(entry);
}

pub fn load_memory_entries(entries: &[SwarmMemoryEntry]) -> Vec<SwarmMemoryEntry> {
    entries.to_vec()
}
