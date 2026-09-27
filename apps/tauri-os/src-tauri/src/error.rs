//! Typed failures for every read the operator console makes against a real
//! service.
//!
//! The console used to answer questions it could not actually answer: a
//! fabricated process-spawn string, a cached swarm list, a node status that was
//! `running: false` forever. Everything here exists so that a command which
//! cannot get an answer returns an error that names what failed, where, and
//! why — never a plausible-looking value. `Unreachable` is the only meaning of
//! an endpoint that did not answer; it is never converted into success.

use serde::Serialize;
use std::fmt;

/// Every way a read against the node's JSON-RPC endpoint or the swarm HTTP API
/// can fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceErrorKind {
    /// The endpoint refused the connection, the host did not resolve, or TLS
    /// failed. Carries the transport's own words.
    Unreachable(String),
    /// The endpoint accepted the connection and did not answer inside the
    /// configured deadline.
    Timeout,
    /// The endpoint answered with a status outside 2xx.
    HttpStatus(u16),
    /// The endpoint answered 2xx with a body that is not the JSON we asked for.
    MalformedBody(String),
    /// The service answered with a JSON-RPC error object. `code` is `None` when
    /// the error object did not carry an integer code, which is itself a
    /// protocol violation we refuse to invent a code for.
    RpcError { code: Option<i64>, message: String },
    /// The answer parsed as JSON but is missing a field the caller needs.
    MissingField(&'static str),
    /// A caller-supplied identifier that must not be interpolated into a URL.
    RejectedInput(String),
}

/// A failed read, with enough context to name the request in an operator log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceError {
    /// The method or request that was attempted, e.g. `system_health`.
    pub request: String,
    /// The endpoint it was attempted against. Empty for pure parsers, which
    /// never touch the network.
    pub endpoint: String,
    pub kind: ServiceErrorKind,
}

impl ServiceError {
    pub fn new(
        request: impl Into<String>,
        endpoint: impl Into<String>,
        kind: ServiceErrorKind,
    ) -> Self {
        Self {
            request: request.into(),
            endpoint: endpoint.into(),
            kind,
        }
    }

    pub fn unreachable(
        request: impl Into<String>,
        endpoint: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(
            request,
            endpoint,
            ServiceErrorKind::Unreachable(detail.into()),
        )
    }

    pub fn timeout(request: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self::new(request, endpoint, ServiceErrorKind::Timeout)
    }

    pub fn http_status(
        request: impl Into<String>,
        endpoint: impl Into<String>,
        status: u16,
    ) -> Self {
        Self::new(request, endpoint, ServiceErrorKind::HttpStatus(status))
    }

    pub fn malformed(request: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(request, "", ServiceErrorKind::MalformedBody(detail.into()))
    }

    pub fn rpc(
        request: impl Into<String>,
        endpoint: impl Into<String>,
        code: Option<i64>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            request,
            endpoint,
            ServiceErrorKind::RpcError {
                code,
                message: message.into(),
            },
        )
    }

    pub fn missing(request: impl Into<String>, field: &'static str) -> Self {
        Self::new(request, "", ServiceErrorKind::MissingField(field))
    }

    pub fn rejected(request: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(request, "", ServiceErrorKind::RejectedInput(reason.into()))
    }

    /// True when the node answered that the method is not served at all.
    ///
    /// Used by exactly one caller: the optional `system_nodeRoles` probe, where
    /// "this node does not report its role" is a fact worth reporting and every
    /// other answer is a real failure.
    pub fn is_method_not_found(&self) -> bool {
        matches!(
            &self.kind,
            ServiceErrorKind::RpcError {
                code: Some(jsonrpc_error::METHOD_NOT_FOUND),
                ..
            }
        )
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.endpoint.is_empty() {
            write!(f, "{}: {}", self.request, self.kind)
        } else {
            write!(f, "{} ({}): {}", self.request, self.endpoint, self.kind)
        }
    }
}

