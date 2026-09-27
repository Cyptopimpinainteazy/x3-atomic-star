//! Chain task watcher.
//!
//! Reads the real NorthernSwarm::Tasks storage map. Storage prefixes are
//! derived from canonical Substrate Twox128 pallet/item names rather than a
//! fabricated full key, and task values are decoded using the pallet's own
//! TaskRecord/TaskKind/TaskStatus types.

use crate::{executor::TaskExecutor, result_submitter::ResultSubmitter, types::*};
use codec::Decode;
use serde_json::Value;
use tracing::{debug, error, info, warn};

type ChainTaskRecord = pallet_northern_swarm::TaskRecord<[u8; 32], u128, u32, [u8; 32]>;

/// Watches the chain for claimable swarm tasks and drives execution.
pub struct ChainWatcher {
    config: Config,
    http_client: reqwest::Client,
}

impl ChainWatcher {
    pub fn new(config: Config) -> Self {
        ChainWatcher {
            config,
            http_client: reqwest::Client::new(),
        }
    }

    /// Main event loop. A worker must win an on-chain claim before executing.
    pub async fn run(&self) -> Result<(), NorthernSwarmError> {
        info!(
            rpc = %self.config.chain_rpc_url,
            "chain watcher running — polling NorthernSwarm::Tasks every 6 s",
        );

        let executor = TaskExecutor::new(self.config.executor_key.clone());
        let submitter = ResultSubmitter::new(self.config.clone());

        loop {
            match self.poll_claimable_tasks().await {
                Ok(tasks) => {
                    if tasks.is_empty() {
                        debug!("no claimable tasks");
                    }

                    for task in tasks {
                        // The chain, not this process, arbitrates claim slots.
                        // A competing executor may win between the read and this
                        // signed extrinsic; that is a normal race, not a reason
                        // to execute work without ownership.
                        if let Err(error) = submitter.claim_task(&task.id).await {
                            debug!(
                                task_id = %task.id,
                                err = %error,
                                "task claim not accepted; skipping execution",
                            );
                            continue;
                        }

                        info!(task_id = %task.id, kind = ?task.kind, "claimed task; dispatching");
                        match self.fetch_payload(&task).await {
                            Ok(payload) => match executor.execute(payload).await {
                                Ok(result) => {
                                    if let Err(error) = submitter.submit(result).await {
                                        error!(
                                            task_id = %task.id,
                                            err = %error,
                                            "result submission failed",
                                        );
                                    }
                                }
                                Err(error) => {
                                    error!(task_id = %task.id, err = %error, "execution failed")
                                }
                            },
                            Err(error) => {
                                error!(task_id = %task.id, err = %error, "payload fetch failed")
                            }
                        }
                    }
                }
                Err(error) => warn!(err = %error, "poll_claimable_tasks error"),
            }

            tokio::time::sleep(std::time::Duration::from_secs(6)).await;
        }
    }

    /// Fetch Pending/Claimed records from the actual Tasks storage map.
    async fn poll_claimable_tasks(&self) -> Result<Vec<NorthernTask>, NorthernSwarmError> {
        let prefix = storage_prefix("NorthernSwarm", "Tasks");
        let prefix_hex = format!("0x{}", hex::encode(prefix));
        let keys = self
            .json_rpc_call("state_getKeys", &[Value::String(prefix_hex)])
            .await?;

        let keys = keys.as_array().ok_or_else(|| {
            NorthernSwarmError::ChainRpc("state_getKeys returned a non-array result".into())
        })?;

        let mut tasks = Vec::new();
        for key in keys {
            let Some(key_hex) = key.as_str() else {
                continue;
            };

            let value = self
                .json_rpc_call("state_getStorage", &[Value::String(key_hex.to_string())])
                .await?;
            let Some(value_hex) = value.as_str() else {
                continue;
            };

            let value_bytes = hex::decode(value_hex.trim_start_matches("0x")).map_err(|error| {
                NorthernSwarmError::ChainRpc(format!(
                    "Tasks value at {key_hex} is not valid hex: {error}"
                ))
            })?;
            let record = ChainTaskRecord::decode(&mut &value_bytes[..]).map_err(|error| {
                NorthernSwarmError::ChainRpc(format!(
                    "Tasks value at {key_hex} failed SCALE decode: {error}"
                ))
            })?;

            if !is_claimable_status(&record.status) {
                continue;
            }

            let key_bytes = hex::decode(key_hex.trim_start_matches("0x")).map_err(|error| {
                NorthernSwarmError::ChainRpc(format!("Tasks storage key is not valid hex: {error}"))
            })?;
            let task_id = task_id_from_storage_key(&key_bytes)?;

            let payload_uri =
                String::from_utf8(record.payload_uri.into_inner()).map_err(|error| {
                    NorthernSwarmError::PayloadFetch {
                        uri: "<NorthernSwarm::Tasks>".into(),
                        reason: format!("payload_uri is not UTF-8: {error}"),
                    }
                })?;

            tasks.push(NorthernTask {
                id: format!("0x{}", hex::encode(task_id)),
                payload_uri,
                submitted_at_block: u64::from(record.submitted_at),
                kind: record.kind,
                status: record.status,
                x3_bytecode_hash: None,
            });
        }

        Ok(tasks)
    }

