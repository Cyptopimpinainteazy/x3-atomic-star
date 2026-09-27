//! The payloads the operator console hands to the webview.
//!
//! Field names are part of the IPC contract with
//! `apps/tauri-os/src/lib/telemetry.ts` and
//! `apps/tauri-os/src/apps/SwarmCommand/SwarmCommand.tsx`; changing one without
//! the other silently breaks the panel, so each group below names its consumer.

use serde::{Deserialize, Serialize};

/* ─── Node status / validator health (apps/tauri-os/src/lib/telemetry.ts) ─── */

/// The chain's finalized head, as the node itself reports it: the hash of the
/// block `chain_getFinalizedHead` named, and the `number` in that block's own
/// header. Both come from the node — neither is derived from a best-block read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalizedHead {
    pub hash: String,
    pub number: u64,
}

/// `system_health`, verbatim. `should_have_peers` is what lets a one-node dev
/// chain report zero peers without being called isolated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemHealth {
    pub peers: u64,
    pub is_syncing: bool,
    pub should_have_peers: bool,
}

/// What the node calls itself (`system_name` / `system_version` / `system_chain`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeIdentity {
    pub name: String,
    pub version: String,
    pub chain: String,
}

/// A live read of the chain the operator is running.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStatus {
    /// Always `true`: the struct only exists if the node answered. It is kept so
    /// the webview can keep its one shape for "connected" and read the failure
    /// from the command's error instead.
    pub running: bool,
    pub finalized: FinalizedHead,
    pub peers: u64,
    pub is_syncing: bool,
    pub name: String,
    pub version: String,
    pub chain: String,
    /// `system_nodeRoles`, when the node serves that method. `None` means the
    /// node did not report a role — it never means "assume full".
    pub role: Option<String>,
    pub observed_at: String,
}

/// The verdict for a validator, derived from the node's own answers by a rule
/// stated in `commands::validator_health`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidatorStatus {
    /// Answering, not syncing, and past genesis.
    Healthy,
    /// Answering, but the node says it is still catching up.
    Syncing,
    /// Answering and past genesis, but with no peers while the node expects
    /// them — it is not following anyone's chain.
    Isolated,
    /// Answering and not syncing, but finality is still at genesis.
    Stalled,
}

/// Everything the console knows about a validator, with the evidence attached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorHealth {
    pub status: ValidatorStatus,
    pub finalized: FinalizedHead,
    pub peers: u64,
    pub is_syncing: bool,
    pub should_have_peers: bool,
    pub role: Option<String>,
    pub observed_at: String,
    /// Why the verdict is not `Healthy`, in the order the facts were read.
    pub notes: Vec<String>,
}

/* ─── System metrics (apps/tauri-os/src/lib/telemetry.ts) ─── */

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMetricsData {
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub disk: Vec<DiskMetrics>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuMetrics {
    pub usage_percent: f32,
    pub cores: u32,
    pub frequency: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryMetrics {
    pub used: u64,
    pub total: u64,
    pub usage_percent: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskMetrics {
    pub name: String,
    pub used: u64,
    pub total: u64,
    pub usage_percent: f32,
}

/* ─── Swarm task queue (apps/tauri-os/src/apps/SwarmCommand/SwarmCommand.tsx) ─── */

/// `/health` from `services/x3-swarm-api`. Field names are the service's own
/// snake_case body — this struct is a mirror of the wire format, not a
/// re-spelling of it, because the webview reads these keys directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwarmHealth {
    pub service: String,
    pub status: String,
    pub mode: String,
    pub agents_enabled: bool,
    pub kill_switch: bool,
}

/// One row of `/tasks` (`crates/x3-swarm-core::task::AgentTask`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwarmTask {
    pub id: String,
    pub title: String,
    pub feature: String,
    pub agent: String,
    pub permission_tier: String,
    pub allowed_paths: Vec<String>,
    pub forbidden_paths: Vec<String>,
    pub required_commands: Vec<String>,
    pub status: String,
    pub approval_required: String,
    pub risk: String,
}

/* ─── Service reachability ─── */

/// One local service the operator console knows about. `healthy` is the result
/// of a real request to the port; a service that does not answer is reported
/// with `healthy: false`, which is an answer, not a failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    pub port: u16,
    pub healthy: bool,
    /// `None` when the service answered; the transport's own words when it did
    /// not, so an operator can tell "down" from "slow" from "refusing".
    pub error: Option<String>,
}

/* ─── Push envelopes ─── */

/// The payload of the `os:node_status` event. A push has no caller to return an
/// error to, so the envelope carries either the read or the failure — the
/// webview always knows which of the two it is looking at.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStatusEvent {
    pub status: Option<NodeStatus>,
    pub error: Option<crate::error::IpcError>,
}

impl NodeStatusEvent {
    pub fn ok(status: NodeStatus) -> Self {
        Self {
            status: Some(status),
            error: None,
        }
    }

    pub fn failed(error: crate::error::IpcError) -> Self {
        Self {
            status: None,
            error: Some(error),
        }
    }
}
