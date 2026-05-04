pub mod coordinator;
pub mod verifier;
pub mod signers;
pub mod ipc;

/// Wallet Core entrypoint.
/// Strict boundary: Only `verifier` talks to RPC. `signers` isolated.
pub struct ExecutionFirewall {}
