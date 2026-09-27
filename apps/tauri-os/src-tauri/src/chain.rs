//! The console's read path into a running X3 node.
//!
//! Everything the operator console shows about the chain comes through
//! [`ChainClient`]. The trait exists so the command layer can be exercised
//! against a client that answers, a client that refuses, and the real node —
//! and so no command can quietly reach for a global.
//!
//! The methods are the ones the repository's own gates already use against a
//! live node (`scripts/local-node-smoke.sh`, `scripts/mainnet/local3_lib.sh`,
//! `scripts/mainnet/public_testnet_gate.sh`): `chain_getFinalizedHead`,
//! `chain_getHeader`, `system_health`, `system_name`, `system_version`,
//! `system_chain`. The node serves them from
//! `substrate_frame_rpc_system::SystemApiServer` (see `node/src/rpc.rs`), so the
//! console reads the same surface the operator CLI does instead of inventing a
//! second one.

use crate::error::ServiceError;
use crate::models::{FinalizedHead, NodeIdentity, SystemHealth};
use serde_json::{json, Value};
use std::error::Error as _;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

/// Where a node listens unless the operator says otherwise. `9944` is both the
/// node's own default (`node/src/cli.rs`) and what `X3_NODE_RPC_URL` falls back
/// to (`node/src/service.rs`).
pub const DEFAULT_NODE_RPC_URL: &str = "http://127.0.0.1:9944";

/// Environment override for [`DEFAULT_NODE_RPC_URL`].
pub const NODE_RPC_URL_ENV: &str = "X3_OS_RPC_URL";

/// How long any single node read may take. An operator console that waits
/// forever on a wedged node is indistinguishable from a hung app.
pub const NODE_RPC_TIMEOUT: Duration = Duration::from_secs(5);

/// Optional: not every build of the node registers a role query, and "this node
/// does not report its role" is a fact worth showing.
const METHOD_NODE_ROLES: &str = "system_nodeRoles";

/// A boxed future, so [`ChainClient`] stays object-safe and the command layer
/// can hold one `Arc<dyn ChainClient>`.
pub type ChainFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ServiceError>> + Send + 'a>>;

/// The node reads the console needs. Implementations must not invent answers:
/// a failed read is an `Err`, never a default.
pub trait ChainClient: Send + Sync {
    /// Perform a JSON-RPC 2.0 call and return its `result`, or the typed reason
    /// it could not be read.
    fn rpc_call<'a>(&'a self, method: &'a str, params: Value) -> ChainFuture<'a, Value>;

    /// The finalized head: the hash the node finalized *and* that block's own
    /// number. Reading only the best header would let a node that is nowhere
    /// near finality look final.
    fn finalized_head(&self) -> ChainFuture<'_, FinalizedHead> {
        Box::pin(async move {
            let hash = self.rpc_call("chain_getFinalizedHead", json!([])).await?;
            let hash = hash
                .as_str()
                .ok_or_else(|| ServiceError::missing("chain_getFinalizedHead", "result"))?
                .to_string();
            let header = self.rpc_call("chain_getHeader", json!([hash])).await?;
            let number = parse_block_number("chain_getHeader", &header)?;
            Ok(FinalizedHead { hash, number })
        })
    }

    /// `system_health`, as the node reports it.
    fn system_health(&self) -> ChainFuture<'_, SystemHealth> {
        Box::pin(async move {
            let value = self.rpc_call("system_health", json!([])).await?;
            parse_system_health(&value)
        })
    }

    /// The peer count from `system_health`. Kept as its own method because it is
    /// the one number an operator watches while the node catches up.
    fn peers(&self) -> ChainFuture<'_, u64> {
        Box::pin(async move { Ok(self.system_health().await?.peers) })
    }

    /// Name, version and chain identity.
    fn node_identity(&self) -> ChainFuture<'_, NodeIdentity> {
        Box::pin(async move {
            let name = self.rpc_call("system_name", json!([])).await?;
            let version = self.rpc_call("system_version", json!([])).await?;
            let chain = self.rpc_call("system_chain", json!([])).await?;
            Ok(NodeIdentity {
                name: parse_string("system_name", &name)?,
                version: parse_string("system_version", &version)?,
                chain: parse_string("system_chain", &chain)?,
            })
        })
    }

    /// The node's role, or `None` when it does not serve `system_nodeRoles`.
    /// Any other failure is propagated: a node that answers this method with an
    /// error is unhealthy in a way we must not paper over.
    fn node_role(&self) -> ChainFuture<'_, Option<String>> {
        Box::pin(async move {
            match self.rpc_call(METHOD_NODE_ROLES, json!([])).await {
                Ok(value) => parse_node_roles(&value),
                Err(error) if error.is_method_not_found() => Ok(None),
                Err(error) => Err(error),
            }
        })
    }
}

