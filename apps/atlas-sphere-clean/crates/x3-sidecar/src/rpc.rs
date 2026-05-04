//! RPC server for x3-sidecar

use crate::job::{Job, JobQueue};
use crate::{ChainSubmitter, ExecutionReceipt, JobStatusEntry, SidecarState, Telemetry};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// RPC server state
pub struct RpcState {
    pub job_queue: Arc<RwLock<JobQueue>>,
    pub sidecar_state: Arc<RwLock<SidecarState>>,
    pub submitter: Arc<ChainSubmitter>,
    pub telemetry: Arc<Telemetry>,
}

/// Create the RPC router
pub fn create_router(state: Arc<RpcState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/jobs", post(submit_job))
        .route("/jobs/:id", get(get_job))
        .route("/jobs/:id/status", get(query_job_status))
        .route("/jobs/:id/cancel", post(cancel_job))
        .route("/receipts/submit", post(submit_receipt))
        .route("/queue/stats", get(queue_stats))
        .route("/queue/clear", post(clear_queue))
        .route("/metrics", get(metrics))
        .with_state(state)
}

/// Create a dedicated metrics router (telemetry-only surface).
pub fn create_metrics_router(state: Arc<RpcState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .with_state(state)
}

/// Health check
async fn health() -> &'static str {
    "OK"
}

/// Status response
#[derive(Serialize)]
struct StatusResponse {
    version: &'static str,
    uptime_secs: u64,
    jobs_executed: u64,
    jobs_pending: usize,
    tracked_jobs: usize,
    executor_registered: bool,
}

async fn status(State(state): State<Arc<RpcState>>) -> Json<StatusResponse> {
    let queue = state.job_queue.read().await;
    let sidecar = state.sidecar_state.read().await;

    Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION"),
        uptime_secs: sidecar.start_time.elapsed().as_secs(),
        jobs_executed: sidecar.jobs_completed,
        jobs_pending: queue.len(),
        tracked_jobs: sidecar.job_statuses.len(),
        executor_registered: sidecar.registered,
    })
}

/// Job submission request
#[derive(Deserialize)]
struct SubmitJobRequest {
    /// Bytecode in hex
    bytecode: String,
    /// Input data in hex
    input: Option<String>,
    /// Gas limit
    gas_limit: Option<u64>,
    /// Priority (1-10)
    priority: Option<u8>,
    /// Callback URL for completion notification
    callback_url: Option<String>,
}

/// Job submission response
#[derive(Serialize)]
struct SubmitJobResponse {
    job_id: String,
    position: usize,
}

async fn submit_job(
    State(state): State<Arc<RpcState>>,
    Json(request): Json<SubmitJobRequest>,
) -> Result<Json<SubmitJobResponse>, (StatusCode, String)> {
    // Parse bytecode
    let bytecode = hex::decode(request.bytecode.trim_start_matches("0x")).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid bytecode hex: {}", e),
        )
    })?;

    // Parse input
    let input = match request.input {
        Some(hex) => hex::decode(hex.trim_start_matches("0x"))
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid input hex: {}", e)))?,
        None => vec![],
    };

    // Create job
    let job = Job {
        id: generate_job_id(),
        bytecode,
        input,
        gas_limit: request.gas_limit.unwrap_or(1_000_000),
        priority: request.priority.unwrap_or(5),
        callback_url: request.callback_url,
        submitted_at: std::time::Instant::now(),
        started_at: None,
    };

    let job_id = hex::encode(job.id);

    // Add to queue
    let mut queue = state.job_queue.write().await;
    let queued_job_id = job.id;
    queue.push(job);
    state.telemetry.record_job_received();
    let position = queue.len();
    drop(queue);

    let mut sidecar = state.sidecar_state.write().await;
    sidecar
        .job_statuses
        .insert(queued_job_id, JobStatusEntry::new("pending", None, None));

    Ok(Json(SubmitJobResponse { job_id, position }))
}

