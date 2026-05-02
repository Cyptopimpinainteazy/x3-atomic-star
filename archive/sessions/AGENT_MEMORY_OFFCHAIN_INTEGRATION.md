# AgentMemory Offchain Indexing Integration Guide

## Issue #6: AgentMemory Offchain Indexing Undocumented

**Priority:** 🟡 MEDIUM  
**Category:** Data Consistency  
**Severity:** Medium — Potential data loss or inconsistency if indexing not properly documented  

---

## Problem Statement

The X3 runtime includes `pallet_agent_memory` for storing autonomous agent execution state:

1. ❌ **No documented indexing requirements** — How should validators index agent memory?
2. ❌ **No consistency model defined** — What consistency guarantees are provided?
3. ❌ **No replication strategy** — How is agent state replicated across validators?
4. ❌ **No query API specified** — How do contracts query agent state?
5. ❌ **No data retention policy** — How long should memory snapshots be retained?

This creates:
- Potential data loss if memory not indexed offchain
- Confusion about validator responsibilities
- Inconsistency between on-chain and offchain state
- Query performance issues (direct pallet storage access inefficient)

---

## Solution Architecture

### 1. Data Classification

#### Tier 1: Public State (On-Chain)
- ✅ Agent metadata (name, owner, version)
- ✅ Public function signatures
- ✅ On-chain permissions and access control
- ✅ Agent lifecycle events (creation, upgrade, termination)
- **Storage:** `pallet_agent_memory::Agents` (FRAME storage)

#### Tier 2: Private State (Offchain Index)
- ❌ **NOT on-chain** — Too large for on-chain storage
- ✅ **Indexed offchain** — Validators maintain local copy
- ✅ **Merkle-root on-chain** — On-chain verification via merkle proof
- Agent execution context (memory variables)
- Execution traces and logs
- Sensitive computation state
- Local variable snapshots
- **Storage:** Validator RocksDB + offchain indexing DB

#### Tier 3: Archive State (Historical)
- Snapshots older than retention period
- Kept on archive nodes (optional)
- Queryable via archive RPC

---

### 2. On-Chain Storage Schema

```rust
// File: pallets/agent-memory/src/lib.rs

use frame_support::{
    pallet_prelude::*,
    storage::bounded_vec::BoundedVec,
};
use sp_core::H256;

#[pallet::config]
pub trait Config: frame_system::Config {
    type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    
    /// Maximum agent memory size (bytes)
    #[pallet::constant]
    type MaxMemorySize: Get<u32>;
    
    /// Memory snapshot retention (blocks)
    /// Default: 432k blocks ≈ 24 hours at 200ms/block
    #[pallet::constant]
    type MemoryRetentionBlocks: Get<u32>;
}

/// Agent metadata (public, on-chain)
#[derive(Debug, Clone, Encode, Decode)]
pub struct AgentMetadata<AccountId, BlockNumber> {
    pub agent_id: H256,
    pub owner: AccountId,
    pub name: BoundedVec<u8, ConstU32<64>>,
    pub version: u32,
    pub created_block: BlockNumber,
    pub last_updated: BlockNumber,
    pub memory_hash: H256,  // Merkle root of offchain memory
}

/// Memory snapshot metadata (on-chain)
#[derive(Debug, Clone, Encode, Decode)]
pub struct MemorySnapshot {
    pub agent_id: H256,
    pub block_number: u32,
    pub memory_hash: H256,          // Hash of full memory blob
    pub size_bytes: u32,
    pub indexed_at: u64,            // Unix timestamp
    pub validator_attestations: u32, // How many validators indexed this
}

#[pallet::storage]
pub type Agents<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    H256,  // agent_id
    AgentMetadata<T::AccountId, T::BlockNumber>,
    OptionQuery,
>;

#[pallet::storage]
pub type MemorySnapshots<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    H256,             // agent_id
    Blake2_128Concat,
    T::BlockNumber,   // block_number
    MemorySnapshot,
    OptionQuery,
>;

#[pallet::storage]
pub type LatestMemoryHash<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    H256,  // agent_id
    H256,  // latest memory merkle root
    ValueQuery,
>;
```