impl fmt::Display for ServiceErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceErrorKind::Unreachable(detail) => write!(f, "unreachable: {detail}"),
            ServiceErrorKind::Timeout => write!(f, "no answer before the deadline"),
            ServiceErrorKind::HttpStatus(status) => write!(f, "HTTP status {status}"),
            ServiceErrorKind::MalformedBody(detail) => write!(f, "malformed body: {detail}"),
            ServiceErrorKind::RpcError { code, message } => match code {
                Some(code) => write!(f, "service error {code}: {message}"),
                None => write!(f, "service error without a code: {message}"),
            },
            ServiceErrorKind::MissingField(field) => write!(f, "the answer has no `{field}`"),
            ServiceErrorKind::RejectedInput(reason) => {
                write!(f, "refused to build the request: {reason}")
            }
        }
    }
}

impl std::error::Error for ServiceError {}

/// The JSON-RPC error codes this console branches on.
pub mod jsonrpc_error {
    /// `Method not found`, served by jsonrpsee for a method nobody registered.
    pub const METHOD_NOT_FOUND: i64 = -32601;
}

/// The error the webview receives. `code` is a stable, greppable string; the
/// operator-facing sentence is `message`; `details` keeps the raw evidence.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IpcError {
    pub code: &'static str,
    pub message: String,
    pub details: Option<String>,
}

/// The endpoint did not answer (or answered nothing in time).
pub const IPC_SERVICE_UNREACHABLE: &str = "SERVICE_UNREACHABLE";
/// The endpoint answered and refused the request.
pub const IPC_SERVICE_REJECTED: &str = "SERVICE_REJECTED";
/// The endpoint answered with something we cannot read.
pub const IPC_SERVICE_BAD_RESPONSE: &str = "SERVICE_BAD_RESPONSE";

impl IpcError {
    pub fn new(code: &'static str, message: impl Into<String>, details: Option<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details,
        }
    }
}

impl From<ServiceError> for IpcError {
    fn from(error: ServiceError) -> Self {
        let code = match &error.kind {
            ServiceErrorKind::Unreachable(_) | ServiceErrorKind::Timeout => IPC_SERVICE_UNREACHABLE,
            ServiceErrorKind::HttpStatus(_) | ServiceErrorKind::RpcError { .. } => {
                IPC_SERVICE_REJECTED
            }
            ServiceErrorKind::MalformedBody(_)
            | ServiceErrorKind::MissingField(_)
            | ServiceErrorKind::RejectedInput(_) => IPC_SERVICE_BAD_RESPONSE,
        };
        let message = if error.endpoint.is_empty() {
            format!("{} did not produce a usable answer", error.request)
        } else {
            format!(
                "{} at {} did not produce a usable answer",
                error.request, error.endpoint
            )
        };
        Self {
            code,
            message,
            details: Some(error.to_string()),
        }
    }
}

impl fmt::Display for IpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for IpcError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unreachable_service_is_reported_as_unreachable_not_as_an_empty_answer() {
        let error = ServiceError::unreachable(
            "system_health",
            "http://127.0.0.1:9944",
            "connection refused",
        );
        assert!(!error.is_method_not_found());
        let ipc = IpcError::from(error);
        assert_eq!(ipc.code, IPC_SERVICE_UNREACHABLE);
        assert!(ipc.details.unwrap().contains("connection refused"));
    }

    #[test]
    fn a_node_that_refuses_the_request_is_not_reported_as_unreachable() {
        let error = ServiceError::rpc(
            "chain_getHeader",
            "http://127.0.0.1:9944",
            Some(-32602),
            "Invalid params",
        );
        let ipc = IpcError::from(error);
        assert_eq!(ipc.code, IPC_SERVICE_REJECTED);
    }

    #[test]
    fn an_error_object_without_a_code_is_not_given_one() {
        let error = ServiceError::rpc("chain_getHeader", "http://127.0.0.1:9944", None, "boom");
        assert!(!error.is_method_not_found());
        assert!(error.to_string().contains("without a code"));
    }

    #[test]
    fn method_not_found_is_recognised_by_its_real_code() {
        let error = ServiceError::rpc(
            "system_nodeRoles",
            "http://127.0.0.1:9944",
            Some(jsonrpc_error::METHOD_NOT_FOUND),
            "Method not found",
        );
        assert!(error.is_method_not_found());
    }

    #[test]
    fn a_refused_identifier_is_a_bad_request_not_a_transport_failure() {
        let error = ServiceError::rejected("POST /tasks/{id}/approve", "the task id contains `..`");
        let ipc = IpcError::from(error);
        assert_eq!(ipc.code, IPC_SERVICE_BAD_RESPONSE);
    }
}
