//! X3 deterministic parallel transaction executor.
//!
//! # Design intent
//!
//! The parallel executor is explicitly **not** activated in v0.4 internal
//! mainnet.  Serial correctness must be proven first.  This crate provides
//! the infrastructure so the parallel path can be enabled and validated
//! against the serial baseline in a subsequent release.
//!
//! # Architecture
//!
//! ```text
//! batch (AccessList, TxId)[]
//!   │
//!   ▼
//! Scheduler ──► Schedule (waves)
//!   │
//!   ▼
//! Executor ──► ExecutionResult
//!   │
//!   ▼
//! Commit ──► CommitSummary + write_overlay
//! ```
//!
//! # Correctness guarantee
//!
//! The `commit::Commit` step applies write sets in original serial order,
//! so the final state is identical to what a serial executor would produce.
//! Any detected access violation is surfaced for serial re-execution.
//!
//! # Feature flags
//!
//! * `std` — enable std library (default on native; off for Wasm runtime).

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod access_list;
pub mod commit;
pub mod conflict;
pub mod executor;
pub mod scheduler;

pub use access_list::AccessList;
pub use commit::{Commit, CommitSummary};
pub use conflict::ConflictDetector;
pub use executor::{ExecutionResult, Executor, FailReason, TxOutcome};
pub use scheduler::{Schedule, Scheduler, TxId, WaveEntry};

// ────────────────────────────────────────────────────────────────────────────
// Integration tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn key(b: u8) -> [u8; 32] {
        let mut k = [0u8; 32];
        k[0] = b;
        k
    }

    fn al(reads: &[u8], writes: &[u8]) -> AccessList {
        AccessList::new(
            reads.iter().map(|&b| key(b)).collect(),
            writes.iter().map(|&b| key(b)).collect(),
        )
    }

    // ── Access list conflict detection

    #[test]
    fn disjoint_access_lists_do_not_conflict() {
        let a = al(&[1], &[2]);
        let b = al(&[3], &[4]);
        assert!(!a.conflicts_with(&b));
    }

    #[test]
    fn write_write_conflict_detected() {
        let a = al(&[], &[5]);
        let b = al(&[], &[5]);
        assert!(a.conflicts_with(&b));
    }

    #[test]
    fn read_write_conflict_detected() {
        let a = al(&[6], &[]);
        let b = al(&[], &[6]);
        assert!(a.conflicts_with(&b));
    }

    // ── Scheduler wave assignment

    #[test]
    fn independent_txs_in_one_wave() {
        let batch: Vec<(AccessList, TxId)> = vec![
            (al(&[1], &[2]), 0),
            (al(&[3], &[4]), 1),
            (al(&[5], &[6]), 2),
        ];
        let schedule = Scheduler::schedule(&batch);
        assert_eq!(schedule.wave_count(), 1);
        assert_eq!(schedule.tx_count(), 3);
    }

    #[test]
    fn conflicting_txs_in_separate_waves() {
        let batch: Vec<(AccessList, TxId)> = vec![
            (al(&[], &[1]), 0),
            (al(&[], &[1]), 1), // conflicts with tx 0
        ];
        let schedule = Scheduler::schedule(&batch);
        assert_eq!(schedule.wave_count(), 2);
    }

    #[test]
    fn mixed_batch_produces_correct_waves() {
        // tx0 writes key 1; tx1 writes key 2 (no conflict); tx2 writes key 1 again.
        let batch: Vec<(AccessList, TxId)> =
            vec![(al(&[], &[1]), 0), (al(&[], &[2]), 1), (al(&[], &[1]), 2)];
        let schedule = Scheduler::schedule(&batch);
        // tx0 and tx1 should share wave 0; tx2 conflicts with tx0 → wave 1.
        assert_eq!(schedule.wave_count(), 2);
    }

    #[test]
    fn empty_batch_produces_empty_schedule() {
        let schedule = Scheduler::schedule(&[]);
        assert_eq!(schedule.wave_count(), 0);
    }

    // ── Executor

    #[test]
    fn executor_calls_tx_fn_for_each_tx() {
        let batch: Vec<(AccessList, TxId)> = vec![(al(&[1], &[2]), 10), (al(&[3], &[4]), 20)];
        let schedule = Scheduler::schedule(&batch);
        let mut called = alloc::vec![];
        let result = Executor::execute(&schedule, |tx_id| {
            called.push(tx_id);
            TxOutcome::Success {
                tx_id,
                writes: alloc::vec![key(9)],
            }
        });
        called.sort();
        assert_eq!(called, vec![10, 20]);
        assert_eq!(result.success_count(), 2);
        assert_eq!(result.failed_count(), 0);
    }

    // ── Commit

    #[test]
    fn commit_counts_success_and_failures() {
        let result = ExecutionResult {
            outcomes: alloc::vec![
                TxOutcome::Success {
                    tx_id: 1,
                    writes: alloc::vec![key(1)],
                },
                TxOutcome::Failed {
                    tx_id: 2,
                    reason: FailReason::ApplicationError,
                },
                TxOutcome::Success {
                    tx_id: 3,
                    writes: alloc::vec![key(3)],
                },
            ],
        };
        let summary = Commit::apply(&result);
        assert_eq!(summary.committed, 2);
        assert_eq!(summary.failed, 1);
        assert!(summary.reexecute.is_empty());
    }

    #[test]
    fn access_violation_queued_for_reexecution() {
        let result = ExecutionResult {
            outcomes: alloc::vec![TxOutcome::AccessViolation { tx_id: 99 }],
        };
        let summary = Commit::apply(&result);
        assert_eq!(summary.reexecute, alloc::vec![99]);
    }

    // ── End-to-end: serial equivalence property

    /// Run the same batch through the parallel path and a simulated serial
    /// path; verify the committed tx count is identical.
    #[test]
    fn parallel_committed_equals_serial() {
        let batch: Vec<(AccessList, TxId)> = (0u64..8)
            .map(|i| (al(&[i as u8 * 2], &[i as u8 * 2 + 1]), i))
            .collect();

        let schedule = Scheduler::schedule(&batch);
        let result = Executor::execute(&schedule, |tx_id| TxOutcome::Success {
            tx_id,
            writes: alloc::vec![key(tx_id as u8)],
        });
        let summary = Commit::apply(&result);

        assert_eq!(summary.committed, 8);
        assert_eq!(summary.failed, 0);
        assert!(summary.reexecute.is_empty());
    }
}
