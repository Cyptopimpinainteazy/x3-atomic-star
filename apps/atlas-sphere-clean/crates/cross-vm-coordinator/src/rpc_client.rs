//! Lightweight JSON-RPC client for cross-VM HTLC operations.
//!
//! Provides a minimal HTTP client for EVM, SVM, and X3 chain RPC calls
//! without requiring heavy dependencies like ethers-rs or solana-client.
//! Uses tokio + serde_json for async HTTP requests.

use crate::types::CoordinatorError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC request envelope.
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    method: String,
    params: Value,
    id: u64,
}

/// JSON-RPC response envelope.
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    result: Option<Value>,
    error: Option<JsonRpcError>,
    #[allow(dead_code)]
    id: u64,
}

/// JSON-RPC error.
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

/// Generic JSON-RPC client.
pub struct RpcClient {
    url: String,
    request_id: std::sync::atomic::AtomicU64,
}

impl RpcClient {
    pub fn new(url: String) -> Self {
        Self {
            url,
            request_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Execute a JSON-RPC call.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, CoordinatorError> {
        let id = self
            .request_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
            id,
        };

        let body = serde_json::to_string(&request).map_err(|e| {
            CoordinatorError::Internal(format!("Failed to serialize RPC request: {}", e))
        })?;

        // Use tokio's TCP stream for HTTP POST (minimal, no reqwest dependency)
        let response_body = self.http_post(&body).await?;

        let response: JsonRpcResponse = serde_json::from_str(&response_body).map_err(|e| {
            CoordinatorError::Internal(format!("Failed to parse RPC response: {}", e))
        })?;

        if let Some(err) = response.error {
            return Err(CoordinatorError::Internal(format!(
                "RPC error {}: {}",
                err.code, err.message
            )));
        }

        response
            .result
            .ok_or_else(|| CoordinatorError::Internal("RPC response missing result".into()))
    }

    /// Minimal HTTP POST implementation using tokio TCP.
    async fn http_post(&self, body: &str) -> Result<String, CoordinatorError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpStream;

        // Parse URL
        let url = &self.url;
        let (host, port, path) = parse_url(url)
            .map_err(|e| CoordinatorError::Internal(format!("Invalid RPC URL: {}", e)))?;

        let addr = format!("{}:{}", host, port);
        let mut stream = TcpStream::connect(&addr).await.map_err(|e| {
            CoordinatorError::Internal(format!("TCP connect to {} failed: {}", addr, e))
        })?;

        // Build HTTP request
        let http_request = format!(
            "POST {} HTTP/1.1\r\n\
             Host: {}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             {}",
            path,
            host,
            body.len(),
            body
        );

        stream
            .write_all(http_request.as_bytes())
            .await
            .map_err(|e| CoordinatorError::Internal(format!("HTTP write failed: {}", e)))?;

        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .map_err(|e| CoordinatorError::Internal(format!("HTTP read failed: {}", e)))?;

        let response_str = String::from_utf8_lossy(&response);

        // Extract body from HTTP response (after \r\n\r\n)
        let body_start = response_str.find("\r\n\r\n");
        match body_start {
            Some(pos) => Ok(response_str[pos + 4..].to_string()),
            None => Err(CoordinatorError::Internal("Malformed HTTP response".into())),
        }
    }
}

/// Parse URL into (host, port, path).
fn parse_url(url: &str) -> Result<(String, u16, String), String> {
    let url = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);

    let (host_port, path) = match url.find('/') {
        Some(pos) => (&url[..pos], url[pos..].to_string()),
        None => (url, "/".to_string()),
    };

    let (host, port) = match host_port.find(':') {
        Some(pos) => {
            let h = &host_port[..pos];
            let p: u16 = host_port[pos + 1..]
                .parse()
                .map_err(|_| "Invalid port".to_string())?;
            (h.to_string(), p)
        }
        None => (host_port.to_string(), 80),
    };

    Ok((host, port, path))
}

// ─── EVM-specific RPC helpers ─────────────────────────────────────────────────