    /// Fetch task payload from the content-addressed store.
    async fn fetch_payload(&self, task: &NorthernTask) -> Result<TaskPayload, NorthernSwarmError> {
        if let Some(cid) = task.payload_uri.strip_prefix("ipfs://") {
            let gateway = if self.config.ipfs_gateway.is_empty() {
                "https://ipfs.io"
            } else {
                &self.config.ipfs_gateway
            };
            let url = format!("{gateway}/ipfs/{cid}");
            let resp = self.http_client.get(&url).send().await.map_err(|error| {
                NorthernSwarmError::PayloadFetch {
                    uri: task.payload_uri.clone(),
                    reason: format!("HTTP GET failed: {error}"),
                }
            })?;

            if !resp.status().is_success() {
                return Err(NorthernSwarmError::PayloadFetch {
                    uri: task.payload_uri.clone(),
                    reason: format!("IPFS gateway returned HTTP {}", resp.status()),
                });
            }

            let body = resp
                .bytes()
                .await
                .map_err(|error| NorthernSwarmError::PayloadFetch {
                    uri: task.payload_uri.clone(),
                    reason: format!("read body failed: {error}"),
                })?;

            return Ok(TaskPayload {
                task_id: task.id.clone(),
                kind: task.kind.clone(),
                body: body.to_vec(),
                params: Default::default(),
                input_uri: None,
            });
        }

        if let Some(hex_body) = task.payload_uri.strip_prefix("hex:") {
            let body = hex::decode(hex_body).map_err(|error| NorthernSwarmError::PayloadFetch {
                uri: task.payload_uri.clone(),
                reason: error.to_string(),
            })?;
            return Ok(TaskPayload {
                task_id: task.id.clone(),
                kind: task.kind.clone(),
                body,
                params: Default::default(),
                input_uri: None,
            });
        }

        Err(NorthernSwarmError::PayloadFetch {
            uri: task.payload_uri.clone(),
            reason: "unsupported URI scheme (want: ipfs:// or hex:)".into(),
        })
    }

    async fn json_rpc_call(
        &self,
        method: &str,
        params: &[Value],
    ) -> Result<Value, NorthernSwarmError> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        let rpc_url = http_rpc_url(&self.config.chain_rpc_url);
        let resp = self
            .http_client
            .post(&rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|error| NorthernSwarmError::ChainConnection {
                url: rpc_url.clone(),
                reason: error.to_string(),
            })?;

        let mut json: Value =
            resp.json()
                .await
                .map_err(|error| NorthernSwarmError::ChainConnection {
                    url: rpc_url.clone(),
                    reason: format!("decode response: {error}"),
                })?;

        if let Some(error) = json.get("error") {
            return Err(NorthernSwarmError::ChainConnection {
                url: rpc_url,
                reason: format!("RPC error: {error}"),
            });
        }

        Ok(json["result"].take())
    }
}

fn is_claimable_status(status: &TaskStatus) -> bool {
    // A first result commit must not make the task invisible to the remaining
    // executors needed to form quorum. The pallet's claim_task accepts this
    // exact set, so discovery and dispatch cannot drift apart.
    matches!(
        status,
        TaskStatus::Pending | TaskStatus::Claimed | TaskStatus::ResultCommitted
    )
}

fn storage_prefix(pallet: &str, item: &str) -> Vec<u8> {
    let mut prefix = Vec::with_capacity(32);
    prefix.extend_from_slice(&sp_core::twox_128(pallet.as_bytes()));
    prefix.extend_from_slice(&sp_core::twox_128(item.as_bytes()));
    prefix
}

fn task_id_from_storage_key(key: &[u8]) -> Result<[u8; 32], NorthernSwarmError> {
    // StorageMap<Blake2_128Concat, H256, ...>:
    // 32 bytes pallet/item prefix + 16 byte Blake2 hash + 32 byte raw H256.
    const EXPECTED: usize = 32 + 16 + 32;
    if key.len() != EXPECTED {
        return Err(NorthernSwarmError::ChainRpc(format!(
            "unexpected NorthernSwarm::Tasks key length: {}, expected {EXPECTED}",
            key.len()
        )));
    }

    key[key.len() - 32..]
        .try_into()
        .map_err(|_| NorthernSwarmError::ChainRpc("task id extraction failed".into()))
}

fn http_rpc_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("ws://") {
        format!("http://{rest}")
    } else if let Some(rest) = url.strip_prefix("wss://") {
        format!("https://{rest}")
    } else {
        url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_prefix_is_32_bytes_and_stable() {
        let prefix = storage_prefix("NorthernSwarm", "Tasks");
        assert_eq!(prefix.len(), 32);
        assert_eq!(prefix, storage_prefix("NorthernSwarm", "Tasks"));
    }

    #[test]
    fn task_id_is_extracted_from_blake2_concat_map_key() {
        let id = [0x42; 32];
        let mut key = storage_prefix("NorthernSwarm", "Tasks");
        key.extend_from_slice(&[0xAA; 16]);
        key.extend_from_slice(&id);
        assert_eq!(task_id_from_storage_key(&key).unwrap(), id);
    }

    #[test]
    fn result_committed_task_remains_discoverable_for_quorum() {
        assert!(is_claimable_status(&TaskStatus::Pending));
        assert!(is_claimable_status(&TaskStatus::Claimed));
        assert!(is_claimable_status(&TaskStatus::ResultCommitted));
        assert!(!is_claimable_status(&TaskStatus::Finalised));
        assert!(!is_claimable_status(&TaskStatus::Disputed));
    }

    #[test]
    fn websocket_rpc_url_is_converted_for_http_polling() {
        assert_eq!(http_rpc_url("ws://127.0.0.1:9944"), "http://127.0.0.1:9944");
        assert_eq!(http_rpc_url("wss://rpc.example"), "https://rpc.example");
    }
}