/// The real client: JSON-RPC 2.0 over HTTP against one node.
#[derive(Debug, Clone)]
pub struct RpcChainClient {
    http: reqwest::Client,
    url: String,
    timeout: Duration,
}

impl RpcChainClient {
    /// Point the console at a node. The URL is not parsed here — an unusable
    /// endpoint is reported by the call that needs it, as a typed error the
    /// operator can read, rather than by refusing to start the app.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            url: url.into(),
            timeout: NODE_RPC_TIMEOUT,
        }
    }

    /// The node named by `X3_OS_RPC_URL`, or the local default.
    pub fn from_env() -> Self {
        Self::new(
            std::env::var(NODE_RPC_URL_ENV).unwrap_or_else(|_| DEFAULT_NODE_RPC_URL.to_string()),
        )
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

impl ChainClient for RpcChainClient {
    fn rpc_call<'a>(&'a self, method: &'a str, params: Value) -> ChainFuture<'a, Value> {
        Box::pin(async move {
            let body = json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": params,
            });
            let response = self
                .http
                .post(&self.url)
                .json(&body)
                .timeout(self.timeout)
                .send()
                .await
                .map_err(|error| classify_transport_error(method, &self.url, &error))?;
            let status = response.status().as_u16();
            let body = response.text().await.map_err(|error| {
                ServiceError::unreachable(
                    method,
                    &self.url,
                    format!(
                        "the answer's body could not be read: {}",
                        transport_detail(&error)
                    ),
                )
            })?;
            decode_rpc_response(method, &self.url, status, &body)
        })
    }
}

/// Turn a transport failure into the one of two things it can mean.
fn classify_transport_error(request: &str, endpoint: &str, error: &reqwest::Error) -> ServiceError {
    if error.is_timeout() {
        ServiceError::timeout(request, endpoint)
    } else {
        ServiceError::unreachable(request, endpoint, transport_detail(error))
    }
}

/// `reqwest`'s own sentence plus its cause chain — "connection refused" lives in
/// the cause, not in the top-level message.
fn transport_detail(error: &reqwest::Error) -> String {
    let mut detail = error.to_string();
    let mut cause = error.source();
    while let Some(inner) = cause {
        detail.push_str(": ");
        detail.push_str(&inner.to_string());
        cause = inner.source();
    }
    detail
}

/// Decode one JSON-RPC answer. Pure, so the error shapes a real node produces
/// (jsonrpsee's error objects, an HTTP gateway in front of the node, a body
/// that is not JSON at all) can be pinned by tests without a node.
pub fn decode_rpc_response(
    method: &str,
    endpoint: &str,
    status: u16,
    body: &str,
) -> Result<Value, ServiceError> {
    if !(200..300).contains(&status) {
        return Err(ServiceError::http_status(method, endpoint, status));
    }
    let value: Value = serde_json::from_str(body).map_err(|error| {
        ServiceError::malformed(method, format!("not a JSON-RPC answer: {error}"))
    })?;
    if let Some(error) = value.get("error") {
        let code = error.get("code").and_then(Value::as_i64);
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("the service sent an error object with no message");
        return Err(ServiceError::rpc(method, endpoint, code, message));
    }
    value
        .get("result")
        .cloned()
        .ok_or_else(|| ServiceError::missing(method, "result"))
}

/// The `number` field of a header, which Substrate encodes as a hex quantity.
pub fn parse_block_number(method: &str, header: &Value) -> Result<u64, ServiceError> {
    let raw = header
        .get("number")
        .and_then(Value::as_str)
        .ok_or_else(|| ServiceError::missing(method, "number"))?;
    decode_quantity(raw).ok_or_else(|| {
        ServiceError::malformed(method, format!("`number` is not a hex quantity: {raw}"))
    })
}

/// `0x…` → `u64`, checked. Substrate (unlike Ethereum) may pad the quantity, so
/// leading zeroes are accepted; anything that overflows or is not hex is not.
pub fn decode_quantity(raw: &str) -> Option<u64> {
    let digits = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X"))?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(digits, 16).ok()
}

