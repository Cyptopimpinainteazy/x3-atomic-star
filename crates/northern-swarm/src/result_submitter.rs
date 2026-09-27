//! Signed Northern Swarm transaction submitter.
//!
//! All calls are encoded from live runtime metadata through Subxt. There are no
//! hard-coded pallet/call indices and no unsigned shortcut: the same registered
//! executor key that claims a task signs the result commit.

use crate::types::*;
use std::str::FromStr;
use subxt::{
    dynamic::{tx, Value},
    transactions::DynamicPayload,
    OnlineClient, SubstrateConfig,
};
use subxt_signer::{sr25519::Keypair, SecretUri};
use tracing::{info, warn};

/// Dynamic transaction payload shape used by Northern Swarm calls.
type SwarmCall = DynamicPayload<Vec<Value>>;

/// Submits claims/results and persists proof bundles.
pub struct ResultSubmitter {
    config: Config,
}

impl ResultSubmitter {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Claim one on-chain task as this configured executor.
    pub async fn claim_task(&self, task_id: &str) -> Result<(), NorthernSwarmError> {
        let task_id = parse_h256(task_id, "task_id")?;
        let call = tx(
            "NorthernSwarm",
            "claim_task",
            vec![Value::from_bytes(task_id)],
        );

        self.submit_signed("claim_task", &call).await?;
        info!(task_id = %hex::encode(task_id), "task claim finalised on-chain");
        Ok(())
    }

    /// Submit a successful execution result as the registered executor.
    pub async fn submit(&self, result: ExecutionResult) -> Result<(), NorthernSwarmError> {
        if result.status != ExecutionStatus::Success {
            warn!(
                task_id = %result.task_id,
                status = ?result.status,
                "skipping submission for non-success result",
            );
            return Ok(());
        }

        let task_id = parse_h256(&result.task_id, "task_id")?;
        let result_hash = parse_h256(&result.result_hash, "result_hash")?;
        let call = tx(
            "NorthernSwarm",
            "submit_result",
            vec![Value::from_bytes(task_id), Value::from_bytes(result_hash)],
        );

        self.submit_signed("submit_result", &call).await?;

        info!(
            task_id = %result.task_id,
            result_hash = %result.result_hash,
            "result hash finalised on-chain",
        );

        self.store_proof_locally(&result.proof).await?;
        Ok(())
    }

    fn signer(&self) -> Result<Keypair, NorthernSwarmError> {
        let uri = SecretUri::from_str(&self.config.executor_key).map_err(|error| {
            NorthernSwarmError::Crypto(format!("invalid NS_EXECUTOR_KEY secret URI: {error}"))
        })?;

        Keypair::from_uri(&uri).map_err(|error| {
            NorthernSwarmError::Crypto(format!("executor key derivation failed: {error}"))
        })
    }

    async fn client(&self) -> Result<OnlineClient<SubstrateConfig>, NorthernSwarmError> {
        OnlineClient::<SubstrateConfig>::from_url(&self.config.chain_rpc_url)
            .await
            .map_err(|error| NorthernSwarmError::ChainConnection {
                url: self.config.chain_rpc_url.clone(),
                reason: error.to_string(),
            })
    }

    async fn submit_signed(
        &self,
        operation: &str,
        call: &SwarmCall,
    ) -> Result<(), NorthernSwarmError> {
        let api = self.client().await?;
        let at = api
            .at_current_block()
            .await
            .map_err(|error| NorthernSwarmError::ChainRpc(error.to_string()))?;
        let signer = self.signer()?;
        let mut tx = at.tx();

        let progress = tx
            .sign_and_submit_then_watch_default(call, &signer)
            .await
            .map_err(|error| NorthernSwarmError::SubmitFailed {
                task_id: operation.to_string(),
                reason: error.to_string(),
            })?;

        progress
            .wait_for_finalized_success()
            .await
            .map_err(|error| NorthernSwarmError::SubmitFailed {
                task_id: operation.to_string(),
                reason: format!("transaction did not finalise successfully: {error}"),
            })?;

        Ok(())
    }

    /// Persist a proof bundle beneath the local proofs directory.
    async fn store_proof_locally(&self, proof: &ProofBundle) -> Result<(), NorthernSwarmError> {
        let dir = std::path::PathBuf::from("proofs");
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{}.proof.json", proof.task_id));
        let json = serde_json::to_vec_pretty(proof)?;
        tokio::fs::write(&path, json).await?;
        info!(task_id = %proof.task_id, path = %path.display(), "proof bundle stored");
        Ok(())
    }
}

fn parse_h256(value: &str, field: &str) -> Result<[u8; 32], NorthernSwarmError> {
    let bytes = hex::decode(value.trim_start_matches("0x")).map_err(|error| {
        NorthernSwarmError::SubmitFailed {
            task_id: value.to_string(),
            reason: format!("{field} is not valid hex: {error}"),
        }
    })?;

    bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| NorthernSwarmError::SubmitFailed {
            task_id: value.to_string(),
            reason: format!("{field} must be exactly 32 bytes, got {}", bytes.len()),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn h256_parser_rejects_short_ids_instead_of_padding_them() {
        let err = parse_h256("deadbeef", "task_id").unwrap_err();
        assert!(err.to_string().contains("exactly 32 bytes"));
    }

    #[test]
    fn h256_parser_accepts_prefixed_and_unprefixed_ids() {
        let raw = "11".repeat(32);
        assert_eq!(parse_h256(&raw, "task_id").unwrap(), [0x11; 32]);
        assert_eq!(
            parse_h256(&format!("0x{raw}"), "task_id").unwrap(),
            [0x11; 32]
        );
    }
}
