//! The X3VM slot channel, end to end: a program's store reaches chain storage.
//!
//! Before this channel existed, `crates/x3-vm` journaled every `evm_sstore`
//! (`VM::drain_storage_journal`) and nothing on a chain called it. `X3Executor::execute` returned
//! `state_changes: vec![]`, and the kernel's receipt field of that name is *balance-shaped* — its
//! decoder reads each entry as (address -> account, key -> asset id, value -> balance) — so a slot
//! write pushed through it would not be a slot write at all.
//!
//! These tests drive the production adapter (`X3VmAdapter`, the one a `std` chain configures) with
//! a real X3BC module whose `main` performs an `evm_sstore`, submit it through the real
//! `submit_comit_v2` extrinsic, and read the slot back from `X3ContractStorage`. The module is
//! assembled with the envelope writer the compiler itself uses, so its checksum is real; the `.x3`
//! source compiler cannot emit `evm_sstore` yet, which is why the fixture is hand-assembled rather
//! than compiled.
//!
//! The runtime here is local to this file and configures the *real* X3 adapter, unlike the pallet's
//! own mock runtime (`mock.rs`), which uses `TestX3Adapter`. That is the point: a mock can show the
//! pallet applies a receipt it is handed, and only a real adapter can show the receipt a chain
//! actually produces.
#![cfg(feature = "std")]

use frame_support::{
    construct_runtime, parameter_types,
    traits::{ConstBool, ConstU32, ConstU64, Get},
};
use frame_system as system;
use parity_scale_codec::Decode;
use sp_core::{H160, H256};
use sp_io::TestExternalities;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage, DispatchError,
};

use pallet_x3_kernel::{MockEvmAdapter, MockSvmAdapter, X3ExecutorAdapter, X3VmAdapter};
use x3_backend::{BytecodeModule, FunctionEntry};

type AccountId = u64;
type Balance = u128;
type Block = system::mocking::MockBlock<StorageTest>;