/// Job info response
#[derive(Serialize)]
struct JobInfoResponse {
    job_id: String,
    status: String,
    gas_limit: Option<u64>,
    priority: Option<u8>,
    queued_for_secs: Option<u64>,
    tx_hash: Option<String>,
    error: Option<String>,
    updated_at_unix: Option<u64>,
}

async fn get_job(
    State(state): State<Arc<RpcState>>,
    Path(job_id): Path<String>,
) -> Result<Json<JobInfoResponse>, (StatusCode, String)> {
    let id_bytes = parse_job_id(&job_id)?;

    let tracked = {
        let sidecar = state.sidecar_state.read().await;
        sidecar.job_statuses.get(&id_bytes).cloned()
    };

    let queued = {
        let queue = state.job_queue.read().await;
        let found = queue.iter().find(|job| job.id == id_bytes).map(|job| {
            (
                job.gas_limit,
                job.priority,
                job.submitted_at.elapsed().as_secs(),
                if job.started_at.is_some() {
                    "running".to_string()
                } else {
                    "pending".to_string()
                },
            )
        });
        found
    };

    if let Some((gas_limit, priority, queued_for_secs, queue_status)) = queued {
        let tracked_status = tracked
            .as_ref()
            .map(|entry| entry.status.clone())
            .unwrap_or(queue_status);
        return Ok(Json(JobInfoResponse {
            job_id: hex::encode(id_bytes),
            status: tracked_status,
            gas_limit: Some(gas_limit),
            priority: Some(priority),
            queued_for_secs: Some(queued_for_secs),
            tx_hash: tracked.as_ref().and_then(|entry| entry.tx_hash.clone()),
            error: tracked.as_ref().and_then(|entry| entry.error.clone()),
            updated_at_unix: tracked.as_ref().map(|entry| entry.updated_at_unix),
        }));
    }

    if let Some(entry) = tracked {
        return Ok(Json(JobInfoResponse {
            job_id: hex::encode(id_bytes),
            status: entry.status,
            gas_limit: None,
            priority: None,
            queued_for_secs: None,
            tx_hash: entry.tx_hash,
            error: entry.error,
            updated_at_unix: Some(entry.updated_at_unix),
        }));
    }

    Err((StatusCode::NOT_FOUND, "Job not found".to_string()))
}

async fn cancel_job(
    State(state): State<Arc<RpcState>>,
    Path(job_id): Path<String>,
) -> Result<&'static str, (StatusCode, String)> {
    let id_bytes = parse_job_id(&job_id)?;

    let mut queue = state.job_queue.write().await;

    if queue.remove(&id_bytes) {
        drop(queue);
        let mut sidecar = state.sidecar_state.write().await;
        sidecar
            .job_statuses
            .insert(id_bytes, JobStatusEntry::new("cancelled", None, None));
        state.telemetry.record_job_cancelled();
        Ok("Job cancelled")
    } else {
        Err((
            StatusCode::NOT_FOUND,
            "Job not found or already running".to_string(),
        ))
    }
}

#[derive(Deserialize)]
struct SubmitReceiptRequest {
    receipt: ExecutionReceipt,
}

#[derive(Serialize)]
struct SubmitReceiptResponse {
    job_id: String,
    tx_hash: String,
}

async fn submit_receipt(
    State(state): State<Arc<RpcState>>,
    Json(request): Json<SubmitReceiptRequest>,
) -> Result<Json<SubmitReceiptResponse>, (StatusCode, String)> {
    let tx_hash = state
        .submitter
        .submit_receipt(&request.receipt)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                format!("Failed to submit receipt to chain: {}", e),
            )
        })?;

    let mut sidecar = state.sidecar_state.write().await;
    sidecar.job_statuses.insert(
        request.receipt.job_id,
        JobStatusEntry::new("submitted", Some(tx_hash.clone()), None),
    );

    Ok(Json(SubmitReceiptResponse {
        job_id: hex::encode(request.receipt.job_id),
        tx_hash,
    }))
}

