use serde::{Serialize, Deserialize};

pub type Hash = [u8; 32];
pub type Address = [u8; 32];

// Stub representations built from Chapter 3 of Design Booklet

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QC {
    pub view: u64,
    pub block_hash: Hash,
    pub signatures: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub parent_hash: Hash,
    pub height: u64,
    pub round: u64,
    pub timestamp: u64,
    pub validator_set_hash: Hash,
    pub qc: QC,
    pub proposer: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashingEvent {
    pub offender: Address,
    pub reason: String,
    pub amount: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub actions: Vec<ActionCommitment>,
    pub action_dag_root: Hash,
    pub execution_order_hash: Hash,
    pub state_root_pre: Hash,
    pub state_root_post: Hash,
    pub receipts_root: Hash,
    pub slashing_events: Vec<SlashingEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionCommitment {
    pub id: u64,
    pub hash: Hash,
}

#[derive(Debug, Clone)]
pub struct ChainState {
    pub last_block: Hash,
    pub state_root: Hash,
}

impl ChainState {
    pub fn state_root(&self) -> Hash {
        self.state_root
    }
}

pub enum BlockError {
    InvalidHeader,
    InvalidQC,
    InvalidParent,
    InvalidDag,
    InvalidOrder,
    ExecutionFailure,
    StateMismatch,
    SlashingFailure,
}

// Logic stubs to make hotstuff ApplyBlock compile and outline architectural intent
fn verify_header(_header: &BlockHeader) -> Result<(), BlockError> { Ok(()) }
fn verify_quorum_cert(_qc: &QC) -> Result<(), BlockError> { Ok(()) }
fn hash<T>(_: &T) -> Hash { [0u8; 32] }

struct DagStub {
    root: Hash,
}
impl DagStub {
    fn root_hash(&self) -> Hash { self.root }
}

fn derive_action_dag(_: &[ActionCommitment]) -> Result<DagStub, BlockError> { Ok(DagStub { root: [0u8; 32] }) }
fn derive_execution_order(_: &DagStub) -> Vec<ActionCommitment> { vec![] }
fn execute_action(_: &mut ChainState, _: &ActionCommitment) -> Result<Hash, BlockError> { Ok([0u8; 32]) }
fn apply_slashing(_: &mut ChainState, _: &[SlashingEvent]) -> Result<(), BlockError> { Ok(()) }

/// Core ApplyBlock state machine function logic implemented identically to the design booklet
pub fn apply_block(state: &ChainState, block: &Block) -> Result<ChainState, BlockError> {
    // 1. Verify block header and QC
    verify_header(&block.header)?;
    verify_quorum_cert(&block.header.qc)?;
    if block.header.parent_hash != state.last_block {
        return Err(BlockError::InvalidParent);
    }

    // 2. Check action commitments & derive DAG
    let dag = derive_action_dag(&block.actions)
        .map_err(|_| BlockError::InvalidDag)?;
    if dag.root_hash() != block.action_dag_root {
        return Err(BlockError::InvalidDag);
    }
    let order = derive_execution_order(&dag);
    if hash(&order) != block.execution_order_hash {
        return Err(BlockError::InvalidOrder);
    }

    // 3. Execute actions in order
    let mut new_state = state.clone();
    let mut _receipts = Vec::new(); // In a real node, we would hash receipts
    for action in order {
        let receipt = execute_action(&mut new_state, &action)
            .map_err(|_| BlockError::ExecutionFailure)?;
        _receipts.push(receipt);
    }

    // 4. Verify post-state root
    if new_state.state_root() != block.state_root_post {
        return Err(BlockError::StateMismatch);
    }

    // 5. Apply slashing events
    apply_slashing(&mut new_state, &block.slashing_events)?;

    new_state.last_block = hash(block);
    return Ok(new_state);
}