/// `system_health` → [`SystemHealth`].
pub fn parse_system_health(value: &Value) -> Result<SystemHealth, ServiceError> {
    let peers = value
        .get("peers")
        .and_then(Value::as_u64)
        .ok_or_else(|| ServiceError::missing("system_health", "peers"))?;
    let is_syncing = value
        .get("isSyncing")
        .and_then(Value::as_bool)
        .ok_or_else(|| ServiceError::missing("system_health", "isSyncing"))?;
    // `shouldHavePeers` is present on every substrate system_health, but a node
    // that omits it has told us nothing about whether it expects peers; default
    // to `true` so an isolated node is still reported as isolated.
    let should_have_peers = value
        .get("shouldHavePeers")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    Ok(SystemHealth {
        peers,
        is_syncing,
        should_have_peers,
    })
}

/// `system_nodeRoles` → the roles the node reports, comma-joined when there is
/// more than one. `None` when the node reports none.
pub fn parse_node_roles(value: &Value) -> Result<Option<String>, ServiceError> {
    let roles = value
        .as_array()
        .ok_or_else(|| ServiceError::missing(METHOD_NODE_ROLES, "a role array"))?;
    let mut names = Vec::with_capacity(roles.len());
    for role in roles {
        let name = role
            .as_str()
            .ok_or_else(|| ServiceError::missing(METHOD_NODE_ROLES, "a role name"))?;
        names.push(name.to_owned());
    }
    if names.is_empty() {
        return Ok(None);
    }
    Ok(Some(names.join(", ")))
}

