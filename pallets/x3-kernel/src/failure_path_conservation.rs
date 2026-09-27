//! Failure-path conservation: a refused comit must leave no trace of the writes it already made.
//!
//! X3-RT-004 asks for "supply conservation across all failure paths", and the pallet's answer used
//! to be a comment: *"In current Substrate architecture, returning error rolls back all storage"*.
//! That is true — `frame-support`'s pallet macro wraps every dispatchable body in
//! `frame_support::storage::with_storage_layer`
//! (`substrate/frame/support/procedural/src/pallet/expand/call.rs` in the SDK checkout), so an
//! `Err` return discards every write made by that call — but *true* is not *proven*, and
//! `submit_comit_v2`'s order of operations is exactly the shape where it matters:
//!
//! ```text
//! Nonces.increment                        writes
//! adapters execute                        external (adapters must be transactional themselves)
//! Currency::withdraw + burn the imbalance writes, and moves total issuance *down*
//! verify_triple_vm_with_receipts          REFUSAL 1: wrong prepare_root
//! X3ExecutionReceipts.insert              writes
//! SubmittedComits.insert                  writes
//! apply_canonical_ledger_update_v2        REFUSAL 2: ledger channel past MAX_STATE_CHANGES
//! apply_x3_storage_writes                 REFUSAL 3: slot channel past MAX_STATE_CHANGES
//! ```
//!
//! Refusal 3 is the one that carries the most already-written state: by the time it fires, the fee
//! has been burned and the canonical ledger has been updated for all three VMs. Every test below
//! drives its refusal through the *extrinsic* — never through the helper functions, whose own
//! refusals are covered elsewhere — and then asserts the fee, the issuance, the balances, the
//! canonical ledger, the X3 slot map, the nonce, the comit record, the stored receipt and the
//! event log are exactly where they started.
//!
//! Each refusal is paired with the *same* call succeeding, because "nothing changed" is also what
//! a test proves when the call never had an effect. The control half asserts that the writes
//! really happen: the fee is burned, and all three ledger legs land.
//!
//! Refusals 2 and 3 need a receipt that no honest adapter produces (one entry past the pallet's
//! bound), so they are driven by the `0xFE` / `0xFD` markers in `mock::TestX3Adapter`. Refusal 1
//! and the execution failures are driven by ordinary fixtures.

use frame_support::{assert_noop, assert_ok};
use sp_core::H256;

use crate::mock::{new_test_ext, Balances, RuntimeEvent, RuntimeOrigin, System, Test, ALICE};
use crate::test_helpers::{wrap_evm_payload, wrap_svm_payload, wrap_x3_payload};
use crate::{
    CanonicalLedger, DecodeFailureCount, Nonces, SubmittedComits, X3ContractStorage,
    X3ExecutionReceipts,
};

type AtlasKernel = crate::Pallet<Test>;
type AtlasError = crate::Error<Test>;

/// The X3 intent markers the mock adapter understands. `0x11` is deliberately not one of them: it
/// is the benign fixture the control half of each pair runs.
const BENIGN_INTENT: u8 = 0x11;
/// The mock's "execution error" marker (see `mock::TestX3Adapter`).
const X3_EXECUTION_FAILS: u8 = 0xFF;
/// The mock's "successful receipt, one ledger entry past the bound" marker.
const X3_LEDGER_CHANNEL_OVERFLOWS: u8 = 0xFE;
/// The mock's "successful receipt, one slot write past the bound" marker.
const X3_SLOT_CHANNEL_OVERFLOWS: u8 = 0xFD;

/// The ledger legs the three mock adapters write, by asset id. Distinct per VM on purpose: a test
/// that saw one value could not tell which domain wrote it.
const EVM_LEG: u32 = 0;
const SVM_LEG: u32 = 1;
const X3_LEG: u32 = 2;
const EVM_WRITES: u128 = 123;
const SVM_WRITES: u128 = 222;
const X3_WRITES: u128 = 333;

