//! LP lock registry — basic rug-pull mitigation.
//!
//! Operators who seed a pool via [`crate::launchpad::Launchpad`] can
//! voluntarily lock their LP tokens until a future block height.  This
//! provides on-chain proof of commitment and prevents immediate liquidity
//! withdrawal after listing.
//!
//! This is an in-memory registry used by the CLI and devnet harness.  The
//! production on-chain variant lives in a pallet `StorageMap`.

use std::collections::BTreeMap;

/// Key: (owner, pool_id).
type LockKey = ([u8; 32], u64);

/// A single LP lock record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LpLock {
    pub owner: [u8; 32],
    pub pool_id: u64,
    pub lp_amount: u128,
    /// Block number at or after which the LP can be withdrawn.
    pub unlock_at_block: u64,
}

/// Errors from the anti-rug module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AntiRugError {
    /// Lock amount is zero.
    ZeroAmount,
    /// No lock exists for the given (owner, pool_id).
    NotFound,
    /// The lock has not yet expired.
    LockNotExpired,
    /// A lock already exists; use `extend` to update.
    AlreadyLocked,
}

/// In-memory LP lock registry.
#[derive(Default)]
pub struct LpLockRegistry {
    locks: BTreeMap<LockKey, LpLock>,
}

impl LpLockRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new LP lock.
    pub fn lock(
        &mut self,
        owner: [u8; 32],
        pool_id: u64,
        lp_amount: u128,
        unlock_at_block: u64,
    ) -> Result<(), AntiRugError> {
        if lp_amount == 0 {
            return Err(AntiRugError::ZeroAmount);
        }
        let key = (owner, pool_id);
        if self.locks.contains_key(&key) {
            return Err(AntiRugError::AlreadyLocked);
        }
        self.locks.insert(
            key,
            LpLock {
                owner,
                pool_id,
                lp_amount,
                unlock_at_block,
            },
        );
        Ok(())
    }

    /// Retrieve an existing lock.
    pub fn get(&self, owner: &[u8; 32], pool_id: u64) -> Option<&LpLock> {
        self.locks.get(&(*owner, pool_id))
    }

    /// Withdraw (remove) a lock once the unlock block has passed.
    ///
    /// `current_block` must be >= `lock.unlock_at_block`.
    pub fn withdraw(
        &mut self,
        owner: &[u8; 32],
        pool_id: u64,
        current_block: u64,
    ) -> Result<LpLock, AntiRugError> {
        let key = (*owner, pool_id);
        let lock = self.locks.get(&key).ok_or(AntiRugError::NotFound)?;
        if current_block < lock.unlock_at_block {
            return Err(AntiRugError::LockNotExpired);
        }
        Ok(self.locks.remove(&key).unwrap())
    }

    pub fn len(&self) -> usize {
        self.locks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.locks.is_empty()
    }
}
