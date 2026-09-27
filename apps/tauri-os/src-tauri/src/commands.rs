//! The Tauri command layer.
//!
//! Each command is a two-line wrapper: a pure `async fn` that takes the client
//! it needs, and a `#[tauri::command]` that hands it the one the app was built
//! with. The split is what makes the layer testable without a webview, and it is
//! why no command here contains a URL, a literal port, or a default value.

use crate::chain::ChainClient;
use crate::error::{IpcError, ServiceError};
use crate::models::{
    FinalizedHead, NodeStatus, ServiceStatus, SwarmHealth, SwarmTask, SystemHealth,
    SystemMetricsData, ValidatorHealth, ValidatorStatus,
};
use crate::probe::ServiceProbe;
use crate::state::{read_snapshot, ConsoleState};
use crate::swarm::SwarmClient;
use tauri::State;

/// The local services the GPU/inferstructor panel watches. Ports and names are
/// the ones the daemons bind (`docs/INFRASTRUCTURE_SETUP.md`, the GPU lane
/// services under `services/`).
pub const LOCAL_SERVICES: &[(&str, u16)] = &[
    ("GPU Lane 1", 9001),
    ("GPU Lane 2", 9002),
    ("GPU Lane 3", 9003),
    ("TPS Bridge", 9999),
    ("Validator Registry", 7001),
    ("RPC Proxy", 8899),
    ("Admin API", 7777),
];

/// When a read happened. Stamped by the console, never by the service, so a
/// cached answer cannot claim to be current.
pub fn observed_at() -> String {
    chrono::Utc::now().to_rfc3339()
}

/* ─── Chain status ─── */

/// Everything the console shows about the chain, read live from the node.
pub async fn node_status(client: &dyn ChainClient) -> Result<NodeStatus, ServiceError> {
    let health = client.system_health().await?;
    let finalized = client.finalized_head().await?;
    let identity = client.node_identity().await?;
    let role = client.node_role().await?;
    Ok(NodeStatus {
        running: true,
        finalized,
        peers: health.peers,
        is_syncing: health.is_syncing,
        name: identity.name,
        version: identity.version,
        chain: identity.chain,
        role,
        observed_at: observed_at(),
    })
}

/// The verdict rule, in one place so it can be read and tested:
///
/// 1. the node says it is syncing → `Syncing`;
/// 2. finality is still at genesis → `Stalled`;
/// 3. the node expects peers and has none → `Isolated`;
/// 4. otherwise → `Healthy`.
///
/// Nothing here is inferred beyond what the node reported, and nothing is
/// upgraded: an unknown is never healthy.
pub fn validator_verdict(
    health: &SystemHealth,
    finalized: &FinalizedHead,
) -> (ValidatorStatus, Vec<String>) {
    let mut notes = Vec::new();
    let status = if health.is_syncing {
        notes.push(
            "the node reports isSyncing=true, so its view of the tip is not settled".to_owned(),
        );
        ValidatorStatus::Syncing
    } else if finalized.number == 0 {
        notes.push("the finalized head is still the genesis block".to_owned());
        ValidatorStatus::Stalled
    } else if health.should_have_peers && health.peers == 0 {
        notes.push(
            "the node has no peers while it says it should have them: it is not following any chain"
                .to_owned(),
        );
        ValidatorStatus::Isolated
    } else {
        ValidatorStatus::Healthy
    };
    if health.peers == 0 && !health.should_have_peers {
        notes.push(
            "no peers, and the node reports it should have none (a self-contained chain)"
                .to_owned(),
        );
    }
    (status, notes)
}

/// Validator health: the same reads as [`node_status`], plus the verdict above.
pub async fn validator_health(client: &dyn ChainClient) -> Result<ValidatorHealth, ServiceError> {
    let health = client.system_health().await?;
    let finalized = client.finalized_head().await?;
    let role = client.node_role().await?;
    let (status, notes) = validator_verdict(&health, &finalized);
    Ok(ValidatorHealth {
        status,
        finalized,
        peers: health.peers,
        is_syncing: health.is_syncing,
        should_have_peers: health.should_have_peers,
        role,
        observed_at: observed_at(),
        notes,
    })
}

/* ─── Swarm task queue ─── */

pub async fn swarm_tasks(client: &dyn SwarmClient) -> Result<Vec<SwarmTask>, ServiceError> {
    client.tasks().await
}

pub async fn swarm_status(client: &dyn SwarmClient) -> Result<SwarmHealth, ServiceError> {
    client.health().await
}

pub async fn swarm_approve(
    client: &dyn SwarmClient,
    task_id: &str,
) -> Result<SwarmTask, ServiceError> {
    client.approve(task_id).await
}

