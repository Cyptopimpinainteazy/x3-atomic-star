//! # Build and verify a finality certificate from a live EVM chain
//!
//! The live drill's driver runs this binary. Each invocation is a **fresh process**, which is what
//! makes the persistence claim real: the accepted tip must be reloaded from the store file by a
//! process that never saw the certificate that accepted it.
//!
//! It prints a single JSON object and never fabricates a result:
//!
//! * `{"outcome":"finalized", ...}` — the producer built a certificate from the chain and the
//!   oracle accepted it.
//! * `{"outcome":"refused","code":"<SwapError variant>", ...}` — the chain data or the tip memory
//!   refused the certificate. The refusal *is* the result; the exit code stays 0.
//! * `{"outcome":"error","code":"...", ...}` — the tool could not do its job (bad arguments, RPC
//!   failure). Exit code 1.

use std::process::ExitCode;

use serde_json::json;
use x3_atomic_swap::error::SwapError;
use x3_atomic_swap::finality::{FinalityOracle, PersistentFinalityOracle};
use x3_atomic_swap::finality_producer::{EvmFinalityProducer, FileFinalityTipStore};
use x3_atomic_swap::intent::ChainKind;
use x3_atomic_swap::rpc_client::RpcClient;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn chain_kind(name: &str) -> Result<ChainKind, String> {
    match name {
        "eth" | "ethereum" => Ok(ChainKind::Ethereum),
        "base" => Ok(ChainKind::Base),
        "arbitrum" | "arb" => Ok(ChainKind::Arbitrum),
        "optimism" | "op" => Ok(ChainKind::Optimism),
        "bsc" => Ok(ChainKind::Bsc),
        "polygon" => Ok(ChainKind::Polygon),
        "avalanche" | "avax" => Ok(ChainKind::Avalanche),
        other => Err(format!("unknown --chain {other}")),
    }
}

fn error_code(err: &SwapError) -> &'static str {
    match err {
        SwapError::CertificateRewindsAcceptedAnchor { .. } => "CertificateRewindsAcceptedAnchor",
        SwapError::CertificateStale { .. } => "CertificateStale",
        SwapError::CertificateChainMismatch { .. } => "CertificateChainMismatch",
        SwapError::CertificateBlockAfterObservation { .. } => "CertificateBlockAfterObservation",
        SwapError::CertificateConfirmationsDisagree { .. } => "CertificateConfirmationsDisagree",
        SwapError::FinalityNotMet { .. } => "FinalityNotMet",
        SwapError::FinalityChainIdMismatch { .. } => "FinalityChainIdMismatch",
        SwapError::FinalityBlockHashMismatch { .. } => "FinalityBlockHashMismatch",
        SwapError::FinalityBlockNumberMismatch { .. } => "FinalityBlockNumberMismatch",
        SwapError::TxNotFound { .. } => "TxNotFound",
        SwapError::FinalityTipStore(_) => "FinalityTipStore",
        SwapError::RpcError(_) => "RpcError",
        _ => "Other",
    }
}

fn emit(value: serde_json::Value) {
    println!("{}", value);
}

fn main() -> ExitCode {
    let rpc_url = match arg("--rpc") {
        Some(v) => v,
        None => {
            emit(json!({"outcome":"error","code":"Usage","detail":"missing --rpc <url>"}));
            return ExitCode::from(1);
        }
    };
    let tx = match arg("--tx") {
        Some(v) => v,
        None => {
            emit(json!({"outcome":"error","code":"Usage","detail":"missing --tx <0xhash>"}));
            return ExitCode::from(1);
        }
    };
    let store_path = match arg("--store") {
        Some(v) => v,
        None => {
            emit(json!({"outcome":"error","code":"Usage","detail":"missing --store <path>"}));
            return ExitCode::from(1);
        }
    };
    let expected_chain_id = match arg("--expect-chain-id").and_then(|v| v.parse::<u64>().ok()) {
        Some(v) => v,
        None => {
            emit(
                json!({"outcome":"error","code":"Usage","detail":"missing --expect-chain-id <u64>"}),
            );
            return ExitCode::from(1);
        }
    };
    let name = arg("--chain").unwrap_or_else(|| "eth".to_string());
    let chain = match chain_kind(&name) {
        Ok(c) => c,
        Err(e) => {
            emit(json!({"outcome":"error","code":"Usage","detail":e}));
            return ExitCode::from(1);
        }
    };

    // Build the certificate from the chain first. A refusal here is a fact about the chain data
    // (wrong chain, unbound hash, unmined transaction), not about the oracle's memory.
    let reader = RpcClient::new(rpc_url, expected_chain_id);
    let mut producer = EvmFinalityProducer::new(reader, chain, expected_chain_id);
    let certificate = match producer.observe(&tx) {
        Ok(cert) => cert,
        Err(err) => {
            emit(json!({
                "outcome": "refused",
                "code": error_code(&err),
                "detail": err.to_string(),
            }));
            return ExitCode::SUCCESS;
        }
    };

    let store = FileFinalityTipStore::new(&store_path);
    let mut oracle = match PersistentFinalityOracle::load(store) {
        Ok(o) => o,
        Err(err) => {
            emit(json!({
                "outcome": "error",
                "code": error_code(&err),
                "detail": err.to_string(),
            }));
            return ExitCode::from(1);
        }
    };

    match oracle.verify_finality(chain, &certificate) {
        Ok(finalized) if finalized => {
            emit(json!({
                "outcome": "finalized",
                "chain": chain.as_str(),
                "block_height": certificate.block_height(),
                "block_hash": format!("0x{}", hex::encode(certificate.block_hash())),
                "tx_id": format!("0x{}", hex::encode(certificate.tx_id())),
                "confirmations": certificate.confirmations(),
                "observed_at": certificate.observed_at(),
                "accepted_tip": oracle.accepted_tip(chain),
                "store": store_path,
            }));
            ExitCode::SUCCESS
        }
        Ok(_) => {
            emit(json!({
                "outcome": "refused",
                "code": "FinalityNotMet",
                "detail": "the oracle returned a non-final verdict",
                "store": store_path,
            }));
            ExitCode::SUCCESS
        }
        Err(err) => {
            emit(json!({
                "outcome": "refused",
                "code": error_code(&err),
                "detail": err.to_string(),
                "chain": chain.as_str(),
                "block_height": certificate.block_height(),
                "tx_id": format!("0x{}", hex::encode(certificate.tx_id())),
                "observed_at": certificate.observed_at(),
                "accepted_tip": oracle.accepted_tip(chain),
                "store": store_path,
            }));
            ExitCode::SUCCESS
        }
    }
}
