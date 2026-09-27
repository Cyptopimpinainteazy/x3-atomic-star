use crate::{
    mock::*,
    ClaimedTaskCount, ResultCommits, TaskClaimSlots, TaskClaims, TaskKind, TaskStatus, Tasks,
};
use frame_support::{assert_noop, assert_ok, traits::Currency, BoundedVec};
use sp_core::H256;

fn register(executor: u64) {
    assert_ok!(NorthernSwarm::register_executor(
        RuntimeOrigin::signed(executor),
        100,
        hardware(),
    ));
}

fn submit_task(reward: u64) -> H256 {
    let payload: BoundedVec<u8, frame_support::traits::ConstU32<512>> =
        b"hex:01020304".to_vec().try_into().expect("bounded fixture");
    assert_ok!(NorthernSwarm::submit_task(
        RuntimeOrigin::signed(1),
        payload,
        reward,
        TaskKind::Compute,
    ));
    Tasks::<Test>::iter_keys()
        .next()
        .expect("submitted task has an id")
}

#[test]
fn multiple_executors_can_claim_one_task_up_to_the_bound() {
    new_test_ext().execute_with(|| {
        register(2);
        register(3);
        register(4);
        let task_id = submit_task(90);

        assert_ok!(NorthernSwarm::claim_task(RuntimeOrigin::signed(2), task_id));
        assert_ok!(NorthernSwarm::claim_task(RuntimeOrigin::signed(3), task_id));
        assert_ok!(NorthernSwarm::claim_task(RuntimeOrigin::signed(4), task_id));

        assert_eq!(TaskClaimSlots::<Test>::get(task_id), 3);
        assert!(TaskClaims::<Test>::contains_key(task_id, 2));
        assert!(TaskClaims::<Test>::contains_key(task_id, 3));
        assert!(TaskClaims::<Test>::contains_key(task_id, 4));

        assert_noop!(
            NorthernSwarm::claim_task(RuntimeOrigin::signed(5), task_id),
            crate::Error::<Test>::NotRegistered
        );
    });
}

#[test]
fn quorum_requires_matching_results() {
    new_test_ext().execute_with(|| {
        register(2);
        register(3);
        register(4);
        let task_id = submit_task(90);

        for executor in [2, 3, 4] {
            assert_ok!(NorthernSwarm::claim_task(
                RuntimeOrigin::signed(executor),
                task_id,
            ));
        }

        let hash_a = H256::repeat_byte(0xAA);
        let hash_b = H256::repeat_byte(0xBB);

        assert_ok!(NorthernSwarm::submit_result(
            RuntimeOrigin::signed(2),
            task_id,
            hash_a,
        ));
        assert_eq!(Tasks::<Test>::get(task_id).unwrap().status, TaskStatus::ResultCommitted);

        assert_ok!(NorthernSwarm::submit_result(
            RuntimeOrigin::signed(3),
            task_id,
            hash_b,
        ));
        assert_eq!(
            Tasks::<Test>::get(task_id).unwrap().status,
            TaskStatus::ResultCommitted,
            "two different hashes must not finalise a 2-of-3 task",
        );

        assert_ok!(NorthernSwarm::submit_result(
            RuntimeOrigin::signed(4),
            task_id,
            hash_a,
        ));
        let task = Tasks::<Test>::get(task_id).unwrap();
        assert_eq!(task.status, TaskStatus::Finalised);
        assert_eq!(task.result_hash, Some(hash_a));
        assert_eq!(ResultCommits::<Test>::get(task_id, 3), Some(hash_b));
    });
}

#[test]
fn task_reward_moves_reserved_balance_to_winner() {
    new_test_ext().execute_with(|| {
        register(2);
        register(3);
        let free_2_before = Balances::free_balance(2);
        let free_3_before = Balances::free_balance(3);
        let task_id = submit_task(90);

        assert_eq!(Balances::reserved_balance(1), 90);

        assert_ok!(NorthernSwarm::claim_task(RuntimeOrigin::signed(2), task_id));
        assert_ok!(NorthernSwarm::claim_task(RuntimeOrigin::signed(3), task_id));

        let hash = H256::repeat_byte(7);
        assert_ok!(NorthernSwarm::submit_result(
            RuntimeOrigin::signed(2),
            task_id,
            hash,
        ));
        assert_eq!(Balances::reserved_balance(1), 90);

        assert_ok!(NorthernSwarm::submit_result(
            RuntimeOrigin::signed(3),
            task_id,
            hash,
        ));

        assert_eq!(Balances::reserved_balance(1), 0);
        assert_eq!(Balances::free_balance(2), free_2_before + 45);
        assert_eq!(Balances::free_balance(3), free_3_before + 45);
        assert_eq!(ClaimedTaskCount::<Test>::get(2), 0);
        assert_eq!(ClaimedTaskCount::<Test>::get(3), 0);
    });
}

#[test]
fn task_reward_preserves_total_issuance() {
    new_test_ext().execute_with(|| {
        register(2);
        register(3);
        let issuance_before = Balances::total_issuance();
        let task_id = submit_task(91);

        assert_ok!(NorthernSwarm::claim_task(RuntimeOrigin::signed(2), task_id));
        assert_ok!(NorthernSwarm::claim_task(RuntimeOrigin::signed(3), task_id));

        let hash = H256::repeat_byte(9);
        assert_ok!(NorthernSwarm::submit_result(
            RuntimeOrigin::signed(2),
            task_id,
            hash,
        ));
        assert_ok!(NorthernSwarm::submit_result(
            RuntimeOrigin::signed(3),
            task_id,
            hash,
        ));

        // 91 / 2 pays 45 to each winner; the 1-unit remainder is unreserved
        // back to the submitter. No mint or burn is allowed.
        assert_eq!(Balances::total_issuance(), issuance_before);
        assert_eq!(Balances::reserved_balance(1), 0);
    });
}

#[test]
fn full_nonmatching_commit_set_enters_dispute_without_payout() {
    new_test_ext().execute_with(|| {
        register(2);
        register(3);
        register(4);
        let task_id = submit_task(90);
        let submitter_free_after_reserve = Balances::free_balance(1);

        for executor in [2, 3, 4] {
            assert_ok!(NorthernSwarm::claim_task(
                RuntimeOrigin::signed(executor),
                task_id,
            ));
        }

        for (executor, byte) in [(2, 1u8), (3, 2u8), (4, 3u8)] {
            assert_ok!(NorthernSwarm::submit_result(
                RuntimeOrigin::signed(executor),
                task_id,
                H256::repeat_byte(byte),
            ));
        }

        assert_eq!(Tasks::<Test>::get(task_id).unwrap().status, TaskStatus::Disputed);
        assert_eq!(Balances::reserved_balance(1), 90);
        assert_eq!(Balances::free_balance(1), submitter_free_after_reserve);
        assert_eq!(TaskClaimSlots::<Test>::get(task_id), 0);
    });
}