---

### 3. Offchain Indexing Schema

#### RocksDB Schema

```sql
-- Agent memory indexing (validators maintain locally)

-- Table 1: Memory snapshots
-- Key: agent_id:block_number
-- Value: full memory blob (serialized)
memory_snapshots {
    agent_id: [u8; 32],
    block_number: u32,
    data: Vec<u8>,
    size_bytes: u32,
    hash: [u8; 32],
    indexed_at: u64,
}

-- Table 2: Memory index (for quick queries)
-- Key: agent_id:block_number:function_name
-- Value: pointer to memory_snapshots + execution trace
memory_index {
    agent_id: [u8; 32],
    block_number: u32,
    function_name: Vec<u8>,
    input_params: Vec<u8>,
    output_value: Vec<u8>,
    execution_time_ms: u32,
    gas_used: u64,
}

-- Table 3: Consistency verification
-- Key: agent_id:block_number:validator_id
-- Value: verification status
memory_consistency {
    agent_id: [u8; 32],
    block_number: u32,
    validator_id: [u8; 32],
    verified: bool,
    hash_match: bool,
    indexed_at: u64,
}

-- Table 4: Memory retention log
-- Key: agent_id:retention_deadline
-- Value: blocks eligible for pruning
retention_log {
    agent_id: [u8; 32],
    block_number: u32,
    deadline_block: u32,
}
```

#### Index Columns

```sql
CREATE INDEX idx_agent_memory_latest 
    ON memory_snapshots (agent_id DESC, block_number DESC);
    -- Fast lookup: latest memory for agent

CREATE INDEX idx_agent_memory_time_window
    ON memory_snapshots (agent_id, block_number) 
    WHERE block_number > current_block - retention_blocks;
    -- Query: memory within retention window

CREATE INDEX idx_consistency_unverified
    ON memory_consistency (agent_id, verified DESC)
    WHERE verified = false;
    -- Find: unverified memory (needs consensus check)

CREATE INDEX idx_retention_expired
    ON retention_log (deadline_block ASC)
    WHERE deadline_block < current_block;
    -- Prune: memory older than retention
```

---

### 4. Offchain Worker Tasks

#### Background Task 1: Memory Indexing Worker

```rust
/// Offchain worker: Index agent memory snapshots
/// Called every block by pallet hooks
pub async fn index_memory_worker(block: BlockNumber) {
    // 1. Query chain for new MemoryUpdate events
    let updates = Self::fetch_memory_updates(block);
    
    for update in updates {
        // 2. Download full memory blob from peer
        if let Ok(memory_blob) = Self::fetch_memory_blob(&update.agent_id, block).await {
            // 3. Verify merkle root matches on-chain hash
            let hash = Self::hash_memory(&memory_blob);
            if hash != update.memory_hash {
                log::warn!("Memory hash mismatch for agent {:?}", update.agent_id);
                continue;
            }
            
            // 4. Store in local RocksDB
            Self::store_memory_snapshot(&update.agent_id, block, &memory_blob).await;
            
            // 5. Index for fast queries
            Self::index_memory(&update.agent_id, block, &memory_blob).await;
            
            // 6. Record successful indexing
            Self::record_indexing_success(&update.agent_id, block);
        }
    }
}

async fn fetch_memory_blob(agent_id: &H256, block: BlockNumber) -> Result<Vec<u8>, Error> {
    // Query other validators (via offchain HTTP or p2p)
    // Implement peer discovery and memory blob download
    // Timeout: 5 seconds per peer
    // Retry: 3 attempts across different peers
    
    // Return: full memory bytes if successfully downloaded
}

async fn store_memory_snapshot(
    agent_id: &H256,
    block: BlockNumber,
    memory_blob: &[u8],
) -> Result<(), Error> {
    // Write to local RocksDB
    let key = format!("memory:{}:{}", hex::encode(agent_id), block);
    sp_io::offchain_index::set(&key.into_bytes(), memory_blob);
    
    // Also write to persistent local DB (for offline queries)
    // Implementation depends on validator setup
}
```

