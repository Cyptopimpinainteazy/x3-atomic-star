//! Types for X3 Parallel Executor

use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_std::vec::Vec;

pub type TransactionId = u64;
pub type StateKey = [u8; 32];

#[derive(Clone, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct Transaction {
    pub id: TransactionId,
    pub instructions: Vec<Instruction>,
}

#[derive(Clone, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct Instruction {
    pub opcode: u8,
    pub operands: Vec<u8>,
}

#[derive(Clone, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct AccessList {
    pub reads: Vec<StateKey>,
    pub writes: Vec<StateKey>,
}

#[derive(Clone, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct Conflict {
    pub tx1: usize,
    pub tx2: usize,
    pub key: StateKey,
}

#[derive(Clone, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct ExecutionBatch {
    pub transactions: Vec<Transaction>,
}

#[derive(Clone, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct TransactionResult {
    pub tx_id: TransactionId,
    pub success: bool,
    pub state_changes: Vec<StateChange>,
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct ExecutionResult {
    pub results: Vec<TransactionResult>,
    pub state_hash: [u8; 32],
}

#[derive(Clone, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct StateChange {
    pub key: StateKey,
    pub old_value: Vec<u8>,
    pub new_value: Vec<u8>,
}

#[derive(Clone, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct Event {
    pub topic: Vec<u8>,
    pub data: Vec<u8>,
}