#[derive(Serialize)]
struct QueryJobStatusResponse {
    job_id: String,
    local_status: Option<String>,
    chain_status: Option<String>,
    tx_hash: Option<String>,
    error: Option<String>,
    updated_at_unix: Option<u64>,
}

async fn query_job_status(
    State(state): State<Arc<RpcState>>,
    Path(job_id): Path<String>,
) -> Result<Json<QueryJobStatusResponse>, (StatusCode, String)> {
    let id_bytes = parse_job_id(&job_id)?;

    let local = {
        let sidecar = state.sidecar_state.read().await;
        sidecar.job_statuses.get(&id_bytes).cloned()
    };

    let chain_status = state.submitter.get_job_status(id_bytes).await.ok();

    Ok(Json(QueryJobStatusResponse {
        job_id: hex::encode(id_bytes),
        local_status: local.as_ref().map(|entry| entry.status.clone()),
        chain_status,
        tx_hash: local.as_ref().and_then(|entry| entry.tx_hash.clone()),
        error: local.as_ref().and_then(|entry| entry.error.clone()),
        updated_at_unix: local.as_ref().map(|entry| entry.updated_at_unix),
    }))
}

/// Queue stats response
#[derive(Serialize)]
struct QueueStatsResponse {
    pending: usize,
    running: usize,
    completed: u64,
    failed: u64,
    avg_wait_time_ms: u64,
}

async fn queue_stats(State(state): State<Arc<RpcState>>) -> Json<QueueStatsResponse> {
    let queue = state.job_queue.read().await;
    let stats = queue.stats();

    Json(QueueStatsResponse {
        pending: stats.pending,
        running: stats.running,
        completed: stats.completed,
        failed: stats.failed,
        avg_wait_time_ms: stats.avg_wait_time_ms,
    })
}

async fn clear_queue(State(state): State<Arc<RpcState>>) -> &'static str {
    let mut queue = state.job_queue.write().await;
    let cancelled = queue.len() as u64;
    queue.clear();
    state.telemetry.record_jobs_cancelled(cancelled);
    "Queue cleared"
}

/// Prometheus metrics endpoint
async fn metrics(State(state): State<Arc<RpcState>>) -> String {
    let queue = state.job_queue.read().await;
    let sidecar = state.sidecar_state.read().await;
    let stats = queue.stats();
    let telemetry_metrics = state.telemetry.prometheus_format();

    format!(
        r#"{}# HELP x3_sidecar_queue_pending Number of pending jobs in local queue
# TYPE x3_sidecar_queue_pending gauge
x3_sidecar_queue_pending {}

# HELP x3_sidecar_queue_running Number of running jobs in local executor
# TYPE x3_sidecar_queue_running gauge
x3_sidecar_queue_running {}

# HELP x3_sidecar_tracked_jobs Number of jobs tracked in sidecar state
# TYPE x3_sidecar_tracked_jobs gauge
x3_sidecar_tracked_jobs {}

# HELP x3_sidecar_queue_avg_wait_time_ms Average queue wait time in milliseconds
# TYPE x3_sidecar_queue_avg_wait_time_ms gauge
x3_sidecar_queue_avg_wait_time_ms {}
"#,
        telemetry_metrics,
        stats.pending,
        stats.running,
        sidecar.job_statuses.len(),
        stats.avg_wait_time_ms
    )
}

fn parse_job_id(job_id: &str) -> Result<[u8; 32], (StatusCode, String)> {
    hex::decode(job_id.trim_start_matches("0x"))
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid job ID: {}", e)))?
        .try_into()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Job ID must be 32 bytes".to_string(),
            )
        })
}