#### Background Task 2: Consistency Verification

```rust
/// Verify memory consistency across validators
pub async fn verify_memory_consistency(block: BlockNumber) {
    let agents = Agents::<T>::iter_keys().collect::<Vec<_>>();
    
    for agent_id in agents {
        if let Some(snapshot) = MemorySnapshots::<T>::get(&agent_id, block) {
            // Query peer validators for same memory
            let peers = Self::get_peer_validators().await;
            let mut hash_votes: HashMap<H256, u32> = HashMap::new();
            
            for peer in peers {
                // Fetch their stored hash
                if let Ok(remote_hash) = Self::fetch_peer_memory_hash(&peer, &agent_id, block).await {
                    *hash_votes.entry(remote_hash).or_insert(0) += 1;
                }
            }
            
            // Check if consensus reached (> 2/3 validators)
            let total_validators = pallet_session::Validators::<T>::get().len() as u32;
            let quorum = (total_validators * 2) / 3;
            
            for (hash, votes) in hash_votes {
                if votes >= quorum {
                    // Consensus: memory hash is consistent
                    Self::record_consistency_verified(&agent_id, block, &hash);
                    break;
                }
            }
        }
    }
}

async fn fetch_peer_memory_hash(
    peer: &ValidatorId,
    agent_id: &H256,
    block: BlockNumber,
) -> Result<H256, Error> {
    // Call peer's RPC method: agent_memory_hash(agent_id, block)
    // Implementation uses offchain HTTP client
}
```

#### Background Task 3: Retention Cleanup

```rust
/// Prune old memory snapshots beyond retention period
pub fn cleanup_old_memory(current_block: BlockNumber) -> Weight {
    let retention = T::MemoryRetentionBlocks::get();
    let cutoff = current_block.saturating_sub(retention);
    
    let mut weight = Weight::zero();
    
    // Delete all snapshots older than cutoff
    for (agent_id, block) in MemorySnapshots::<T>::iter_keys() {
        if block < cutoff {
            MemorySnapshots::<T>::remove(&agent_id, &block);
            weight.saturating_accrue(T::DbWeight::get().write);
            
            log::debug!("Pruned memory snapshot for agent {:?} at block {}", agent_id, block);
        }
    }
    
    weight
}
```

---

### 5. Query API for RPC

#### New RPC Methods

```rust
// File: crates/x3-rpc/src/agent_memory.rs

/// Agent memory query API (exposed via RPC)
pub struct AgentMemoryRpc {
    client: Arc<FullClient>,
}

impl AgentMemoryRpc {
    /// Get latest memory hash for an agent
    pub fn agent_memory_hash(&self, agent_id: H256) -> RpcResult<H256> {
        let hash = LatestMemoryHash::<T>::get(&agent_id);
        Ok(hash)
    }
    
    /// Get memory snapshot at specific block
    pub fn agent_memory_at_block(
        &self,
        agent_id: H256,
        block_number: u32,
    ) -> RpcResult<Vec<u8>> {
        // Query local RocksDB for memory blob
        Self::query_memory_snapshot(&agent_id, block_number)
    }
    
    /// Execute readonly query on agent memory
    pub fn agent_query(
        &self,
        agent_id: H256,
        block_number: u32,
        function_name: String,
        params: Vec<u8>,
    ) -> RpcResult<Vec<u8>> {
        // Load memory snapshot
        let memory = Self::query_memory_snapshot(&agent_id, block_number)?;
        
        // Execute function (sandboxed WASM runtime)
        let result = self.execute_agent_function(&memory, &function_name, &params)?;
        
        Ok(result)
    }
    
    /// Verify memory consistency (returns validators in consensus)
    pub fn agent_memory_consensus(
        &self,
        agent_id: H256,
        block_number: u32,
    ) -> RpcResult<MemoryConsensusStatus> {
        // Query consistency verification results
        let attestations = Self::get_consistency_attestations(&agent_id, block_number);
        let total = pallet_session::Validators::<T>::get().len() as u32;
        
        Ok(MemoryConsensusStatus {
            attestations_received: attestations.len() as u32,
            attestations_required: (total * 2) / 3,
            consensus_reached: attestations.len() as u32 >= (total * 2) / 3,
        })
    }
}

#[derive(Encode, Decode, Clone, Debug)]
pub struct MemoryConsensusStatus {
    pub attestations_received: u32,
    pub attestations_required: u32,
    pub consensus_reached: bool,
}
```

