use axum::{extract::{Path, State}, response::IntoResponse, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentRecord {
    id: String,
    kind: String,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskRecord {
    id: String,
    title: String,
    feature: String,
    agent: String,
    permission_tier: String,
    allowed_paths: Vec<String>,
    forbidden_paths: Vec<String>,
    required_commands: Vec<String>,
    status: String,
    approval_required: String,
    risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryRecord {
    id: String,
    agent: String,
    feature: String,
    finding: String,
    severity: String,
    test_added: Option<String>,
    fix_commit: Option<String>,
    result: String,
    timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EventRecord {
    id: String,
    event_type: String,
    message: String,
    timestamp: String,
}

type AppState = Arc<Mutex<StateData>>;

#[derive(Default)]
struct StateData {
    agents: HashMap<String, AgentRecord>,
    tasks: HashMap<String, TaskRecord>,
    memory: Vec<MemoryRecord>,
    events: Vec<EventRecord>,
    kill_switch: bool,
}

#[derive(Debug, Deserialize)]
struct NewTask {
    title: String,
    feature: String,
    agent: String,
    permission_tier: String,
    allowed_paths: Option<Vec<String>>,
    forbidden_paths: Option<Vec<String>>,
    required_commands: Option<Vec<String>>,
    approval_required: String,
    risk: String,
}

#[derive(Debug, Deserialize)]
struct MemoryPayload {
    id: String,
    agent: String,
    feature: String,
    finding: String,
    severity: String,
    test_added: Option<String>,
    fix_commit: Option<String>,
    result: String,
}

#[derive(Debug, Deserialize)]
struct EventPayload {
    id: String,
    event_type: String,
    message: String,
}

#[tokio::main]
async fn main() {
    let state: AppState = Arc::new(Mutex::new(StateData::default()));

    let app = Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/agents", get(list_agents))
        .route("/tasks", get(list_tasks).post(create_task))
        .route("/tasks/:id", get(get_task))
        .route("/tasks/:id/start", post(start_task))
        .route("/tasks/:id/complete", post(complete_task))
        .route("/tasks/:id/fail", post(fail_task))
        .route("/tasks/:id/approve", post(approve_task))
        .route("/tasks/:id/reject", post(reject_task))
        .route("/scoreboard", get(scoreboard))
        .route("/memory", get(list_memory).post(create_memory))
        .route("/events", get(list_events).post(create_event))
        .route("/kill-switch", post(kill_switch))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8787));
    println!("Starting x3-swarm-api on http://{}", addr);
    axum_server::bind(addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "service": "x3-swarm-api",
        "status": "ok",
        "mode": "GUARDED_TESTNET",
        "agents_enabled": true,
        "kill_switch": false,
    }))
}

async fn status(state: State<AppState>) -> impl IntoResponse {
    let state = state.lock().await;
    Json(serde_json::json!({
        "service": "x3-swarm-api",
        "status": "ok",
        "tasks": state.tasks.len(),
        "agents": state.agents.len(),
        "kill_switch": state.kill_switch,
    }))
}

async fn list_agents(state: State<AppState>) -> impl IntoResponse {
    let state = state.lock().await;
    let agents: Vec<_> = state.agents.values().cloned().collect();
    Json(agents)
}

async fn list_tasks(state: State<AppState>) -> impl IntoResponse {
    let state = state.lock().await;
    let tasks: Vec<_> = state.tasks.values().cloned().collect();
    Json(tasks)
}

#[axum::debug_handler]
async fn create_task(state: State<AppState>, Json(payload): Json<NewTask>) -> impl IntoResponse {
    let mut state = state.lock().await;
    if let Some(existing) = state.tasks.values().find(|task| {
        task.title == payload.title && task.feature == payload.feature && task.agent == payload.agent
    }) {
        return Json(existing.clone());
    }

    let task = TaskRecord {
        id: format!("x3-task-{:04}", state.tasks.len() + 1),
        title: payload.title,
        feature: payload.feature,
        agent: payload.agent,
        permission_tier: payload.permission_tier,
        allowed_paths: payload.allowed_paths.unwrap_or_default(),
        forbidden_paths: payload.forbidden_paths.unwrap_or_default(),
        required_commands: payload.required_commands.unwrap_or_default(),
        status: "Pending".into(),
        approval_required: payload.approval_required,
        risk: payload.risk,
    };
    state.tasks.insert(task.id.clone(), task.clone());
    Json(task)
}

