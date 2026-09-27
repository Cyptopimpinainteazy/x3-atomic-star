//! The operator console, exercised from outside the crate.
//!
//! Two of these tests talk to a server that speaks the real protocols — a JSON-RPC
//! 2.0 method dispatcher for the node, and the swarm API's own routes — so the
//! wire path is tested, not just the parsers. One talks to a node that is not
//! there, and asserts on the typed error rather than on a string. The last one
//! needs a real `x3-chain-node`, so it is behind the `live-node` feature and is
//! built only by `apps/tauri-os/src-tauri/run-live-test.sh`. A feature gate
//! rather than a skip attribute: a skipped test can be skipped silently, while
//! this one either compiles and must pass, or does not exist.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri_os_backend::chain::RpcChainClient;
use tauri_os_backend::commands::{node_status, swarm_approve, swarm_tasks, validator_health};
use tauri_os_backend::error::{IPC_SERVICE_REJECTED, IPC_SERVICE_UNREACHABLE};
use tauri_os_backend::swarm::HttpSwarmClient;

#[cfg(feature = "live-node")]
use tauri_os_backend::chain::DEFAULT_NODE_RPC_URL;
use tauri_os_backend::models::ValidatorStatus;

/// A local port nothing is listening on: bound to learn a free number, then
/// released.
fn closed_port() -> u16 {
    let probe = TcpListener::bind("127.0.0.1:0").expect("bind a free port");
    let port = probe.local_addr().expect("read the bound address").port();
    drop(probe);
    port
}

/// Serve every request with `handler(path, body) -> (status, body)`.
///
/// Answers carry `Connection: close`, so each request arrives on its own
/// connection and the server needs no keep-alive state. The listener and the
/// per-connection threads are left running: a test binary has no shutdown hook
/// and the threads are asleep in `accept`, which costs nothing.
fn spawn_double(
    handler: impl Fn(&str, &str) -> (u16, String) + Send + Sync + 'static,
) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a test listener");
    let address = listener.local_addr().expect("read the bound address");
    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let seen_by_server = Arc::clone(&seen);
    let handler = Arc::new(handler);

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            let handler = Arc::clone(&handler);
            let seen = Arc::clone(&seen_by_server);
            thread::spawn(move || serve_one(stream, &*handler, &seen));
        }
    });

    (format!("http://{address}"), seen)
}

fn serve_one(
    mut stream: TcpStream,
    handler: &(dyn Fn(&str, &str) -> (u16, String) + Send + Sync),
    seen: &Arc<Mutex<Vec<String>>>,
) {
    let Some((path, body)) = read_request(&mut stream) else {
        return;
    };
    seen.lock().expect("request log").push(path.clone());

    let (status, response) = handler(&path, &body);
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        423 => "Locked",
        502 => "Bad Gateway",
        _ => "Status",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn read_request(stream: &mut TcpStream) -> Option<(String, String)> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).ok()? == 0 {
        return None;
    }
    let mut parts = request_line.split_whitespace();
    parts.next()?; // method
    let path = parts.next()?.to_owned();

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            break;
        }
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        let lower = header.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("content-length:") {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }

    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body).ok()?;
    Some((path, String::from_utf8_lossy(&body).into_owned()))
}

fn json_rpc_result(id: &Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

/// A local JSON-RPC server that answers the six methods the console reads, with the
/// values a real node would send — including the padded hex quantity in a
/// header.
fn spawn_node_double() -> String {
    let (base, _seen) = spawn_double(|path, body| {
        assert_eq!(path, "/", "the node client posts to the endpoint root");
        let request: Value = serde_json::from_str(body).expect("the client sends JSON");
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let response = match method {
            "chain_getFinalizedHead" => json_rpc_result(&id, json!("0xabc123")),
            "chain_getHeader" => {
                json_rpc_result(&id, json!({ "number": "0x2a", "parentHash": "0xdef" }))
            }
            "system_health" => json_rpc_result(
                &id,
                json!({ "peers": 4, "isSyncing": false, "shouldHavePeers": true }),
            ),
            "system_name" => json_rpc_result(&id, json!("x3-chain-node")),
            "system_version" => json_rpc_result(&id, json!("0.1.0-test")),
            "system_chain" => json_rpc_result(&id, json!("X3 Development")),
            "system_nodeRoles" => json_rpc_result(&id, json!(["authority"])),
            other => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {other}") }
            })
            .to_string(),
        };
        (200, response)
    });
    base
}

#[tokio::test]
async fn a_node_that_is_not_there_is_a_typed_error_not_an_empty_status() {
    let client = RpcChainClient::new(format!("http://127.0.0.1:{}", closed_port()))
        .with_timeout(Duration::from_millis(750));

    let error = node_status(&client)
        .await
        .expect_err("nothing is listening");
    let ipc = tauri_os_backend::error::IpcError::from(error);
    assert_eq!(ipc.code, IPC_SERVICE_UNREACHABLE);
    assert!(ipc.details.unwrap_or_default().contains("system_health"));
}

#[tokio::test]
async fn the_console_reads_the_chain_off_a_real_jsonrpc_conversation() {
    let client = RpcChainClient::new(spawn_node_double());

    let status = node_status(&client).await.expect("every read is answered");
    assert!(status.running);
    assert_eq!(status.finalized.hash, "0xabc123");
    assert_eq!(
        status.finalized.number, 42,
        "0x2a comes from the finalized block's own header"
    );
    assert_eq!(status.peers, 4);
    assert!(!status.is_syncing);
    assert_eq!(status.name, "x3-chain-node");
    assert_eq!(status.version, "0.1.0-test");
    assert_eq!(status.chain, "X3 Development");
    assert_eq!(status.role.as_deref(), Some("authority"));

    let health = validator_health(&client)
        .await
        .expect("every read is answered");
    assert_eq!(health.status, ValidatorStatus::Healthy);
    assert_eq!(health.finalized.number, 42);
    assert!(health.notes.is_empty(), "{:?}", health.notes);
}