#### RPC Endpoint Registration

```rust
// In node/src/rpc.rs

pub fn create_full<C, P>(
    client: Arc<FullClient<C>>,
    pool: Arc<TransactionPool<P>>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error>>
where
    // ...
{
    let mut io = RpcModule::new(());
    
    // ... existing RPC methods ...
    
    // Register AgentMemory RPC API
    let agent_memory = AgentMemoryRpc::new(client);
    io.merge(agent_memory.into_rpc())?;
    
    Ok(io)
}
```

---

### 6. Event Logging

```rust
#[pallet::event]
pub enum Event<T: Config> {
    /// Memory update submitted by agent
    MemoryUpdated {
        agent_id: H256,
        block: T::BlockNumber,
        new_hash: H256,
        size_bytes: u32,
    },
    
    /// Memory snapshot indexed by offchain worker
    MemoryIndexed {
        agent_id: H256,
        block: T::BlockNumber,
        validator: T::AccountId,
    },
    
    /// Memory consistency verified (quorum reached)
    MemoryConsensusReached {
        agent_id: H256,
        block: T::BlockNumber,
        attestations: u32,
        consensus_hash: H256,
    },
    
    /// Memory pruned (retention period expired)
    MemoryPruned {
        agent_id: H256,
        block: T::BlockNumber,
    },
}
```

---

## Consistency Model

### Eventual Consistency

**Guarantee:** All validators will eventually converge to same memory state within `MemoryRetentionBlocks` blocks.

**Process:**
1. Block N: Agent memory update → stored on-chain as merkle root
2. Block N+1 to N+5: Offchain workers fetch and index memory blob
3. Block N+100: Consistency verification task compares hashes across peers
4. Block N+101: If 2/3 validators match → consensus event emitted
5. Block N+432k: Memory eligible for pruning

### Strong Consistency (Optional)

For critical agent state, validators can enforce:
- Wait for 2/3 consensus before allowing on-chain execution
- Return "pending" for queries until consensus reached
- Fallback to old snapshot if new memory unavailable

```rust
pub fn query_memory_with_consensus(
    agent_id: H256,
    block: BlockNumber,
    require_consensus: bool,
) -> Result<Vec<u8>, Error> {
    if require_consensus {
        // Check if memory has reached quorum
        let attestations = Self::get_consistency_attestations(&agent_id, block).len();
        let quorum = (pallet_session::Validators::<T>::get().len() * 2) / 3;
        
        ensure!(attestations >= quorum, Error::<T>::NoConsensus);
    }
    
    // Return memory snapshot
    MemorySnapshots::<T>::get(&agent_id, block)
        .ok_or(Error::<T>::MemoryNotFound)
}
```

---

## Data Retention Policy

### Tier 1: Hot Data (0 to 7 days)
- **Retention:** 432k blocks (≈24 hours) on all validators
- **Access:** Full RPC query support
- **Cost:** Higher disk space
- **Use:** Active agent queries, recent execution traces

### Tier 2: Warm Data (7 to 30 days)
- **Retention:** Retained on archive nodes only
- **Access:** Via archive RPC or explicit query
- **Cost:** Lower (pruned from regular nodes)
- **Use:** Historical analysis, debugging