/// Generate a unique job ID
fn generate_job_id() -> [u8; 32] {
    use blake2::{Blake2s256, Digest};

    let mut hasher = Blake2s256::new();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    hasher.update(nanos.to_le_bytes());
    hasher.update(&rand::random::<[u8; 16]>());

    let result = hasher.finalize();
    let mut id = [0u8; 32];
    id.copy_from_slice(&result);
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    fn test_state() -> Arc<RpcState> {
        Arc::new(RpcState {
            job_queue: Arc::new(RwLock::new(JobQueue::new())),
            sidecar_state: Arc::new(RwLock::new(SidecarState::default())),
            submitter: Arc::new(ChainSubmitter::new(
                "http://127.0.0.1:1".to_string(),
                "01".repeat(32),
            )),
            telemetry: Telemetry::new(),
        })
    }

    fn sample_bytecode_hex() -> &'static str {
        // X3VC magic + header + HALT
        "583356430001000000000000a1"
    }

    async fn submit_job_and_get_id(app: &Router) -> String {
        let req = Request::builder()
            .method("POST")
            .uri("/jobs")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({
                    "bytecode": sample_bytecode_hex(),
                    "priority": 7
                })
                .to_string(),
            ))
            .expect("request build");

        let resp = app.clone().oneshot(req).await.expect("submit request");
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body())
            .await
            .expect("body bytes");
        serde_json::from_slice::<serde_json::Value>(&body)
            .expect("submit response json")
            .get("job_id")
            .and_then(|v| v.as_str())
            .expect("job_id present")
            .to_string()
    }

    #[tokio::test]
    async fn submit_get_and_query_job_status_routes_work() {
        let app = create_router(test_state());
        let job_id = submit_job_and_get_id(&app).await;

        let get_req = Request::builder()
            .method("GET")
            .uri(format!("/jobs/{job_id}"))
            .body(Body::empty())
            .expect("request build");
        let get_resp = app.clone().oneshot(get_req).await.expect("get request");
        assert_eq!(get_resp.status(), StatusCode::OK);
        let get_body = hyper::body::to_bytes(get_resp.into_body())
            .await
            .expect("body bytes");
        let get_json: serde_json::Value =
            serde_json::from_slice(&get_body).expect("get response json");
        assert_eq!(
            get_json.get("status").and_then(|v| v.as_str()),
            Some("pending")
        );

        let status_req = Request::builder()
            .method("GET")
            .uri(format!("/jobs/{job_id}/status"))
            .body(Body::empty())
            .expect("request build");
        let status_resp = app
            .clone()
            .oneshot(status_req)
            .await
            .expect("status request");
        assert_eq!(status_resp.status(), StatusCode::OK);
        let status_body = hyper::body::to_bytes(status_resp.into_body())
            .await
            .expect("body bytes");
        let status_json: serde_json::Value =
            serde_json::from_slice(&status_body).expect("status response json");
        assert_eq!(
            status_json.get("local_status").and_then(|v| v.as_str()),
            Some("pending")
        );
    }

    #[tokio::test]
    async fn cancel_and_metrics_routes_update_telemetry() {
        let app = create_router(test_state());
        let job_id = submit_job_and_get_id(&app).await;

        let cancel_req = Request::builder()
            .method("POST")
            .uri(format!("/jobs/{job_id}/cancel"))
            .body(Body::empty())
            .expect("request build");
        let cancel_resp = app
            .clone()
            .oneshot(cancel_req)
            .await
            .expect("cancel request");
        assert_eq!(cancel_resp.status(), StatusCode::OK);

        let metrics_req = Request::builder()
            .method("GET")
            .uri("/metrics")
            .body(Body::empty())
            .expect("request build");
        let metrics_resp = app
            .clone()
            .oneshot(metrics_req)
            .await
            .expect("metrics request");
        assert_eq!(metrics_resp.status(), StatusCode::OK);
        let metrics_body = hyper::body::to_bytes(metrics_resp.into_body())
            .await
            .expect("body bytes");
        let metrics_text = String::from_utf8(metrics_body.to_vec()).expect("utf8 metrics");

        assert!(metrics_text.contains("x3_sidecar_jobs_received_total 1"));
        assert!(metrics_text.contains("x3_sidecar_jobs_cancelled_total 1"));
    }
}