#[tokio::test]
async fn a_node_that_refuses_the_role_query_does_not_get_a_role_invented() {
    let (base, _seen) = spawn_double(|_path, body| {
        let request: Value = serde_json::from_str(body).expect("json");
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match method {
            "system_nodeRoles" => (
                200,
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": "Method not found" }
                })
                .to_string(),
            ),
            "chain_getFinalizedHead" => (200, json_rpc_result(&id, json!("0xabc123"))),
            "chain_getHeader" => (200, json_rpc_result(&id, json!({ "number": "0x1" }))),
            "system_health" => (
                200,
                json_rpc_result(
                    &id,
                    json!({ "peers": 0, "isSyncing": false, "shouldHavePeers": false }),
                ),
            ),
            "system_name" => (200, json_rpc_result(&id, json!("x3-chain-node"))),
            "system_version" => (200, json_rpc_result(&id, json!("0.1.0-test"))),
            "system_chain" => (200, json_rpc_result(&id, json!("X3 Development"))),
            _ => (200, json_rpc_result(&id, Value::Null)),
        }
    });

    let client = RpcChainClient::new(base);
    let status = node_status(&client)
        .await
        .expect("every other read answers");
    assert_eq!(
        status.role, None,
        "an unserved method means no role, not a default one"
    );
}

#[tokio::test]
async fn a_swarm_api_that_refuses_is_a_typed_error_not_a_stale_task_list() {
    let (base, _seen) = spawn_double(|_path, _body| (502, "<html>bad gateway</html>".to_owned()));
    let client = HttpSwarmClient::new(base);
    let error = swarm_tasks(&client).await.expect_err("the server refuses");
    let ipc = tauri_os_backend::error::IpcError::from(error);
    assert_eq!(ipc.code, IPC_SERVICE_REJECTED);
}

#[tokio::test]
async fn the_console_calls_the_routes_the_swarm_service_actually_serves() {
    let task_body = json!({
        "id": "x3-task-0001",
        "title": "Audit core runtime path guard",
        "feature": "swarm-forbidden-path",
        "agent": "swarm-guard",
        "permission_tier": "constrained",
        "allowed_paths": ["crates/x3-swarm-core/src"],
        "forbidden_paths": ["./.git"],
        "required_commands": [],
        "status": "Pending",
        "approval_required": "manual",
        "risk": "medium"
    })
    .to_string();

    let (base, seen) = spawn_double(move |path, _body| match path {
        "/tasks" => (200, format!("[{task_body}]")),
        "/tasks/x3-task-0001/approve" | "/tasks/x3-task-0001/reject" => (200, task_body.clone()),
        _ => (404, String::new()),
    });

    let client = HttpSwarmClient::new(base);
    let tasks = swarm_tasks(&client)
        .await
        .expect("the server serves /tasks");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, "x3-task-0001");
    assert_eq!(tasks[0].permission_tier, "constrained");
    assert_eq!(tasks[0].approval_required, "manual");

    let approved = swarm_approve(&client, "x3-task-0001")
        .await
        .expect("the server serves /tasks/{id}/approve");
    assert_eq!(approved.id, "x3-task-0001");

    let paths = seen.lock().expect("request log").clone();
    assert!(paths.contains(&"/tasks".to_owned()), "{paths:?}");
    assert!(
        paths.contains(&"/tasks/x3-task-0001/approve".to_owned()),
        "the console must use the service's route, not `/approve/{{id}}`: {paths:?}"
    );
}

#[tokio::test]
async fn an_unknown_task_is_reported_with_the_status_the_service_returned() {
    let (base, _seen) = spawn_double(|_path, _body| (404, String::new()));
    let client = HttpSwarmClient::new(base);
    let error = swarm_approve(&client, "x3-task-0001")
        .await
        .expect_err("404");
    let ipc = tauri_os_backend::error::IpcError::from(error);
    assert_eq!(ipc.code, IPC_SERVICE_REJECTED);
    assert!(ipc.details.unwrap_or_default().contains("404"));
}

/// The live half: it needs a node, and a suite that cannot start one must not
/// report it green. Built only with `--features live-node`, which
/// `run-live-test.sh` passes after it has booted a chain.
#[cfg(feature = "live-node")]
#[tokio::test]
async fn live_operator_console_reads_a_running_node() {
    let url = std::env::var("X3_OS_RPC_URL").unwrap_or_else(|_| DEFAULT_NODE_RPC_URL.to_owned());
    let client = RpcChainClient::new(url.clone());

    let status = node_status(&client)
        .await
        .unwrap_or_else(|error| panic!("no node answered at {url}: {error}"));
    assert!(status.running);
    assert!(!status.name.is_empty(), "the node must name itself");
    assert!(!status.chain.is_empty(), "the node must name its chain");
    assert!(
        status.finalized.number >= 1,
        "finality must have advanced past genesis: {status:?}"
    );
    assert!(
        status.finalized.hash.starts_with("0x"),
        "the finalized head must be a block hash: {status:?}"
    );

    let health = validator_health(&client).await.expect("the node answers");
    assert_eq!(health.finalized.number, status.finalized.number);
    assert_ne!(
        health.status,
        ValidatorStatus::Stalled,
        "a node past genesis is not stalled: {health:?}"
    );

    println!(
        "live console read: {} {} chain={} finalized={} ({}) peers={} syncing={} role={:?} verdict={:?}",
        status.name,
        status.version,
        status.chain,
        status.finalized.number,
        status.finalized.hash,
        status.peers,
        status.is_syncing,
        status.role,
        health.status
    );
}
