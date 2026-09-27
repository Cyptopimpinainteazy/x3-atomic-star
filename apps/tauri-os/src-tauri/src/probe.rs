//! Reachability of the local services the operator console watches.
//!
//! A probe answers one question: did this endpoint answer with a success? A
//! service that refuses the connection is an error the panel shows next to the
//! service; a service that answers `500` is a reachable-but-unhealthy service.
//! The two are different operator facts, so they are different values.

use crate::error::ServiceError;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

/// A local service should answer quickly or be reported as down.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Boxed future, so the command layer can take `&dyn ServiceProbe`.
pub type ProbeFuture<'a> = Pin<Box<dyn Future<Output = Result<bool, ServiceError>> + Send + 'a>>;

/// Probes one URL.
pub trait ServiceProbe: Send + Sync {
    /// `Ok(true)` when the endpoint answered 2xx, `Ok(false)` when it answered
    /// anything else, and `Err` when it did not answer at all.
    fn probe<'a>(&'a self, url: &'a str) -> ProbeFuture<'a>;
}

/// The real probe: an HTTP GET.
#[derive(Debug, Clone)]
pub struct HttpProbe {
    http: reqwest::Client,
    timeout: Duration,
}

impl HttpProbe {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
            timeout: PROBE_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl Default for HttpProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceProbe for HttpProbe {
    fn probe<'a>(&'a self, url: &'a str) -> ProbeFuture<'a> {
        Box::pin(async move {
            let response = self
                .http
                .get(url)
                .timeout(self.timeout)
                .send()
                .await
                .map_err(|error| {
                    if error.is_timeout() {
                        ServiceError::timeout(url, url)
                    } else {
                        ServiceError::unreachable(url, url, error.to_string())
                    }
                })?;
            Ok(response.status().is_success())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ServiceErrorKind;

    #[tokio::test]
    async fn a_port_nothing_listens_on_is_an_error_not_an_unhealthy_service() {
        let port = {
            let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("bind a free port");
            probe.local_addr().expect("read the bound address").port()
        };
        let url = format!("http://127.0.0.1:{port}/health");
        let error = HttpProbe::new()
            .with_timeout(Duration::from_millis(750))
            .probe(&url)
            .await
            .unwrap_err();
        assert!(matches!(error.kind, ServiceErrorKind::Unreachable(_)));
    }
}