async fn get_task(Path(id): Path<String>, state: State<AppState>) -> Result<Json<TaskRecord>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let state = state.lock().await;
    if let Some(task) = state.tasks.get(&id) {
        Ok(Json(task.clone()))
    } else {
        Err((axum::http::StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not found"}))))
    }
}

async fn start_task(Path(id): Path<String>, state: State<AppState>) -> Result<Json<TaskRecord>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let mut state = state.lock().await;
    if let Some(task) = state.tasks.get_mut(&id) {
        task.status = "Running".into();
        Ok(Json(task.clone()))
    } else {
        Err((axum::http::StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not found"}))))
    }
}

async fn complete_task(Path(id): Path<String>, state: State<AppState>) -> Result<Json<TaskRecord>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let mut state = state.lock().await;
    if let Some(task) = state.tasks.get_mut(&id) {
        task.status = "Passed".into();
        Ok(Json(task.clone()))
    } else {
        Err((axum::http::StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not found"}))))
    }
}

async fn fail_task(Path(id): Path<String>, state: State<AppState>) -> Result<Json<TaskRecord>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let mut state = state.lock().await;
    if let Some(task) = state.tasks.get_mut(&id) {
        task.status = "Failed".into();
        Ok(Json(task.clone()))
    } else {
        Err((axum::http::StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not found"}))))
    }
}

async fn approve_task(Path(id): Path<String>, state: State<AppState>) -> Result<Json<TaskRecord>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let mut state = state.lock().await;
    if let Some(task) = state.tasks.get_mut(&id) {
        task.status = "Pending".into();
        Ok(Json(task.clone()))
    } else {
        Err((axum::http::StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not found"}))))
    }
}

async fn reject_task(Path(id): Path<String>, state: State<AppState>) -> Result<Json<TaskRecord>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let mut state = state.lock().await;
    if let Some(task) = state.tasks.get_mut(&id) {
        task.status = "Blocked".into();
        Ok(Json(task.clone()))
    } else {
        Err((axum::http::StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not found"}))))
    }
}

async fn scoreboard(state: State<AppState>) -> impl IntoResponse {
    let state = state.lock().await;
    Json(serde_json::json!({
        "service": "x3-swarm-api",
        "total_tasks": state.tasks.len(),
        "success_rate": 0.0,
    }))
}

async fn list_memory(state: State<AppState>) -> impl IntoResponse {
    let state = state.lock().await;
    Json(state.memory.clone())
}

#[axum::debug_handler]
async fn create_memory(state: State<AppState>, Json(payload): Json<MemoryPayload>) -> impl IntoResponse {
    let mut state = state.lock().await;
    let entry = MemoryRecord {
        id: payload.id,
        agent: payload.agent,
        feature: payload.feature,
        finding: payload.finding,
        severity: payload.severity,
        test_added: payload.test_added,
        fix_commit: payload.fix_commit,
        result: payload.result,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    state.memory.push(entry.clone());
    Json(entry)
}

async fn list_events(state: State<AppState>) -> impl IntoResponse {
    let state = state.lock().await;
    Json(state.events.clone())
}

#[axum::debug_handler]
async fn create_event(state: State<AppState>, Json(payload): Json<EventPayload>) -> impl IntoResponse {
    let mut state = state.lock().await;
    let event = EventRecord {
        id: payload.id,
        event_type: payload.event_type,
        message: payload.message,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    state.events.push(event.clone());
    Json(event)
}

async fn kill_switch(state: State<AppState>) -> impl IntoResponse {
    let mut state = state.lock().await;
    state.kill_switch = true;
    Json(serde_json::json!({"status": "kill switch engaged"}))
}