### Tier 3: Cold Data (> 30 days)
- **Retention:** Off-node archival (S3, etc.)
- **Access:** Batch download, not real-time
- **Cost:** Minimal on-node
- **Use:** Compliance, forensics

```rust
parameter_types! {
    /// Hot data: 432k blocks ≈ 24 hours
    pub const HotRetentionBlocks: u32 = 432_000;
    
    /// Archive retention: 2.6M blocks ≈ 150 days
    pub const ArchiveRetentionBlocks: u32 = 2_592_000;
}
```

---

## Validator Participation

### Mandatory Requirements
- ✅ Verify on-chain memory update events
- ✅ Maintain consistency verification state
- ✅ Respond to peer queries for memory hash (RPC)

### Optional Enhancements (Incentivized)
- ℹ️ Index and store full memory blobs
- ℹ️ Serve memory blobs to other validators
- ℹ️ Provide memory query execution (sandboxed WASM)
- ℹ️ Archive historical snapshots

### Operator Configuration

```toml
# config.toml

[agent_memory]
# Enable offchain indexing (default: true)
enable_indexing = true

# Enable memory blob serving (default: false)
serve_memory_blobs = true

# Storage path for memory snapshots
storage_path = "/var/lib/x3/agent-memory"

# Maximum memory snapshots per agent
max_snapshots_per_agent = 100

# Enable archive storage (default: false)
enable_archive_storage = true
archive_storage_path = "/var/lib/x3/agent-memory-archive"
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_memory_snapshot_storage() {
        // Store memory snapshot
        // Verify on-chain hash
        // Query via RPC
    }
    
    #[test]
    fn test_consistency_verification() {
        // Submit same memory from 3 validators
        // Verify consensus reached (2/3 quorum)
    }
    
    #[test]
    fn test_retention_cleanup() {
        // Create snapshots at block 100
        // Advance to block 100 + retention + 1
        // Verify old snapshots pruned
    }
    
    #[test]
    fn test_memory_query() {
        // Store memory snapshot
        // Query via RPC with function name
        // Verify result returned
    }
}
```

### Integration Tests

```bash
# Test offchain indexing with 3 validators
cargo test -p pallet-agent-memory --test offchain_indexing

# Test consistency verification
cargo test -p pallet-agent-memory --test consistency_verification

# Test retention cleanup
cargo test -p pallet-agent-memory --test retention_cleanup

# Benchmark memory query performance
cargo bench --bench agent_memory_queries
```

---

## Deployment Checklist

- [ ] Memory indexing tables created in RocksDB
- [ ] Offchain worker tasks registered in pallet hooks
- [ ] RPC API methods exposed and tested
- [ ] Consistency verification running on all validators
- [ ] Retention cleanup task scheduled
- [ ] Monitoring alerts configured (late indexing, missed consensus)
- [ ] Documentation updated for operators
- [ ] Archive storage configured (if enabled)

---

## Monitoring & Alerting

### Metrics
- `agent_memory_snapshots_total` — Total snapshots indexed
- `agent_memory_indexing_latency_blocks` — Delay between event and indexing
- `agent_memory_consistency_success_rate` — % reaching consensus
- `agent_memory_query_latency_ms` — RPC query response time

### Alerts
- 🚨 **Critical:** Indexing latency > 10 blocks
- ⚠️ **Warning:** Consistency rate < 70%
- ℹ️ **Info:** Memory snapshot pruned

---

## Related Issues & Dependencies

- ✅ Issue #1: GPU Sidecar (independent)
- ✅ Issue #2: CrossChainStateRootApi (independent)
- ✅ Issue #3: Pallet Ordering (independent)
- ✅ Issue #4: EVM Precompiles (independent)
- ✅ Issue #5: Settlement Timeout (independent)
- ❌ Issue #6: **This file** (AgentMemory Documentation)
- ✅ Issue #7: TX Pool Sizing (independent)

---

**Next Action:** Implement offchain worker tasks and RPC API following this architecture.

**Estimated Effort:** 6-8 hours (worker tasks + RPC + testing)

**Target Deadline:** Before testnet launch