/// A JSON-RPC `result` that must be a plain string.
pub fn parse_string(method: &str, value: &Value) -> Result<String, ServiceError> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| ServiceError::missing(method, "a string result"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{jsonrpc_error, ServiceErrorKind};
    use serde_json::json;

    #[test]
    fn a_padded_header_number_decodes_and_garbage_does_not() {
        assert_eq!(decode_quantity("0x0"), Some(0));
        assert_eq!(decode_quantity("0x1f4"), Some(500));
        assert_eq!(decode_quantity("0x0001"), Some(1));
        assert_eq!(decode_quantity("1f4"), None);
        assert_eq!(decode_quantity("0x"), None);
        assert_eq!(decode_quantity("0xzz"), None);
        // u64::MAX + 1 — a header number we cannot represent is not a number.
        assert_eq!(decode_quantity("0x10000000000000000"), None);
    }

    #[test]
    fn a_header_without_a_number_is_reported_as_missing_not_as_genesis() {
        let error =
            parse_block_number("chain_getHeader", &json!({ "parentHash": "0x00" })).unwrap_err();
        assert_eq!(error.kind, ServiceErrorKind::MissingField("number"));
    }

    #[test]
    fn system_health_parses_the_shape_the_node_serves() {
        // Captured from a `--dev` node: a single node with `shouldHavePeers:false`.
        let health = parse_system_health(&json!({
            "peers": 0,
            "isSyncing": false,
            "shouldHavePeers": false
        }))
        .unwrap();
        assert_eq!(
            health,
            SystemHealth {
                peers: 0,
                is_syncing: false,
                should_have_peers: false
            }
        );
    }

    #[test]
    fn system_health_without_peers_or_syncing_is_rejected() {
        assert_eq!(
            parse_system_health(&json!({ "isSyncing": false }))
                .unwrap_err()
                .kind,
            ServiceErrorKind::MissingField("peers")
        );
        assert_eq!(
            parse_system_health(&json!({ "peers": 1 }))
                .unwrap_err()
                .kind,
            ServiceErrorKind::MissingField("isSyncing")
        );
    }

    #[test]
    fn a_health_answer_that_omits_should_have_peers_still_expects_peers() {
        assert!(
            parse_system_health(&json!({ "peers": 0, "isSyncing": false }))
                .unwrap()
                .should_have_peers
        );
    }

    #[test]
    fn node_roles_are_reported_verbatim() {
        assert_eq!(
            parse_node_roles(&json!(["authority"])).unwrap(),
            Some("authority".to_owned())
        );
        assert_eq!(
            parse_node_roles(&json!(["full", "authority"])).unwrap(),
            Some("full, authority".to_owned())
        );
        assert_eq!(parse_node_roles(&json!([])).unwrap(), None);
        assert!(parse_node_roles(&json!("authority")).is_err());
    }

    #[test]
    fn a_jsonrpc_error_object_becomes_a_typed_error_with_its_code() {
        // The body jsonrpsee writes for an unregistered method.
        let body =
            r#"{"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found"},"id":1}"#;
        let error = decode_rpc_response("system_nodeRoles", "http://127.0.0.1:9944", 200, body)
            .unwrap_err();
        assert_eq!(
            error.kind,
            ServiceErrorKind::RpcError {
                code: Some(jsonrpc_error::METHOD_NOT_FOUND),
                message: "Method not found".to_owned()
            }
        );
        assert!(error.is_method_not_found());
    }

    #[test]
    fn an_error_object_without_a_code_is_not_given_the_method_not_found_code() {
        let body = r#"{"jsonrpc":"2.0","error":{"message":"boom"},"id":1}"#;
        let error =
            decode_rpc_response("system_health", "http://127.0.0.1:9944", 200, body).unwrap_err();
        assert!(!error.is_method_not_found());
        assert_eq!(
            error.kind,
            ServiceErrorKind::RpcError {
                code: None,
                message: "boom".to_owned()
            }
        );
    }

    #[test]
    fn a_non_2xx_answer_is_reported_with_its_status() {
        let error = decode_rpc_response(
            "system_health",
            "http://127.0.0.1:9944",
            502,
            "<html>bad gateway</html>",
        )
        .unwrap_err();
        assert_eq!(error.kind, ServiceErrorKind::HttpStatus(502));
    }

    #[test]
    fn an_html_body_is_not_read_as_an_empty_result() {
        let error = decode_rpc_response(
            "system_health",
            "http://127.0.0.1:9944",
            200,
            "<html>hi</html>",
        )
        .unwrap_err();
        assert!(matches!(error.kind, ServiceErrorKind::MalformedBody(_)));
    }

    #[test]
    fn a_success_without_a_result_is_missing_not_null() {
        let error = decode_rpc_response(
            "system_name",
            "http://127.0.0.1:9944",
            200,
            r#"{"jsonrpc":"2.0","id":1}"#,
        )
        .unwrap_err();
        assert_eq!(error.kind, ServiceErrorKind::MissingField("result"));
    }

    #[tokio::test]
    async fn a_dead_endpoint_is_reported_as_unreachable() {
        // Bind and release a port, so the address is real but nothing listens on
        // it. The assertion is on the typed error, not on a message.
        let port = {
            let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("bind a free port");
            probe.local_addr().expect("read the bound address").port()
        };
        let client = RpcChainClient::new(format!("http://127.0.0.1:{port}"))
            .with_timeout(Duration::from_millis(750));

        let error = client.system_health().await.unwrap_err();
        assert!(
            matches!(error.kind, ServiceErrorKind::Unreachable(_)),
            "expected an unreachable error, got {error:?}"
        );
        assert_eq!(error.request, "system_health");
        assert_eq!(error.endpoint, client.url());
        // The transport's own words are kept for the operator log.
        if let ServiceErrorKind::Unreachable(detail) = error.kind {
            assert!(!detail.is_empty());
        }
    }

    #[tokio::test]
    async fn a_wedged_endpoint_is_reported_as_a_timeout_not_as_a_success() {
        // A listener that accepts and never answers. The deadline must fire and
        // must be classified as a timeout, which is a different operator fact
        // from "nothing is listening".
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let handle = std::thread::spawn(move || {
            if let Ok((socket, _)) = listener.accept() {
                std::thread::sleep(Duration::from_secs(5));
                drop(socket);
            }
        });

        let client = RpcChainClient::new(format!("http://127.0.0.1:{port}"))
            .with_timeout(Duration::from_millis(500));
        let error = client.system_health().await.unwrap_err();
        assert_eq!(error.kind, ServiceErrorKind::Timeout);

        std::mem::forget(handle);
    }

    #[tokio::test]
    async fn a_node_that_does_not_serve_node_roles_reports_no_role_instead_of_guessing() {
        let body =
            r#"{"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found"},"id":1}"#;
        let client = CannedRpc {
            response: body.to_string(),
        };
        assert_eq!(client.node_role().await.unwrap(), None);
    }

    /// The smallest possible node: one canned body for every method.
    struct CannedRpc {
        response: String,
    }

    impl ChainClient for CannedRpc {
        fn rpc_call<'a>(&'a self, method: &'a str, _params: Value) -> ChainFuture<'a, Value> {
            Box::pin(async move {
                let value: Value = serde_json::from_str(&self.response).expect("valid canned json");
                decode_rpc_response(method, "canned", 200, &value.to_string())
            })
        }
    }
}
