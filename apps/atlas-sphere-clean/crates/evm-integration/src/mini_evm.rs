//! Minimal no-std EVM executor using SputnikVM (`evm` crate)
//!
//! Provides real EVM bytecode execution without requiring `std`.
//! Uses `evm::executor::stack::StackExecutor` with an in-memory backend
//! backed by `alloc::collections::BTreeMap` (available in no-std + alloc).
//!
//! # Limitations vs the full Frontier runner
//! - No persistent world-state across calls (ephemeral in-memory backend)
//! - No chain-specific precompiles (SHA-256, ECRECOVER, etc. are skipped)
//! - The kernel's canonical ledger records the authoritative balance changes;
//!   this adapter supplies gas accounting, return data, and success flag.

use crate::{EvmError, EvmExecutionResult, EvmResult};

use evm::{
    backend::{MemoryAccount, MemoryBackend, MemoryVicinity},
    executor::stack::{MemoryStackState, PrecompileFn, StackExecutor, StackSubstateMetadata},
    Config, ExitReason,
};
use sp_core::{H160, U256};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::prelude::Vec;
use sp_std::vec;

/// Execute EVM bytecode using the SputnikVM interpreter (no-std compatible).
///
/// `payload` is the call-data / init-code.  The adapter seeds a deterministic
/// contract address with `payload` as its code and issues a `CALL` into it.
pub fn execute_evm(payload: &[u8], gas_limit: u64) -> EvmResult<EvmExecutionResult> {
    if payload.is_empty() {
        return Err(EvmError::InvalidPayload);
    }

    let config = Config::istanbul();

    let vicinity = MemoryVicinity {
        gas_price: U256::from(1u64),
        origin: H160::zero(),
        block_hashes: vec![],
        block_number: U256::zero(),
        block_coinbase: H160::zero(),
        block_timestamp: U256::zero(),
        block_difficulty: U256::zero(),
        block_gas_limit: U256::from(u64::MAX),
        chain_id: U256::from(3375u64), // X3 chain ID placeholder
        block_base_fee_per_gas: U256::from(1u64),
        block_randomness: None,
    };

    // Derive a deterministic contract address from the payload hash
    let target_addr = derive_target(payload);

    let mut state_map: BTreeMap<H160, MemoryAccount> = BTreeMap::new();
    // Caller has enough balance to cover any gas cost
    state_map.insert(
        H160::zero(),
        MemoryAccount {
            nonce: U256::zero(),
            balance: U256::from(u128::MAX),
            storage: BTreeMap::new(),
            code: vec![],
        },
    );
    // Contract: code = payload, already deployed
    state_map.insert(
        target_addr,
        MemoryAccount {
            nonce: U256::one(),
            balance: U256::zero(),
            storage: BTreeMap::new(),
            code: payload.to_vec(),
        },
    );

    let mut backend = MemoryBackend::new(&vicinity, state_map);
    let metadata = StackSubstateMetadata::new(gas_limit, &config);
    let stack_state = MemoryStackState::new(metadata, &mut backend);
    // No precompiles in the no-std path
    let precompiles: BTreeMap<H160, PrecompileFn> = BTreeMap::new();
    let mut executor = StackExecutor::new_with_precompiles(stack_state, &config, &precompiles);

    let (exit_reason, return_data) = executor.transact_call(
        H160::zero(),     // caller
        target_addr,      // target
        U256::zero(),     // value
        payload.to_vec(), // call data
        gas_limit,
        vec![], // access_list
    );

    let gas_used = executor.used_gas();

    let success = matches!(exit_reason, ExitReason::Succeed(_));

    if !success {
        let err = map_exit_reason(&exit_reason, gas_used);
        return Err(err);
    }

    Ok(EvmExecutionResult {
        success: true,
        output: return_data,
        gas_used,
        logs: Vec::new(), // canonical ledger owns the event log
        state_changes: Vec::new(),
        state_root: [0u8; 32],
    })
}

/// Estimate gas for EVM payload (EIP-2028 formula, no execution).
pub fn estimate_gas_evm(payload: &[u8]) -> EvmResult<u64> {
    if payload.is_empty() {
        return Err(EvmError::InvalidPayload);
    }
    let calldata_gas: u64 = payload
        .iter()
        .map(|&b| if b == 0 { 4u64 } else { 16u64 })
        .sum();
    Ok(21_000 + calldata_gas)
}

/// Basic structural validation for EVM payload.
pub fn validate_evm(payload: &[u8]) -> EvmResult<()> {
    if payload.is_empty() {
        return Err(EvmError::InvalidPayload);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Derive a stable H160 target address from payload bytes via blake2-256.
fn derive_target(payload: &[u8]) -> H160 {
    let hash = sp_io::hashing::blake2_256(payload);
    H160::from_slice(&hash[..20])
}

fn map_exit_reason(reason: &ExitReason, gas_used: u64) -> EvmError {
    match reason {
        ExitReason::Revert(_) => EvmError::ExecutionReverted,
        ExitReason::Error(evm::ExitError::OutOfGas) => EvmError::OutOfGas,
        ExitReason::Error(evm::ExitError::StackOverflow) => EvmError::StackOverflow,
        ExitReason::Error(evm::ExitError::StackUnderflow) => EvmError::StackUnderflow,
        ExitReason::Error(evm::ExitError::CreateCollision) => EvmError::CreateCollision,
        ExitReason::Error(evm::ExitError::InvalidCode(op)) => EvmError::InvalidOpcode(op.as_u8()),
        _ => EvmError::ExecutionFailed(gas_used as u32),
    }
}