construct_runtime!(
    pub enum StorageTest {
        System: frame_system,
        Timestamp: pallet_timestamp,
        Balances: pallet_balances,
        Kernel: pallet_x3_kernel,
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const ExistentialDeposit: Balance = 1;
    pub const MinimumPeriod: u64 = 6000;
}

impl system::Config for StorageTest {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type Nonce = u64;
    type Block = Block;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<AccountId>;
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type ExtensionsWeightInfo = ();
    type RuntimeTask = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
    type MaxConsumers = ConstU32<16>;
}

impl pallet_timestamp::Config for StorageTest {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl pallet_balances::Config for StorageTest {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ConstU32<50>;
    type ReserveIdentifier = [u8; 8];
    type RuntimeHoldReason = ();
    type FreezeIdentifier = ();
    type RuntimeFreezeReason = ();
    type MaxFreezes = ConstU32<0>;
    type DoneSlashHandler = ();
}

pub struct TestEmergencyHaltController;
impl pallet_x3_kernel::EmergencyHaltController for TestEmergencyHaltController {
    fn trigger() {}
}

pub struct TestProofVerifier;
impl pallet_x3_kernel::CrossChainProofVerifier<AccountId> for TestProofVerifier {
    fn verify_proof(
        _origin: &AccountId,
        _operation: &x3_cross_vm_bridge::CrossVmOperation,
        _proof: &pallet_x3_kernel::CrossChainProof,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
}

pub struct TestBridgeEvmEscrow;
impl Get<H160> for TestBridgeEvmEscrow {
    fn get() -> H160 {
        H160::repeat_byte(0xE3)
    }
}

pub struct TestBridgeSvmEscrow;
impl Get<[u8; 32]> for TestBridgeSvmEscrow {
    fn get() -> [u8; 32] {
        [0x53; 32]
    }
}

impl pallet_x3_kernel::Config for StorageTest {
    type Currency = Balances;
    type Balance = Balance;
    type AssetId = u32;
    type AtlasId = u32;
    type MaxAssetsPerAccount = ConstU32<16>;
    type MaxAssetSymbolLength = ConstU32<16>;
    type MaxEvmPayloadLength = ConstU32<4096>;
    type MaxSvmPayloadLength = ConstU32<4096>;
    type MaxX3PayloadLength = ConstU32<4096>;
    type MaxCombinedPayloadLength = ConstU32<8192>;
    type MaxCombinedPayloadLengthV2 = ConstU32<12_288>;
    type MaxAuthorities = ConstU32<100>;
    type MinAuthorities = ConstU32<1>;
    type DefaultEvmGasLimit = ConstU64<10_000_000>;
    type DefaultSvmComputeLimit = ConstU64<200_000>;
    type DefaultX3GasLimit = ConstU64<5_000_000>;
    type WeightInfo = ();
    // These two arms are the kernel's own non-fabricating mocks; they are only reached if a test
    // passes a payload to them (the slot tests below pass none).
    type EvmAdapter = MockEvmAdapter;
    type SvmAdapter = MockSvmAdapter;
    // The production X3 adapter: the component under test.
    type X3Adapter = X3VmAdapter;
    type GovernanceOrigin = frame_system::EnsureRoot<AccountId>;
    type CrossVmPrepareTtl = ConstU64<10>;
    type MaxPreparedCrossVmOps = ConstU32<16>;
    type MaxPreparedOpsPerBlock = ConstU32<8>;
    type MaxReplayPruneItemsPerBlock = ConstU32<64>;
    type RequireCrossVmProof = ConstBool<false>;
    type CrossChainProofVerifier = TestProofVerifier;
    type BridgeEvmEscrow = TestBridgeEvmEscrow;
    type BridgeSvmEscrow = TestBridgeSvmEscrow;
    type EmergencyHaltController = TestEmergencyHaltController;
}

const ALICE: AccountId = 1;
const INITIAL_BALANCE: Balance = 1_000_000_000_000;

fn new_test_ext() -> TestExternalities {
    let mut storage = frame_system::GenesisConfig::<StorageTest>::default()
        .build_storage()
        .expect("system genesis");
    pallet_balances::GenesisConfig::<StorageTest> {
        balances: vec![(ALICE, INITIAL_BALANCE)],
        dev_accounts: None,
    }
    .assimilate_storage(&mut storage)
    .expect("balances genesis");
    pallet_x3_kernel::GenesisConfig::<StorageTest> {
        assets: Vec::new(),
        authorities: vec![ALICE],
        authorized_accounts: vec![ALICE],
        evm_escrow: H160::repeat_byte(0xE3),
        svm_escrow: [0x53; 32],
    }
    .assimilate_storage(&mut storage)
    .expect("kernel genesis");
    let mut t = TestExternalities::new(storage);
    t.execute_with(|| {
        System::set_block_number(1);
        Timestamp::set_timestamp(12_000);
    });
    t
}

fn module_with_code(code: Vec<u8>) -> Vec<u8> {
    let mut module = BytecodeModule::new();
    module.functions.push(FunctionEntry {
        name: "main".to_string(),
        entry_point: 0,
        param_count: 0,
        local_count: 8,
        max_stack: 8,
        return_type_tag: 1,
    });
    module.code = code;
    module.to_bytes()
}

/// `main() { sstore(slot: 0, value: 7); return; }`, written through the real envelope writer.
fn store_seven_in_slot_zero() -> Vec<u8> {
    module_with_code(vec![
        0x18, 0x01, 0x00, // LoadImm r1, 0  (slot)
        0x18, 0x02, 0x07, // LoadImm r2, 7  (value)
        0xB4, 0x01, 0x02, // EvmSstore slot=r1 val=r2
        0x06, // RetVoid
    ])
}

/// `main() { atomic_begin(0); sstore(slot: 0, value: 9); atomic_rollback(0); return; }`
///
/// The store is inside a window that rolls back, so it must survive nowhere: the VM aborts the call
/// (`AtomicAborted`), the executor reports an unsuccessful receipt carrying no writes, and the
/// kernel refuses the comit.
fn store_inside_a_rolled_back_window() -> Vec<u8> {
    module_with_code(vec![
        0x18, 0x01, 0x00, // LoadImm r1, 0 (slot)
        0x18, 0x02, 0x09, // LoadImm r2, 9 (value)
        0x90, 0x00, 0x00, // AtomicBegin id=0
        0xB4, 0x01, 0x02, // EvmSstore slot=r1 val=r2
        0x92, 0x00, 0x00, // AtomicRollback id=0
        0x06, // RetVoid
    ])
}

/// `main() { sstore(slot: 0, value: 7); 1 / 0; }`
///
/// The store happens *before* the fault and outside any window, so the VM journaled it. The call
/// still fails, and a failed call's partial writes must never be reported: this is the receipt-level
/// half of the fail-closed rule whose chain-level half is `a_reverted_atomic_window_writes_no_chain_state`.
fn store_then_divide_by_zero() -> Vec<u8> {
    module_with_code(vec![
        0x18, 0x01, 0x00, // LoadImm r1, 0 (slot)
        0x18, 0x02, 0x07, // LoadImm r2, 7 (value)
        0xB4, 0x01, 0x02, // EvmSstore slot=r1 val=r2 — journaled, no window
        0x18, 0x03, 0x01, // LoadImm r3, 1
        0x18, 0x04, 0x00, // LoadImm r4, 0
        0x23, 0x05, 0x03, 0x04, // DivI dst=r5 a=r3 b=r4 -> DivisionByZero
        0x06, // RetVoid
    ])
}

/// The slot key the VM derives for slot `n`: `"X3EVM_SL"` little-endian, then the slot number.
///
/// Recomputed here from the layout rather than read back from the VM, so the test fails if the key
/// derivation drifts.
const EVM_SLOT_DOMAIN: u64 = 0x5833_4556_4D5F_534C; // "X3EVM_SL"

fn evm_slot_key(slot: u64) -> H256 {
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&EVM_SLOT_DOMAIN.to_le_bytes());
    key[8..16].copy_from_slice(&slot.to_le_bytes());
    H256::from(key)
}

/// The exact 32-byte payload the VM writes for `Value::I64(7)`: tag 1 (int), length 8, LE digits.
fn encoded_i64_payload(value: i64) -> [u8; 32] {
    let mut payload = [0u8; 32];
    payload[0] = 1;
    payload[1] = 8;
    payload[2..10].copy_from_slice(&value.to_le_bytes());
    payload
}

fn submit_v2(
    comit_id: H256,
    evm_payload: Vec<u8>,
    x3_payload: Vec<u8>,
) -> Result<(), DispatchError> {
    let nonce = 0u64;
    let fee: Balance = 1_000;
    let prepare_root =
        Kernel::compute_prepare_root_v2(comit_id, &evm_payload, &[], &x3_payload, nonce, fee);
    Kernel::submit_comit_v2(
        RuntimeOrigin::signed(ALICE),
        comit_id,
        evm_payload,
        Vec::new(),
        x3_payload,
        nonce,
        fee,
        prepare_root,
    )
}

#[test]
fn a_store_reaches_chain_storage_through_the_receipt() {
    new_test_ext().execute_with(|| {
        let comit_id = H256::from_low_u64_be(0x5017);
        let payload = store_seven_in_slot_zero();

        // Sanity: the production adapter really executes this module and really reports the write.
        let direct = X3VmAdapter::execute(&payload, 5_000_000).expect("the adapter executes it");
        assert!(direct.success);
        assert_eq!(
            direct.storage_writes.len(),
            1,
            "one store must produce exactly one typed slot write"
        );
        assert_eq!(direct.storage_writes[0].key, evm_slot_key(0));
        assert_eq!(direct.storage_writes[0].old_value, None);
        assert_eq!(
            direct.storage_writes[0].new_value,
            Some(encoded_i64_payload(7))
        );

        // The slot is not on the chain before the comit runs.
        assert_eq!(Kernel::x3_contract_slot(evm_slot_key(0)), None);

        submit_v2(comit_id, Vec::new(), payload).expect("the comit must be accepted");

        // The store is visible in chain storage, keyed by the slot the program asked for.
        assert_eq!(
            Kernel::x3_contract_slot(evm_slot_key(0)),
            Some(encoded_i64_payload(7)),
            "the stored value must be readable from chain storage"
        );

        // The receipt the chain kept carries the same write, so the state change is auditable from
        // the receipt and not only from the storage map.
        let receipt = Kernel::x3_execution_receipt(comit_id).expect("the receipt is stored");
        assert_eq!(receipt.version, pallet_x3_kernel::EXECUTION_RECEIPT_VERSION);
        assert_eq!(receipt.storage_writes.len(), 1);
        assert_eq!(receipt.storage_writes[0].key, evm_slot_key(0));
        assert_eq!(
            receipt.storage_writes[0].new_value,
            Some(encoded_i64_payload(7))
        );

        // A slot write is not a balance change: the kernel must not have tried to decode it as one.
        assert_eq!(
            pallet_x3_kernel::DecodeFailureCount::<StorageTest>::get(),
            0,
            "a storage-writing comit must not be decoded as a balance change"
        );
    });
}

#[test]
fn a_reverted_atomic_window_writes_no_chain_state() {
    new_test_ext().execute_with(|| {
        let comit_id = H256::from_low_u64_be(0x5018);
        let payload = store_inside_a_rolled_back_window();

        // The adapter reports the failure as a receipt, not as a panic, and carries no writes.
        let direct =
            X3VmAdapter::execute(&payload, 5_000_000).expect("the adapter returns a receipt");
        assert!(!direct.success, "a rolled-back window aborts the call");
        assert!(
            direct.storage_writes.is_empty(),
            "a reverted window must report no slot writes"
        );

        // The kernel refuses the comit, and no slot is created.
        assert!(submit_v2(comit_id, Vec::new(), payload).is_err());
        assert_eq!(
            Kernel::x3_contract_slot(evm_slot_key(0)),
            None,
            "a reverted write must not reach chain storage"
        );
        assert!(Kernel::x3_execution_receipt(comit_id).is_none());
        assert_eq!(
            pallet_x3_kernel::DecodeFailureCount::<StorageTest>::get(),
            0
        );
    });
}

#[test]
fn a_failed_execution_does_not_report_the_writes_it_journaled() {
    new_test_ext().execute_with(|| {
        let comit_id = H256::from_low_u64_be(0x501A);
        let payload = store_then_divide_by_zero();

        // The store really was journaled before the fault — otherwise this test would be vacuous.
        // `X3VmAdapter` hides the journal behind the receipt, so the receipt is the evidence: it
        // must report the failure, and it must report *no* writes even though a write happened.
        let direct =
            X3VmAdapter::execute(&payload, 5_000_000).expect("the adapter returns a receipt");
        assert!(!direct.success, "a divide by zero fails the call");
        assert!(
            direct.storage_writes.is_empty(),
            "a failed execution must report no slot writes, however many it journaled"
        );

        // And the chain refuses the comit, so nothing reaches storage.
        assert!(submit_v2(comit_id, Vec::new(), payload).is_err());
        assert_eq!(Kernel::x3_contract_slot(evm_slot_key(0)), None);
        assert!(Kernel::x3_execution_receipt(comit_id).is_none());
        assert_eq!(
            pallet_x3_kernel::DecodeFailureCount::<StorageTest>::get(),
            0
        );
    });
}

/// The control for the tests above: the *balance* channel still decodes exactly as it did.
///
/// Without this, `a_store_reaches_chain_storage_through_the_receipt` could pass on an implementation
/// that simply stopped decoding anything into the balance ledger.
#[test]
fn a_balance_shaped_change_still_reaches_the_ledger() {
    new_test_ext().execute_with(|| {
        let comit_id = H256::from_low_u64_be(0x5019);
        let evm_payload = b"balance-shaped".to_vec();

        // The kernel's mock EVM adapter keys its deterministic change off `blake2_256(payload)`.
        let state_root = sp_io::hashing::blake2_256(&evm_payload);
        let expected_asset = u32::decode(&mut &state_root[..]).expect("asset id decodes");
        let expected_balance = u128::decode(&mut &state_root[..]).expect("balance decodes");

        let before = pallet_x3_kernel::DecodeFailureCount::<StorageTest>::get();
        submit_v2(comit_id, evm_payload, Vec::new()).expect("an EVM-only comit must be accepted");

        assert_eq!(
            pallet_x3_kernel::CanonicalLedger::<StorageTest>::get(0u64, expected_asset),
            expected_balance,
            "the balance-shaped channel must still land in CanonicalLedger"
        );
        assert_eq!(
            pallet_x3_kernel::DecodeFailureCount::<StorageTest>::get(),
            before,
            "a well-formed balance change must not be counted as a decode failure"
        );
        assert_eq!(
            Kernel::x3_contract_slot(evm_slot_key(0)),
            None,
            "an EVM balance change must not be written to the X3VM slot keyspace"
        );
    });
}
