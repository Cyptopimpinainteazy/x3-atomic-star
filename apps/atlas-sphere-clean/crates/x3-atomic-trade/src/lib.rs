#![allow(
    dead_code,
    unused_imports,
    unused_variables,
    unused_mut,
    non_snake_case,
    unexpected_cfgs,
    unused_parens,
    non_camel_case_types,
    clippy::all
)]

//! X3 Atomic Trade Engine
//!
//! RPC endpoints for swaps, quotes, and DEX operations.

pub mod rollback_listener;
pub mod swap_rpc;

pub use rollback_listener::{
    FailureNotification, FailureReason, RollbackEventListener, RollbackLog, SeverityLevel,
    TradeBatchFailure,
};
pub use swap_rpc::{AMMPool, SwapOrder, SwapQuote, SwapRPCServer, SwapStatus, TokenPair};
