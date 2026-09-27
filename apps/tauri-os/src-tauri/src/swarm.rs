//! The console's read/write path into `services/x3-swarm-api`.
//!
//! The routes here are the service's real ones (`/health`, `/tasks`,
//! `/tasks/{id}/approve`, `/tasks/{id}/reject` — see
//! `services/x3-swarm-api/src/main.rs`). The console previously posted to
//! `/approve/{id}`, which the service has never served, and parsed `/tasks` into
//! a struct whose field names did not match the service's body, so both the
//! listing and the approve button failed silently into a cache. Both are fixed
//! here and pinned by tests.

use crate::error::ServiceError;
use crate::models::{SwarmHealth, SwarmTask};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

/// Where the swarm API listens unless the operator says otherwise.
pub const DEFAULT_SWARM_API_URL: &str = "http://127.0.0.1:8787";

/// Environment override for [`DEFAULT_SWARM_API_URL`].
pub const SWARM_API_URL_ENV: &str = "X3_OS_SWARM_API_URL";

/// A local service should answer quickly or be reported as down.
pub const SWARM_API_TIMEOUT: Duration = Duration::from_secs(3);

/// Boxed future, so the command layer can hold one `Arc<dyn SwarmClient>`.
pub type SwarmFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ServiceError>> + Send + 'a>>;

/// The swarm reads and writes the console needs. A failed request is an `Err`:
/// a stale task list is worse than an error, because an operator cannot tell it
/// apart from a current one.
pub trait SwarmClient: Send + Sync {
    fn get<'a>(&'a self, path: &'a str) -> SwarmFuture<'a, Value>;
    fn post<'a>(&'a self, path: &'a str) -> SwarmFuture<'a, Value>;

    fn health(&self) -> SwarmFuture<'_, SwarmHealth> {
        Box::pin(async move {
            let value = self.get("/health").await?;
            serde_json::from_value(value).map_err(|error| {
                ServiceError::malformed("GET /health", format!("not a swarm health body: {error}"))
            })
        })
    }

    fn tasks(&self) -> SwarmFuture<'_, Vec<SwarmTask>> {
        Box::pin(async move {
            let value = self.get("/tasks").await?;
            serde_json::from_value(value).map_err(|error| {
                ServiceError::malformed("GET /tasks", format!("not a swarm task list: {error}"))
            })
        })
    }

    fn approve<'a>(&'a self, task_id: &'a str) -> SwarmFuture<'a, SwarmTask> {
        Box::pin(async move {
            let path = task_action_path("approve", task_id)?;
            let value = self.post(&path).await?;
            serde_json::from_value(value).map_err(|error| {
                ServiceError::malformed(
                    "POST /tasks/{id}/approve",
                    format!("not a swarm task: {error}"),
                )
            })
        })
    }

    fn reject<'a>(&'a self, task_id: &'a str) -> SwarmFuture<'a, SwarmTask> {
        Box::pin(async move {
            let path = task_action_path("reject", task_id)?;
            let value = self.post(&path).await?;
            serde_json::from_value(value).map_err(|error| {
                ServiceError::malformed(
                    "POST /tasks/{id}/reject",
                    format!("not a swarm task: {error}"),
                )
            })
        })
    }
}

/// Build `/tasks/{id}/{action}`, refusing ids that would change the path.
///
/// The id comes from the webview, so it is attacker-influenced input on its way
/// into a URL: `../` or a query separator must never reach the request.
pub fn task_action_path(action: &str, task_id: &str) -> Result<String, ServiceError> {
    let request = format!("POST /tasks/{{id}}/{action}");
    if task_id.is_empty() {
        return Err(ServiceError::rejected(request, "the task id is empty"));
    }
    let safe = task_id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if !safe || task_id.contains("..") {
        return Err(ServiceError::rejected(
            request,
            format!("the task id `{task_id}` is not a plain identifier"),
        ));
    }
    Ok(format!("/tasks/{task_id}/{action}"))
}

/// The real client: HTTP against one swarm API.
#[derive(Debug, Clone)]
pub struct HttpSwarmClient {
    http: reqwest::Client,
    base_url: String,
    timeout: Duration,
}

impl HttpSwarmClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into(),
            timeout: SWARM_API_TIMEOUT,
        }
    }

    pub fn from_env() -> Self {
        Self::new(
            std::env::var(SWARM_API_URL_ENV).unwrap_or_else(|_| DEFAULT_SWARM_API_URL.to_string()),
        )
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    async fn send(&self, method: reqwest::Method, path: &str) -> Result<Value, ServiceError> {
        let url = self.url(path);
        let response = self
            .http
            .request(method, &url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    ServiceError::timeout(path, &url)
                } else {
                    ServiceError::unreachable(path, &url, error.to_string())
                }
            })?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(|error| {
            ServiceError::unreachable(
                path,
                &url,
                format!("the answer's body could not be read: {error}"),
            )
        })?;
        decode_swarm_response(path, &url, status, &body)
    }
}

impl SwarmClient for HttpSwarmClient {
    fn get<'a>(&'a self, path: &'a str) -> SwarmFuture<'a, Value> {
        Box::pin(async move { self.send(reqwest::Method::GET, path).await })
    }