/// Everything a refused comit must not have moved.
///
/// Total issuance is read from `pallet_balances` rather than from the ledger because it is the side
/// of the conservation identity the fee withdrawal moves: `withdraw(.., FEE, ..)` produces a
/// `NegativeImbalance` that is dropped immediately, and dropping it burns the issuance.
#[derive(Debug, PartialEq, Eq)]
struct Conservation {
    total_issuance: u128,
    alice_free: u128,
    evm_leg: u128,
    svm_leg: u128,
    x3_leg: u128,
    x3_slots: usize,
    nonce: u64,
    comit_recorded: bool,
    receipt_stored: bool,
    decode_failures: u32,
    kernel_events: usize,
}

fn conservation(comit_id: H256) -> Conservation {
    Conservation {
        total_issuance: Balances::total_issuance(),
        alice_free: Balances::free_balance(ALICE),
        evm_leg: CanonicalLedger::<Test>::get(ALICE, EVM_LEG),
        svm_leg: CanonicalLedger::<Test>::get(ALICE, SVM_LEG),
        x3_leg: CanonicalLedger::<Test>::get(ALICE, X3_LEG),
        x3_slots: X3ContractStorage::<Test>::iter().count(),
        nonce: Nonces::<Test>::get(ALICE),
        comit_recorded: SubmittedComits::<Test>::contains_key(comit_id),
        receipt_stored: X3ExecutionReceipts::<Test>::contains_key(comit_id),
        decode_failures: DecodeFailureCount::<Test>::get(),
        kernel_events: System::events()
            .into_iter()
            .filter(|record| matches!(record.event, RuntimeEvent::AtlasKernel(_)))
            .count(),
    }
}

/// A comit with all three domains present, so every "did the earlier writes survive?" question has
/// a write to find. `x3_intent` is the X3 adapter's marker byte.
fn fixture(x3_intent: u8) -> (H256, Vec<u8>, Vec<u8>, Vec<u8>, u128) {
    (
        H256::from_low_u64_be(0x5151),
        wrap_evm_payload(&[0x01]),
        wrap_svm_payload(&[0x02]),
        wrap_x3_payload(&[x3_intent, 0x00, 0x00, 0x00]),
        1_000,
    )
}

fn submit_v2(
    comit_id: H256,
    evm_payload: Vec<u8>,
    svm_payload: Vec<u8>,
    x3_payload: Vec<u8>,
    fee: u128,
    prepare_root: H256,
) -> sp_runtime::DispatchResult {
    AtlasKernel::submit_comit_v2(
        RuntimeOrigin::signed(ALICE),
        comit_id,
        evm_payload,
        svm_payload,
        x3_payload,
        0,
        fee,
        prepare_root,
    )
}

fn root_for(comit_id: H256, evm: &[u8], svm: &[u8], x3: &[u8], fee: u128) -> H256 {
    AtlasKernel::compute_prepare_root_v2(comit_id, evm, svm, x3, 0, fee)
}

/// The control half: the same comit with a benign X3 intent must burn the fee and write all three
/// ledger legs. Without this, a refusal test that sees "nothing changed" proves only that the call
/// does nothing at all.
fn assert_the_same_call_succeeds_and_writes() {
    new_test_ext().execute_with(|| {
        let before = conservation(H256::zero());
        let (comit_id, evm, svm, x3, fee) = fixture(BENIGN_INTENT);
        let root = root_for(comit_id, &evm, &svm, &x3, fee);

        assert_ok!(submit_v2(comit_id, evm, svm, x3, fee, root));

        let after = conservation(comit_id);
        assert!(
            after.total_issuance < before.total_issuance,
            "control: the fee must be withdrawn and burned (issuance {} -> {})",
            before.total_issuance,
            after.total_issuance
        );
        assert!(
            after.alice_free < before.alice_free,
            "control: the caller must pay"
        );
        assert_eq!(
            (after.evm_leg, after.svm_leg, after.x3_leg),
            (EVM_WRITES, SVM_WRITES, X3_WRITES),
            "control: all three domains' ledger legs must land"
        );
        assert_eq!(after.nonce, before.nonce + 1, "control: the nonce advances");
        assert!(after.comit_recorded, "control: the comit is recorded");
        assert!(after.receipt_stored, "control: the X3 receipt is stored");
    });
}