impl RpcClient {
    /// eth_call — read-only EVM call.
    pub async fn eth_call(&self, to: &str, data: &str) -> Result<String, CoordinatorError> {
        let params = serde_json::json!([
            {
                "to": to,
                "data": data,
            },
            "latest"
        ]);
        let result = self.call("eth_call", params).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| {
            CoordinatorError::Internal("eth_call: expected hex string result".into())
        })
    }

    /// eth_sendRawTransaction — broadcast signed transaction.
    pub async fn eth_send_raw_tx(&self, raw_tx: &str) -> Result<String, CoordinatorError> {
        let params = serde_json::json!([raw_tx]);
        let result = self.call("eth_sendRawTransaction", params).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| {
            CoordinatorError::Internal("eth_sendRawTransaction: expected tx hash".into())
        })
    }

    /// eth_getTransactionReceipt — check if tx is confirmed.
    pub async fn eth_get_receipt(&self, tx_hash: &str) -> Result<Option<Value>, CoordinatorError> {
        let params = serde_json::json!([tx_hash]);
        let result = self.call("eth_getTransactionReceipt", params).await?;
        if result.is_null() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    /// eth_blockNumber — current block height.
    pub async fn eth_block_number(&self) -> Result<u64, CoordinatorError> {
        let result = self.call("eth_blockNumber", serde_json::json!([])).await?;
        let hex_str = result
            .as_str()
            .ok_or_else(|| CoordinatorError::Internal("expected hex block number".into()))?;
        let without_prefix = hex_str.strip_prefix("0x").unwrap_or(hex_str);
        u64::from_str_radix(without_prefix, 16)
            .map_err(|e| CoordinatorError::Internal(format!("Failed to parse block number: {}", e)))
    }

    /// eth_getBlock — get block timestamp.
    pub async fn eth_block_timestamp(&self) -> Result<u64, CoordinatorError> {
        let params = serde_json::json!(["latest", false]);
        let result = self.call("eth_getBlockByNumber", params).await?;
        let timestamp_hex = result["timestamp"]
            .as_str()
            .ok_or_else(|| CoordinatorError::Internal("Missing block timestamp".into()))?;
        let without_prefix = timestamp_hex.strip_prefix("0x").unwrap_or(timestamp_hex);
        u64::from_str_radix(without_prefix, 16)
            .map_err(|e| CoordinatorError::Internal(format!("Failed to parse timestamp: {}", e)))
    }
}

// ─── SVM-specific RPC helpers ─────────────────────────────────────────────────

impl RpcClient {
    /// getSlot — Solana current slot.
    pub async fn solana_get_slot(&self) -> Result<u64, CoordinatorError> {
        let result = self
            .call("getSlot", serde_json::json!([{"commitment": "finalized"}]))
            .await?;
        result
            .as_u64()
            .ok_or_else(|| CoordinatorError::Internal("Expected slot number".into()))
    }

    /// getAccountInfo — read Solana account data.
    pub async fn solana_get_account_info(
        &self,
        pubkey: &str,
    ) -> Result<Option<Vec<u8>>, CoordinatorError> {
        let params = serde_json::json!([
            pubkey,
            {"encoding": "base64", "commitment": "finalized"}
        ]);
        let result = self.call("getAccountInfo", params).await?;
        if result.is_null() || result["value"].is_null() {
            return Ok(None);
        }
        let data_str = result["value"]["data"][0]
            .as_str()
            .ok_or_else(|| CoordinatorError::Internal("Expected base64 account data".into()))?;

        // Decode base64
        let data = base64_decode(data_str)
            .map_err(|e| CoordinatorError::Internal(format!("Base64 decode failed: {}", e)))?;

        Ok(Some(data))
    }

    /// sendTransaction — broadcast Solana transaction.
    pub async fn solana_send_tx(&self, tx_base64: &str) -> Result<String, CoordinatorError> {
        let params = serde_json::json!([
            tx_base64,
            {"encoding": "base64", "preflightCommitment": "finalized"}
        ]);
        let result = self.call("sendTransaction", params).await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| CoordinatorError::Internal("Expected tx signature".into()))
    }
}

// ─── X3 Chain RPC helpers ─────────────────────────────────────────────────────

impl RpcClient {
    /// system_health — X3 node health check.
    pub async fn x3_health(&self) -> Result<bool, CoordinatorError> {
        let result = self.call("system_health", serde_json::json!([])).await?;
        Ok(!result["isSyncing"].as_bool().unwrap_or(true))
    }

    /// chain_getHeader — current finalized header.
    pub async fn x3_get_block_number(&self) -> Result<u64, CoordinatorError> {
        let result = self.call("chain_getHeader", serde_json::json!([])).await?;
        let number_hex = result["number"]
            .as_str()
            .ok_or_else(|| CoordinatorError::Internal("Missing block number".into()))?;
        let without_prefix = number_hex.strip_prefix("0x").unwrap_or(number_hex);
        u64::from_str_radix(without_prefix, 16).map_err(|e| {
            CoordinatorError::Internal(format!("Failed to parse X3 block number: {}", e))
        })
    }
}

// ─── Utility ──────────────────────────────────────────────────────────────────

/// Simple base64 decoder (avoid pulling in a crate for this).
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let input = input.trim_end_matches('=');
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;

    for c in input.bytes() {
        let val = TABLE
            .iter()
            .position(|&b| b == c)
            .ok_or_else(|| format!("Invalid base64 character: {}", c as char))?
            as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url_basic() {
        let (host, port, path) = parse_url("http://localhost:8545/").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 8545);
        assert_eq!(path, "/");
    }

    #[test]
    fn parse_url_no_port() {
        let (host, port, path) = parse_url("http://mainnet.infura.io/v3/key").unwrap();
        assert_eq!(host, "mainnet.infura.io");
        assert_eq!(port, 80);
        assert_eq!(path, "/v3/key");
    }

    #[test]
    fn base64_decode_works() {
        let encoded = "SGVsbG8=";
        let decoded = base64_decode(encoded).unwrap();
        assert_eq!(&decoded, b"Hello");
    }
}