    fn post<'a>(&'a self, path: &'a str) -> SwarmFuture<'a, Value> {
        Box::pin(async move { self.send(reqwest::Method::POST, path).await })
    }
}

/// Decode one swarm API answer. Pure, so the service's real bodies and its real
/// failure codes (`404` for an unknown task, `423` when the kill switch holds a
/// task) can be pinned without booting the service.
pub fn decode_swarm_response(
    request: &str,
    endpoint: &str,
    status: u16,
    body: &str,
) -> Result<Value, ServiceError> {
    if !(200..300).contains(&status) {
        // The service refuses a request with a bare status (`404` for an unknown
        // task, `423` when the kill switch holds it). Keeping the status is what
        // lets the operator tell "no such task" from "the swarm is halted".
        return Err(ServiceError::http_status(request, endpoint, status));
    }
    serde_json::from_str(body)
        .map_err(|error| ServiceError::malformed(request, format!("not a JSON body: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ServiceErrorKind;

    #[test]
    fn the_approve_path_is_the_route_the_service_serves() {
        assert_eq!(
            task_action_path("approve", "x3-task-0001").unwrap(),
            "/tasks/x3-task-0001/approve"
        );
        assert_eq!(
            task_action_path("reject", "x3-task-0009").unwrap(),
            "/tasks/x3-task-0009/reject"
        );
    }

    #[test]
    fn a_task_id_that_would_rewrite_the_path_is_refused() {
        for hostile in [
            "",
            "..",
            "../tasks",
            "x3-task/../..",
            "x3 task",
            "a?b=c",
            "a#b",
            "a\nb",
        ] {
            let error = task_action_path("approve", hostile)
                .expect_err(&format!("`{hostile}` must not become a URL path"));
            assert!(matches!(error.kind, ServiceErrorKind::RejectedInput(_)));
        }
    }

    #[test]
    fn the_services_health_body_parses_into_its_own_field_names() {
        // The exact body `services/x3-swarm-api::health` writes.
        let body = r#"{"service":"x3-swarm-api","status":"ok","mode":"GUARDED_TESTNET","agents_enabled":true,"kill_switch":false}"#;
        let value =
            decode_swarm_response("GET /health", "http://127.0.0.1:8787", 200, body).unwrap();
        let health: SwarmHealth = serde_json::from_value(value).unwrap();
        assert_eq!(health.service, "x3-swarm-api");
        assert_eq!(health.mode, "GUARDED_TESTNET");
        assert!(health.agents_enabled);
        assert!(!health.kill_switch);
    }

    #[test]
    fn the_health_body_the_panel_used_to_expect_would_not_parse() {
        // Guards the regression: the console hand-rolled a different shape once.
        let wrong = r#"{"node":"x3","peers":3}"#;
        let value =
            decode_swarm_response("GET /health", "http://127.0.0.1:8787", 200, wrong).unwrap();
        assert!(serde_json::from_value::<SwarmHealth>(value).is_err());
    }

    #[test]
    fn a_task_list_parses_the_field_names_the_panel_reads() {
        // Taken verbatim from a real `/tasks` answer (`reports/swarm_task_queue.json`).
        let body = r#"[{"id":"x3-task-0001","title":"Audit core runtime path guard","feature":"swarm-forbidden-path","agent":"swarm-guard","permission_tier":"constrained","allowed_paths":["crates/x3-swarm-core/src"],"forbidden_paths":["./.git"],"required_commands":["cargo test -p x3-swarm-core -- --nocapture"],"status":"Passed","approval_required":"manual","risk":"medium"}]"#;
        let value =
            decode_swarm_response("GET /tasks", "http://127.0.0.1:8787", 200, body).unwrap();
        let tasks: Vec<SwarmTask> = serde_json::from_value(value).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "x3-task-0001");
        assert_eq!(tasks[0].permission_tier, "constrained");
        assert_eq!(tasks[0].approval_required, "manual");
        assert_eq!(tasks[0].risk, "medium");
    }

    #[test]
    fn the_task_shape_the_console_used_to_expect_is_rejected() {
        // `{"name":..,"priority":..}` was the old struct; the service never sent it.
        let wrong = r#"[{"id":"x3-task-0001","name":"n","status":"Pending","agent":"a","priority":1,"created_at":"now"}]"#;
        let value =
            decode_swarm_response("GET /tasks", "http://127.0.0.1:8787", 200, wrong).unwrap();
        let parsed: Result<Vec<SwarmTask>, _> = serde_json::from_value(value);
        assert!(
            parsed.is_err(),
            "the old field set must not be silently accepted"
        );
    }

    #[test]
    fn an_unknown_task_and_a_held_task_are_reported_with_their_status() {
        let not_found =
            decode_swarm_response("POST /tasks/{id}/approve", "http://127.0.0.1:8787", 404, "")
                .unwrap_err();
        assert_eq!(not_found.kind, ServiceErrorKind::HttpStatus(404));
        let held =
            decode_swarm_response("POST /tasks/{id}/approve", "http://127.0.0.1:8787", 423, "")
                .unwrap_err();
        assert_eq!(held.kind, ServiceErrorKind::HttpStatus(423));
    }
}
