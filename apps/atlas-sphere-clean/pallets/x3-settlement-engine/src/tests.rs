#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::RuntimeOrigin;
    use crate::mock::{new_test_ext, Test, ALICE, BOB};
    use crate::{Bonds, BondsByOwner, Pallet};
    use frame_support::assert_ok;

    #[test]
    fn create_and_request_withdrawal() {
        let mut ext = new_test_ext();
        ext.execute_with(|| {
            // Create bond
            let id = Pallet::<Test>::create_bond_internal(&ALICE, b"ASSET".to_vec(), 500u128, 0)
                .unwrap();
            assert!(Bonds::<Test>::contains_key(id));
            let rec = Bonds::<Test>::get(id).expect("exists");
            assert_eq!(rec.state, 0);

            // Request withdrawal
            assert_ok!(Pallet::<Test>::request_withdrawal_internal(id));
            let rec2 = Bonds::<Test>::get(id).expect("exists");
            assert_eq!(rec2.state, 1);
        });
    }

    #[test]
    fn finalize_and_slash() {
        let mut ext = new_test_ext();
        ext.execute_with(|| {
            // Create and finalize withdraw
            let id = Pallet::<Test>::create_bond_internal(&ALICE, b"ASSET".to_vec(), 100u128, 0)
                .unwrap();
            assert_ok!(Pallet::<Test>::request_withdrawal_internal(id));
            assert_ok!(Pallet::<Test>::finalize_withdraw_internal(id));
            assert!(!Bonds::<Test>::contains_key(id));
            let list = BondsByOwner::<Test>::get(ALICE);
            assert!(!list.iter().any(|x| *x == id));

            // Create and slash
            let id2 =
                Pallet::<Test>::create_bond_internal(&BOB, b"B".to_vec(), 200u128, 0).unwrap();
            assert_ok!(Pallet::<Test>::slash_bond_internal(id2));
            let rec = Bonds::<Test>::get(id2).expect("exists");
            assert_eq!(rec.state, 2);
        });
    }

    #[test]
    fn extrinsic_flow() {
        let mut ext = new_test_ext();
        ext.execute_with(|| {
            // Deposit bond via extrinsic
            assert_ok!(Pallet::<Test>::deposit_bond(
                RuntimeOrigin::signed(ALICE),
                b"ASSET".to_vec(),
                100u128,
                0
            ));

            // There should be a bond for ALICE
            let list = BondsByOwner::<Test>::get(ALICE);
            assert_eq!(list.len(), 1);
            let id = list[0];

            // Request withdraw via extrinsic
            assert_ok!(Pallet::<Test>::request_bond_withdraw(
                RuntimeOrigin::signed(ALICE),
                id
            ));
            let rec = Bonds::<Test>::get(id).expect("exists");
            assert_eq!(rec.state, 1);

            // Finalize withdraw via extrinsic
            assert_ok!(Pallet::<Test>::finalize_bond_withdraw(
                RuntimeOrigin::signed(ALICE),
                id
            ));
            assert!(!Bonds::<Test>::contains_key(id));
        });
    }
}
