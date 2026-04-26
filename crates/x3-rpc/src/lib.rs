#![allow(
    dead_code,
    unused_imports,
    unused_variables,
    unused_mut,
    non_snake_case,
    unexpected_cfgs,
    unused_parens,
    non_camel_case_types,
    deprecated,
    clippy::all
)]

//! X3 RPC Server
//!
//! JSON-RPC endpoints for block exploration, gas estimation, wallet operations, and DEX integration.

pub mod benchmark;
pub mod gas_estimation;
pub mod wallet_dex_rpc;

pub use benchmark::{
    BenchmarkChainType, BenchmarkIntegrationTier, BenchmarkJobRequest, BenchmarkJobResponse,
    BenchmarkJobStatus, BenchmarkMetrics, BenchmarkReport, BenchmarkReportArtifact,
    BenchmarkReportSummary, BenchmarkRpcApi, BenchmarkService, X3BenchmarkRpc,
};
pub use gas_estimation::{ExecutionStatus, GasEstimation, GasEstimationRPC, RPCTransaction};
pub use wallet_dex_rpc::{
    HardwareSigningRequest, SwapRequest, SwapResponse, WalletDexApi, WalletDexRpc,
};
