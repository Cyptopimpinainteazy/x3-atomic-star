//! Failure reproduction: can a swap that already completed be refunded?
//!
//! `SwapCoordinator::abort` sets `phase = Aborting` without matching on the
//! current phase, while every other mutator goes through
//! `validate_phase_transition`. If a completed swap can be walked back to
//! `Aborting` and then to `Refunded`, both chains paid out and then refunded:
//! the exact double-spend atomic swaps exist to prevent.
//!
//! This test drives the real coordinator along the happy path to `Complete` and
//! then asserts that the swap is no longer refundable.

use std::sync::Arc;

use x3_cross_vm_coordinator::{
    CoordinatorConfig, HtlcCreateParams, HtlcId, HtlcRecord, HtlcSecret, HtlcStatus,
    InMemoryPersistence, SessionPersistence, SwapCoordinator, SwapPhase, SwapSession, VmTarget,
};

const NOW: u64 = 1_700_000_000;
const FAST_TIMELOCK: u64 = 3_600;
const SLOW_TIMELOCK: u64 = 7_200;

fn session(id: &str, secret: &HtlcSecret) -> SwapSession {
    SwapSession {
        session_id: id.to_string(),
        hash_lock: secret.hash(),
        htlc_fast: None,
        htlc_slow: None,
        flash_legs: Vec::new(),
        leg_outcomes: Vec::new(),
        phase: SwapPhase::Setup,
        timelock_fast: NOW + FAST_TIMELOCK,
        timelock_slow: NOW + SLOW_TIMELOCK,
        created_at: NOW,
        updated_at: NOW,
        operation_journal: Vec::new(),
        requires_merkle_verification: false,
    }
}

fn record(secret: &HtlcSecret, fast: bool) -> HtlcRecord {
    let (vm, timelock) = if fast {
        (VmTarget::X3Vm, NOW + FAST_TIMELOCK)
    } else {
        (VmTarget::Svm, NOW + SLOW_TIMELOCK)
    };
    HtlcRecord {
        id: HtlcId(vec![if fast { 1 } else { 2 }]),
        params: HtlcCreateParams {
            vm,
            recipient: vec![0x11; 32],
            hash_lock: secret.hash(),
            timelock,
            asset: vec![0xAA; 32],
            amount: 1_000,
        },
        status: HtlcStatus::Funded,
        created_at_block: 100,
        confirmations_required: 1,
        confirmations: 1,
        params_hash: [if fast { 1 } else { 2 }; 32],
    }
}

/// Drive the real coordinator to `Complete` and return it with its persistence.
fn completed_swap() -> (SwapCoordinator<InMemoryPersistence>, String, HtlcSecret) {
    let secret = HtlcSecret([0x61; 32]);
    let id = "sim-refund-after-claim".to_string();
    let persistence = Arc::new(InMemoryPersistence::new());
    persistence.save(&session(&id, &secret));
    let mut coord =
        SwapCoordinator::with_persistence(CoordinatorConfig::default(), persistence.clone());

    coord
        .record_htlc_fast(&id, record(&secret, true), NOW + 1)
        .expect("fast lock");
    coord
        .record_htlc_slow(&id, record(&secret, false), NOW + 2)
        .expect("slow lock");
    coord
        .begin_flash_execution(&id, NOW + 3)
        .expect("begin flash");
    coord.begin_settlement(&id, NOW + 4).expect("settle");
    coord
        .record_fast_claim(&id, secret.clone(), NOW + 5)
        .expect("fast claim");
    coord.record_slow_claim(&id, NOW + 6).expect("slow claim");

    assert_eq!(
        coord.get_session(&id).map(|s| s.phase),
        Some(SwapPhase::Complete),
        "precondition: the swap must be Complete before the refund attempt"
    );
    (coord, id, secret)
}

#[test]
fn completed_swap_cannot_be_aborted() {
    let (mut coord, id, _secret) = completed_swap();
    let result = coord.abort(&id, "post-completion abort", NOW + 7);
    assert!(
        result.is_err(),
        "abort() must refuse a swap that already completed; it accepted the abort and \
         moved a COMPLETE swap back to {:?}",
        coord.get_session(&id).map(|s| s.phase)
    );
}

#[test]
fn completed_swap_cannot_be_refunded() {
    let (mut coord, id, _secret) = completed_swap();

    // Attempt the full post-completion walk-back.
    let _ = coord.abort(&id, "post-completion abort", NOW + 7);
    let refund = coord.record_refunds(&id, NOW + 8);

    let phase = coord.get_session(&id).map(|s| s.phase);
    assert!(
        refund.is_err(),
        "a swap whose two legs were both CLAIMED must never reach Refunded; \
         record_refunds returned Ok and left the session at {phase:?}"
    );
}

/// Positive control: the new guard must not block a legitimate refund.
#[test]
fn an_active_swap_can_still_be_aborted_and_refunded() {
    let secret = HtlcSecret([0x62; 32]);
    let id = "sim-legit-refund".to_string();
    let persistence = Arc::new(InMemoryPersistence::new());
    persistence.save(&session(&id, &secret));
    let mut coord =
        SwapCoordinator::with_persistence(CoordinatorConfig::default(), persistence.clone());

    coord
        .abort(&id, "operator abort", NOW + 1)
        .expect("aborting a Setup-phase swap is the normal refund path");
    coord
        .record_refunds(&id, NOW + 2)
        .expect("a refund after a real abort must still work");

    assert_eq!(
        coord.get_session(&id).map(|s| s.phase),
        Some(SwapPhase::Refunded)
    );
}