pub async fn swarm_reject(
    client: &dyn SwarmClient,
    task_id: &str,
) -> Result<SwarmTask, ServiceError> {
    client.reject(task_id).await
}

/* ─── Local service reachability ─── */

/// Probe every local service. This one cannot fail: "which of these seven ports
/// answers" is a question the answer *is* the list. A port that does not answer
/// is reported with `healthy: false` and the probe's own reason.
pub async fn service_statuses(
    probe: &dyn ServiceProbe,
    services: &[(&str, u16)],
) -> Vec<ServiceStatus> {
    let mut results = Vec::with_capacity(services.len());
    for (name, port) in services {
        let url = format!("http://127.0.0.1:{port}/health");
        let (healthy, error) = match probe.probe(&url).await {
            Ok(healthy) => (healthy, None),
            Err(error) => (false, Some(error.to_string())),
        };
        results.push(ServiceStatus {
            name: (*name).to_owned(),
            port: *port,
            healthy,
            error,
        });
    }
    results
}

/* ─── The Tauri surface ─── */
//
// Thin wrappers only. If a command here ever grows logic, it belongs in the
// function above it, where a test can reach it.

#[tauri::command]
pub async fn get_node_status(state: State<'_, ConsoleState>) -> Result<NodeStatus, IpcError> {
    node_status(state.chain.as_ref())
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn get_validator_health(
    state: State<'_, ConsoleState>,
) -> Result<ValidatorHealth, IpcError> {
    validator_health(state.chain.as_ref())
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn get_system_metrics(
    state: State<'_, ConsoleState>,
) -> Result<SystemMetricsData, IpcError> {
    // Tauri requires a `Result` from an async command that borrows its state;
    // reading a snapshot cannot fail, so this is always `Ok`.
    Ok(read_snapshot(&state.system_metrics))
}

#[tauri::command]
pub async fn swarm_get_tasks(state: State<'_, ConsoleState>) -> Result<Vec<SwarmTask>, IpcError> {
    swarm_tasks(state.swarm.as_ref())
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn swarm_get_health(state: State<'_, ConsoleState>) -> Result<SwarmHealth, IpcError> {
    swarm_status(state.swarm.as_ref())
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn swarm_approve_task(
    state: State<'_, ConsoleState>,
    task_id: String,
) -> Result<SwarmTask, IpcError> {
    swarm_approve(state.swarm.as_ref(), &task_id)
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn swarm_reject_task(
    state: State<'_, ConsoleState>,
    task_id: String,
) -> Result<SwarmTask, IpcError> {
    swarm_reject(state.swarm.as_ref(), &task_id)
        .await
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn inferstructor_check_services(
    state: State<'_, ConsoleState>,
) -> Result<Vec<ServiceStatus>, IpcError> {
    // Same Tauri constraint as `get_system_metrics`: a probe that fails is
    // reported inside the list, so the command itself always answers.
    Ok(service_statuses(state.probe.as_ref(), LOCAL_SERVICES).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::ChainFuture;
    use crate::error::ServiceErrorKind;
    use crate::models::NodeIdentity;
    use crate::probe::ProbeFuture;
    use crate::swarm::SwarmFuture;
    use serde_json::Value;

    /* ── test doubles: the two answers a client can give, plus a broken one ── */

    /// A node that answers every read with fixed, plausible values.
    struct AnsweringChain {
        peers: u64,
        is_syncing: bool,
        should_have_peers: bool,
        number: u64,
        role: Option<&'static str>,
    }

    impl ChainClient for AnsweringChain {
        fn rpc_call<'a>(&'a self, method: &'a str, _params: Value) -> ChainFuture<'a, Value> {
            Box::pin(async move {
                Err(ServiceError::unreachable(
                    method,
                    "test-double",
                    "this double answers through the typed methods only",
                ))
            })
        }

        fn system_health(&self) -> ChainFuture<'_, SystemHealth> {
            Box::pin(async move {
                Ok(SystemHealth {
                    peers: self.peers,
                    is_syncing: self.is_syncing,
                    should_have_peers: self.should_have_peers,
                })
            })
        }

        fn finalized_head(&self) -> ChainFuture<'_, FinalizedHead> {
            Box::pin(async move {
                Ok(FinalizedHead {
                    hash: "0xabc".to_owned(),
                    number: self.number,
                })
            })
        }

        fn node_identity(&self) -> ChainFuture<'_, NodeIdentity> {
            Box::pin(async move {
                Ok(NodeIdentity {
                    name: "x3-chain-node".to_owned(),
                    version: "0.1.0".to_owned(),
                    chain: "X3 Development".to_owned(),
                })
            })
        }

        fn node_role(&self) -> ChainFuture<'_, Option<String>> {
            Box::pin(async move { Ok(self.role.map(str::to_owned)) })
        }
    }

    /// A node that is not there at all.
    struct DeadChain;

    impl ChainClient for DeadChain {
        fn rpc_call<'a>(&'a self, method: &'a str, _params: Value) -> ChainFuture<'a, Value> {
            Box::pin(async move {
                Err(ServiceError::unreachable(
                    method,
                    "http://127.0.0.1:1",
                    "connection refused",
                ))
            })
        }
    }

    struct StubSwarm {
        tasks: Vec<SwarmTask>,
        answer: Result<SwarmTask, ServiceError>,
    }

    fn task(id: &str) -> SwarmTask {
        SwarmTask {
            id: id.to_owned(),
            title: "Audit core runtime path guard".to_owned(),
            feature: "swarm-forbidden-path".to_owned(),
            agent: "swarm-guard".to_owned(),
            permission_tier: "constrained".to_owned(),
            allowed_paths: vec!["crates/x3-swarm-core/src".to_owned()],
            forbidden_paths: vec!["./.git".to_owned()],
            required_commands: vec![],
            status: "Pending".to_owned(),
            approval_required: "manual".to_owned(),
            risk: "medium".to_owned(),
        }
    }

    impl SwarmClient for StubSwarm {
        fn get<'a>(&'a self, path: &'a str) -> SwarmFuture<'a, Value> {
            Box::pin(async move {
                Err(ServiceError::unreachable(
                    path,
                    "test-double",
                    "use the typed methods",
                ))
            })
        }

        fn post<'a>(&'a self, path: &'a str) -> SwarmFuture<'a, Value> {
            Box::pin(async move {
                Err(ServiceError::unreachable(
                    path,
                    "test-double",
                    "use the typed methods",
                ))
            })
        }

        fn tasks(&self) -> SwarmFuture<'_, Vec<SwarmTask>> {
            Box::pin(async move { Ok(self.tasks.clone()) })
        }

        fn approve(&self, _task_id: &str) -> SwarmFuture<'_, SwarmTask> {
            Box::pin(async move { self.answer.clone() })
        }

        fn reject(&self, _task_id: &str) -> SwarmFuture<'_, SwarmTask> {
            Box::pin(async move { self.answer.clone() })
        }
    }

    struct StubProbe {
        answer: Result<bool, ServiceError>,
    }

    impl ServiceProbe for StubProbe {
        fn probe<'a>(&'a self, _url: &'a str) -> ProbeFuture<'a> {
            Box::pin(async move { self.answer.clone() })
        }
    }

    /* ── chain status ── */

    #[tokio::test]
    async fn a_node_that_answers_produces_status_with_its_own_numbers() {
        let chain = AnsweringChain {
            peers: 7,
            is_syncing: false,
            should_have_peers: true,
            number: 4242,
            role: Some("authority"),
        };
        let status = node_status(&chain).await.unwrap();
        assert!(status.running);
        assert_eq!(status.finalized.number, 4242);
        assert_eq!(status.finalized.hash, "0xabc");
        assert_eq!(status.peers, 7);
        assert!(!status.is_syncing);
        assert_eq!(status.name, "x3-chain-node");
        assert_eq!(status.chain, "X3 Development");
        assert_eq!(status.role.as_deref(), Some("authority"));
        assert!(!status.observed_at.is_empty());
    }

    #[tokio::test]
    async fn a_node_that_does_not_answer_produces_an_error_not_a_default_status() {
        let error = node_status(&DeadChain).await.unwrap_err();
        assert!(matches!(error.kind, ServiceErrorKind::Unreachable(_)));
        let ipc = IpcError::from(error);
        assert_eq!(ipc.code, crate::error::IPC_SERVICE_UNREACHABLE);
    }

    #[tokio::test]
    async fn a_node_that_does_not_report_a_role_does_not_have_one_invented_for_it() {
        let chain = AnsweringChain {
            peers: 1,
            is_syncing: false,
            should_have_peers: true,
            number: 10,
            role: None,
        };
        assert_eq!(node_status(&chain).await.unwrap().role, None);
    }

    /* ── validator verdicts ── */

    fn head(number: u64) -> FinalizedHead {
        FinalizedHead {
            hash: "0xabc".to_owned(),
            number,
        }
    }

    fn health(peers: u64, is_syncing: bool, should_have_peers: bool) -> SystemHealth {
        SystemHealth {
            peers,
            is_syncing,
            should_have_peers,
        }
    }

    #[test]
    fn a_syncing_node_is_never_reported_healthy() {
        let (status, notes) = validator_verdict(&health(3, true, true), &head(500));
        assert_eq!(status, ValidatorStatus::Syncing);
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn finality_at_genesis_is_stalled_even_though_the_node_answers() {
        let (status, notes) = validator_verdict(&health(3, false, true), &head(0));
        assert_eq!(status, ValidatorStatus::Stalled);
        assert!(notes[0].contains("genesis"));
    }

    #[test]
    fn a_node_with_no_peers_that_expects_them_is_isolated() {
        let (status, notes) = validator_verdict(&health(0, false, true), &head(90));
        assert_eq!(status, ValidatorStatus::Isolated);
        assert!(notes[0].contains("no peers"));
    }

    #[test]
    fn a_self_contained_dev_chain_is_healthy_and_says_why_it_has_no_peers() {
        let (status, notes) = validator_verdict(&health(0, false, false), &head(12));
        assert_eq!(status, ValidatorStatus::Healthy);
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("should have none"));
    }

    #[test]
    fn a_syncing_node_is_never_downgraded_to_healthy_by_a_high_finalized_head() {
        // The precedence rule, pinned: syncing wins over a good-looking height.
        let (status, _) = validator_verdict(&health(9, true, true), &head(1_000_000));
        assert_eq!(status, ValidatorStatus::Syncing);
    }

    #[tokio::test]
    async fn validator_health_carries_the_evidence_it_was_derived_from() {
        let chain = AnsweringChain {
            peers: 0,
            is_syncing: false,
            should_have_peers: true,
            number: 77,
            role: Some("full"),
        };
        let health = validator_health(&chain).await.unwrap();
        assert_eq!(health.status, ValidatorStatus::Isolated);
        assert_eq!(health.finalized.number, 77);
        assert_eq!(health.role.as_deref(), Some("full"));
    }

    /* ── swarm ── */

    #[tokio::test]
    async fn a_dead_swarm_api_is_an_error_rather_than_a_stale_list() {
        let swarm = StubSwarm {
            tasks: vec![],
            answer: Err(ServiceError::unreachable(
                "POST /tasks/{id}/approve",
                "http://127.0.0.1:8787",
                "connection refused",
            )),
        };
        let error = swarm_approve(&swarm, "x3-task-0001").await.unwrap_err();
        assert!(matches!(error.kind, ServiceErrorKind::Unreachable(_)));
    }

    #[tokio::test]
    async fn the_task_list_is_the_clients_list_and_not_a_cache() {
        let swarm = StubSwarm {
            tasks: vec![task("x3-task-0001")],
            answer: Ok(task("x3-task-0001")),
        };
        let tasks = swarm_tasks(&swarm).await.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "x3-task-0001");
        assert_eq!(tasks[0].permission_tier, "constrained");
    }

    #[tokio::test]
    async fn approving_a_task_returns_the_task_the_service_returned() {
        let swarm = StubSwarm {
            tasks: vec![],
            answer: Ok(task("x3-task-0007")),
        };
        assert_eq!(
            swarm_approve(&swarm, "x3-task-0007").await.unwrap().id,
            "x3-task-0007"
        );
    }

    /* ── local services ── */

    #[tokio::test]
    async fn a_probe_that_refuses_is_reported_next_to_the_service_it_refused() {
        let probe = StubProbe {
            answer: Err(ServiceError::unreachable(
                "http://127.0.0.1:9001/health",
                "http://127.0.0.1:9001/health",
                "connection refused",
            )),
        };
        let statuses = service_statuses(&probe, &[("GPU Lane 1", 9001)]).await;
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].name, "GPU Lane 1");
        assert_eq!(statuses[0].port, 9001);
        assert!(!statuses[0].healthy);
        let reason = statuses[0].error.as_deref().unwrap_or_default();
        assert!(reason.contains("connection refused"), "{reason}");
    }

    #[tokio::test]
    async fn a_service_that_answers_500_is_reachable_but_unhealthy() {
        let probe = StubProbe { answer: Ok(false) };
        let statuses = service_statuses(&probe, &[("RPC Proxy", 8899)]).await;
        assert!(!statuses[0].healthy);
        assert_eq!(statuses[0].error, None, "an answer is not an error");
    }

    #[tokio::test]
    async fn every_service_in_the_panel_is_probed_exactly_once() {
        let probe = StubProbe { answer: Ok(true) };
        let statuses = service_statuses(&probe, LOCAL_SERVICES).await;
        assert_eq!(statuses.len(), LOCAL_SERVICES.len());
        assert!(statuses.iter().all(|status| status.healthy));
        assert_eq!(statuses[0].name, "GPU Lane 1");
    }
}