/// Phase two of a pair: `marker` makes the X3 adapter produce a receipt the pallet refuses *after*
/// it has burned the fee and written the ledger, and the whole run must leave no trace.
fn assert_marker_is_refused_without_a_trace(marker: u8, expected: AtlasError) {
    new_test_ext().execute_with(|| {
        let before = conservation(H256::zero());
        let (comit_id, evm, svm, x3, fee) = fixture(marker);
        let root = root_for(comit_id, &evm, &svm, &x3, fee);

        assert_noop!(submit_v2(comit_id, evm, svm, x3, fee, root), expected);

        assert_eq!(
            conservation(comit_id),
            before,
            "a comit refused after the fee was burned and the ledger written must leave no trace"
        );
    });
}

#[test]
fn a_comit_that_passes_every_check_burns_the_fee_and_writes_all_three_ledger_legs() {
    assert_the_same_call_succeeds_and_writes();
}

/// Refusal 1: the prepare root is checked *after* the fee withdrawal, so a mismatch must give the
/// fee back rather than burn it.
#[test]
fn a_wrong_prepare_root_is_refused_after_the_fee_is_withdrawn_and_leaves_no_trace() {
    assert_the_same_call_succeeds_and_writes();

    new_test_ext().execute_with(|| {
        let before = conservation(H256::zero());
        let (comit_id, evm, svm, x3, fee) = fixture(BENIGN_INTENT);
        let wrong_root = H256::from_low_u64_be(0xDEAD_BEEF);
        assert_ne!(
            wrong_root,
            root_for(comit_id, &evm, &svm, &x3, fee),
            "the fixture's root must actually be wrong"
        );

        assert_noop!(
            submit_v2(comit_id, evm, svm, x3, fee, wrong_root),
            AtlasError::ComitVerificationFailed
        );

        assert_eq!(
            conservation(comit_id),
            before,
            "the fee is withdrawn before the root is checked, so refusing the root must roll it back"
        );
    });
}

/// A failure in the last of the three VMs. EVM and SVM have already returned usable receipts by
/// this point; the pallet must apply nothing from them.
#[test]
fn a_failure_in_the_last_vm_leaves_the_fee_and_the_earlier_receipts_unapplied() {
    assert_the_same_call_succeeds_and_writes();
    assert_marker_is_refused_without_a_trace(X3_EXECUTION_FAILS, AtlasError::X3ExecutionFailed);
}

/// Refusal 2: the ledger channel carries more entries than the pallet accepts. The check runs after
/// the fee is withdrawn; the fee must come back.
#[test]
fn an_over_wide_ledger_channel_is_refused_after_the_fee_is_withdrawn_and_rolls_it_back() {
    assert_the_same_call_succeeds_and_writes();
    assert_marker_is_refused_without_a_trace(
        X3_LEDGER_CHANNEL_OVERFLOWS,
        AtlasError::TooManyStateChanges,
    );
}

/// Refusal 3: the slot channel carries more writes than the pallet accepts, and the check runs
/// *after* `apply_canonical_ledger_update_v2` has already written all three ledger legs. This is
/// the strongest case in the file: the fee is burned, the comit is recorded, the X3 receipt is
/// stored and the canonical ledger is updated, and then the comit is refused. All of it must go.
#[test]
fn an_over_wide_slot_channel_is_refused_after_the_ledger_is_written_and_rolls_it_back() {
    assert_the_same_call_succeeds_and_writes();
    assert_marker_is_refused_without_a_trace(
        X3_SLOT_CHANNEL_OVERFLOWS,
        AtlasError::TooManyStorageWrites,
    );
}

/// The slot channel's refusal is also the one that guards `X3ContractStorage` itself: a receipt that
/// is refused must not leave the slots it named behind, and the control shows the same fixture is
/// accepted when it is one write smaller.
#[test]
fn a_refused_slot_channel_write_set_does_not_reach_chain_storage() {
    new_test_ext().execute_with(|| {
        let (comit_id, evm, svm, x3, fee) = fixture(X3_SLOT_CHANNEL_OVERFLOWS);
        let root = root_for(comit_id, &evm, &svm, &x3, fee);

        assert_noop!(
            submit_v2(comit_id, evm, svm, x3, fee, root),
            AtlasError::TooManyStorageWrites
        );

        assert_eq!(
            X3ContractStorage::<Test>::iter().count(),
            0,
            "the refused receipt's slots must not be readable from chain state"
        );
    });
}
