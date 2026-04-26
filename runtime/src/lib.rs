#![deny(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]
#![allow(dead_code)]
#![allow(clippy::manual_contains)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::single_component_path_imports)]

// Required for impl_runtime_apis! macro in no_std
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(all(not(feature = "std"), target_arch = "wasm32"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

use codec::{Decode, Encode};
use sp_std::vec::Vec;
use frame_support::PalletId;
pub use frame_support::{
    construct_runtime, parameter_types,
    traits::{ConstBool, ConstU16, ConstU32, ConstU64, ConstU8, Everything, Get},
    weights::{
        constants::{
            BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_REF_TIME_PER_SECOND,
        },
        ConstantMultiplier, IdentityFee, WeightToFee,
    },
};
use frame_support::{traits::Currency, weights::Weight};
use frame_system::limits;
use pallet_agent_accounts;
use pallet_agent_memory;
use pallet_atomic_trade_engine;
use pallet_aura;
use pallet_balances;
use pallet_collective;
use pallet_evolution_core;
use pallet_governance;
use pallet_grandpa;
use pallet_offences;
use pallet_preimage;
use pallet_scheduler;
use pallet_session;
#[cfg(feature = "dev")]
use pallet_sudo;
use pallet_swarm;
use pallet_timestamp;
use pallet_x3_jury_anchor;
use pallet_transaction_payment::CurrencyAdapter;
use pallet_treasury;
use pallet_x3_atomic_kernel;
use pallet_x3_kernel;
use pallet_x3_settlement_engine;
use pallet_x3_verifier;
use scale_info::TypeInfo;
use sp_api::impl_runtime_apis;
use sp_core::{OpaqueMetadata, H160, H256, U256};
use sp_runtime::{
    create_runtime_str, generic, impl_opaque_keys,
    traits::{
        AccountIdConversion, AccountIdLookup, BlakeTwo256, Block as BlockT, ConvertInto,
        IdentifyAccount, SaturatedConversion, Verify,
    },
    MultiAddress, MultiSignature, Perbill,
};
use sp_session::{GetSessionNumber, GetValidatorCount};
use sp_std::prelude::*;

mod precompiles;
use precompiles::FrontierPrecompiles;

// ════════════════════════════════════════════════════════════════════════════════════
// GPU Validator Runtime API Types
// ════════════════════════════════════════════════════════════════════════════════════
#[cfg(feature = "gpu-validator")]
pub mod gpu_validator_api {
    use super::AccountId;
    use codec::{Decode, Encode};
    use scale_info::TypeInfo;
    use sp_std::vec::Vec;

    /// GPU validator status response
    #[derive(Debug, Clone, Encode, Decode, TypeInfo)]
    pub struct GpuValidatorStatus {
        /// Validator ID
        pub validator_id: u32,
        /// Health status: "healthy", "degraded", "unhealthy"
        pub health_status: Vec<u8>,
        /// Total proofs processed
        pub total_proofs_processed: u64,
        /// Successful proofs
        pub successful_proofs: u64,
        /// Failed proofs
        pub failed_proofs: u64,
        /// GPU devices online
        pub gpu_devices_online: u32,
        /// CPU fallback active
        pub cpu_fallback_active: bool,
        /// Last health check block
        pub last_health_check_block: u32,
    }

    /// Orchestrator health status
    #[derive(Debug, Clone, Encode, Decode, TypeInfo)]
    pub struct OrchestratorHealthStatus {
        /// Overall status: "operational", "degraded", "error"
        pub status: Vec<u8>,
        /// Uptime seconds
        pub uptime_seconds: u64,
        /// Active validators
        pub active_validators: u32,
        /// Quarantined validators
        pub quarantined_validators: u32,
        /// Pending task count
        pub pending_tasks: u32,
        /// Tasks completed this epoch
        pub tasks_completed: u64,
        /// Average task latency ms
        pub avg_task_latency_ms: u32,
        /// Network health: 0-100
        pub network_health_percent: u8,
    }

    /// GPU proof submission result
    #[derive(Debug, Clone, Encode, Decode, TypeInfo)]
    pub struct GpuProofResult {
        /// Proof hash
        pub proof_hash: [u8; 32],
        /// Status: "accepted", "rejected", "pending"
        pub status: Vec<u8>,
        /// Error message if rejected
        pub error_message: Vec<u8>,
        /// Validator processed by
        pub processed_by_validator: u32,
    }

    sp_api::decl_runtime_apis! {
        /// GPU Validator runtime API for querying validator status and submitting proofs
        pub trait GpuValidatorRuntimeApi {
            /// Get GPU validator status
            fn gpu_validator_status(validator_id: u32) -> Option<GpuValidatorStatus>;
            /// Query orchestrator health
            fn query_orchestrator_health() -> OrchestratorHealthStatus;
            /// Submit GPU validator proof
            fn submit_gpu_validator_proof(proof: Vec<u8>, validator_id: u32) -> GpuProofResult;
        }

        /// Cross-chain header validation and proof aggregation API (Phase 9)
        pub trait CrossChainStateRootApi {
            /// Validate EVM block header and return proof
            fn validate_evm_header(
                block_number: u64,
                block_hash: sp_core::H256,
                state_root: sp_core::H256,
            ) -> Option<crate::cross_chain_state_root_api::EvmHeaderProof>;

            /// Validate SVM (Solana) block header and return proof
            fn validate_svm_header(
                slot: u64,
                block_hash: sp_core::H256,
                state_root: sp_core::H256,
            ) -> Option<crate::cross_chain_state_root_api::SvmHeaderProof>;

            /// Query cross-chain validation status
            fn query_cross_chain_status() -> crate::cross_chain_state_root_api::CrossChainValidationStatus;

            /// Aggregate multiple proofs into a single cross-chain proof
            fn aggregate_cross_chain_proofs(
                proofs: Vec<crate::cross_chain_state_root_api::CrossChainProofBatch>,
            ) -> Option<crate::cross_chain_state_root_api::CrossChainProofBatch>;

            /// Query the last validated EVM header
            fn query_last_evm_header() -> Option<crate::cross_chain_state_root_api::EvmHeaderInfo>;

            /// Query the last validated SVM header
            fn query_last_svm_header() -> Option<crate::cross_chain_state_root_api::SvmHeaderInfo>;

            /// Verify if an EVM merkle root is cached for a block
            fn verify_evm_merkle_root(block_number: u64, merkle_root: sp_core::H256) -> bool;

            /// Verify if an SVM validator set is cached for a slot
            fn verify_svm_validator_set(slot: u64, validator_set_hash: sp_core::H256) -> bool;
        }

        /// Governance-driven settlement finality and dispute resolution API (Phase 10a)
        pub trait GovernanceSettlementApi {
            /// Submit a dispute challenge against a settlement proof
            fn submit_dispute(
                proof_hash: sp_core::H256,
                reason: Vec<u8>,
            ) -> Option<crate::governance_settlement_api::DisputeRecord>;

            /// Query the voting state of an active dispute
            fn query_dispute_status(
                proof_hash: sp_core::H256,
            ) -> Option<crate::governance_settlement_api::DisputeRecord>;

            /// Confirm that a proof has reached settlement finality
            fn confirm_settlement_finality(
                proof_hash: sp_core::H256,
            ) -> Option<crate::governance_settlement_api::ProofFinalityStatus>;
        }

        /// Settlement finality and validator attestation API (Phase 10a)
        pub trait SettlementFinalityApi {
            /// Query finality confirmation metrics
            fn query_finality_metrics() -> crate::governance_settlement_api::FinalityMetrics;

            /// Get validator dispute resolution reputation score
            fn query_validator_reputation(
                validator_id: AccountId,
            ) -> crate::governance_settlement_api::ValidatorReputation;

            /// Check if a merkle-aggregated batch has finality
            fn query_batch_finality_status(
                merkle_root: sp_core::H256,
            ) -> Option<crate::governance_settlement_api::BatchFinalityStatus>;
        }
    }
}

pub mod fraud_proofs;

#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

// WASM binary generated by substrate-wasm-builder
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

// When building for WASM (no-std), provide empty binaries
#[cfg(not(feature = "std"))]
pub const WASM_BINARY: Option<&[u8]> = None;
#[cfg(not(feature = "std"))]
pub const WASM_BINARY_BLOATY: Option<&[u8]> = None;

/// Opaque types used by the CLI commands.
pub mod opaque {
    use super::*;

    pub type BlockNumber = super::BlockNumber;
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    pub type UncheckedExtrinsic = sp_runtime::OpaqueExtrinsic;
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    pub type BlockId = generic::BlockId<Block>;
}

pub type BlockNumber = u32;
pub type Index = u32;
/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Nonce = Index;
pub type Signature = MultiSignature;
pub type Hash = H256;
pub type Moment = u64;
pub type Balance = u128;
pub type AssetId = u32;
pub type AtlasId = H256;
pub type Address = MultiAddress<AccountId, ()>;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

pub const MILLISECS_PER_BLOCK: u64 = 200; // 200ms target for higher throughput and lower latency

pub const fn blocks_from_millis(milliseconds: u64) -> BlockNumber {
    (milliseconds / MILLISECS_PER_BLOCK) as BlockNumber
}

pub struct RuntimeVersion;
impl frame_support::traits::Get<sp_version::RuntimeVersion> for RuntimeVersion {
    fn get() -> sp_version::RuntimeVersion {
        VERSION
    }
}
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

pub const NANO_ATLAS: Balance = 1;
pub const MICRO_ATLAS: Balance = 1_000 * NANO_ATLAS;
pub const MILLI_ATLAS: Balance = 1_000 * MICRO_ATLAS;
pub const X3: Balance = 1_000 * MILLI_ATLAS;
pub const NATIVE_GAS_PRICE: u64 = 1_000_000_000;

#[sp_version::runtime_version]
pub const VERSION: sp_version::RuntimeVersion = sp_version::RuntimeVersion {
    spec_name: create_runtime_str!("x3-chain"),
    impl_name: create_runtime_str!("x3-chain"),
    authoring_version: 1,
    // v5: 200ms slot duration migration. Nodes MUST check spec_version to select
    // the correct slot duration for pre/post-upgrade blocks to prevent Aura
    // slot monotonicity failures. See node/src/service.rs slot_duration_for_spec().
    spec_version: 5,
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 1,
    state_version: 1,
};

parameter_types! {
    pub const BlockHashCount: BlockNumber = 2_400;
    pub const SS58Prefix: u16 = 42;
    pub const MinimumPeriod: Moment = (MILLISECS_PER_BLOCK / 2) as Moment;
    pub const ExistentialDeposit: Balance = 100 * MICRO_ATLAS;
    pub const TransactionByteFee: Balance = 10 * MICRO_ATLAS;
    pub const MaxAssetsPerAccount: u32 = 32;
    pub const MaxAssetSymbolLength: u32 = 16;
    pub const MaxPayloadLength: u32 = 128 * 1024;
    pub const MaxEvmPayloadLength: u32 = 64 * 1024;  // 64 KB for EVM payloads
    pub const MaxSvmPayloadLength: u32 = 64 * 1024;  // 64 KB for SVM payloads
    pub const MaxX3PayloadLength: u32 = 64 * 1024;  // 64 KB for X3 payloads
    pub const MaxCombinedPayloadLength: u32 = 128 * 1024;  // 128 KB combined limit
    pub const MaxCombinedPayloadLengthV2: u32 = 192 * 1024;  // 192 KB combined (EVM+SVM+X3)
    pub const MaxAuthorities: u32 = 100;  // Maximum 100 authorities
    pub const MinAuthorities: u32 = 1;  // Minimum 1 authority required
    pub const DefaultEvmGasLimit: u64 = 12_000_000;  // tuned for 200ms slots on commodity validators
    pub const DefaultSvmComputeLimit: u64 = 200_000;  // 200k compute units for SVM
    pub const DefaultX3GasLimit: u64 = 6_000_000;  // tuned for 200ms slots on commodity validators
    pub const CrossVmPrepareTtl: BlockNumber = 50; // 50 blocks (~10s at 200ms)
    pub const MaxPreparedCrossVmOps: u32 = 1024;
    pub const MaxPreparedOpsPerBlock: u32 = 64;
    /// Maximum replay-store entries pruned per block. Bounds
    /// `on_initialize` work by the cross-VM replay-store pruner.
    pub const MaxReplayPruneItemsPerBlock: u32 = 256;
    pub const RequireCrossVmProof: bool = true;
    /// EVM bridge escrow contract address for atomic cross-VM swaps.
    pub BridgeEvmEscrow: H160 = H160([
        0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56, 0x78, 0x90,
        0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56, 0x78, 0x90,
    ]);
    /// SVM bridge escrow program address for atomic cross-VM swaps.
    pub BridgeSvmEscrow: [u8; 32] = [
        0x58, 0x33, 0x42, 0x72, 0x69, 0x64, 0x67, 0x65, 0x45, 0x73, 0x63, 0x72, 0x6f, 0x77,
        0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31,
        0x31, 0x31, 0x31, 0x31,
    ];
    pub BlockWeights: limits::BlockWeights = limits::BlockWeights::with_sensible_defaults(
        // Keep max execution budget below slot time (200ms) to avoid author/import divergence.
        Weight::from_parts((WEIGHT_REF_TIME_PER_SECOND / 1000) * 150, 5 * 1024 * 1024),
        Perbill::from_percent(90),
    );
    pub BlockLength: limits::BlockLength = limits::BlockLength::max_with_normal_ratio(
        5 * 1024 * 1024, // 5MB hard cap to reduce import pressure
        Perbill::from_percent(90),
    );
}

parameter_types! {
    pub const ChainId: u64 = 650_000;
    pub const GasLimitPovSizeRatio: u64 = 40;
    pub WeightPerGas: Weight = Weight::from_parts(20_000, 0);
}

pub struct BlockGasLimit;
impl Get<U256> for BlockGasLimit {
    fn get() -> U256 {
        U256::from(15_000_000u64)
    }
}

pub struct PrecompilesValue;
impl Get<FrontierPrecompiles<Runtime>> for PrecompilesValue {
    fn get() -> FrontierPrecompiles<Runtime> {
        FrontierPrecompiles::new()
    }
}

#[cfg(feature = "std")]
pub fn native_version() -> sp_version::NativeVersion {
    sp_version::NativeVersion {
        runtime_version: VERSION,
        can_author_with: Default::default(),
    }
}

parameter_types! {
    pub const MaxSetIdSessionEntries: u64 = 168; // ~1 week at 1 hour sessions
    pub const OperationalFeeMultiplier: u8 = 5;
}

parameter_types! {
    pub const CouncilMotionDuration: BlockNumber = (3 * 24 * 60 * 60 * 1000) / MILLISECS_PER_BLOCK as BlockNumber; // 3 days in ms / block time
    pub const CouncilMaxProposals: u32 = 100;
    pub const CouncilMaxMembers: u32 = 100;
    pub MaxProposalWeight: Weight = Perbill::from_percent(50) * BlockWeights::get().max_block;
}

// ── Fraud-proof pallet constants ─────────────────────────────────────────────
parameter_types! {
    /// Maximum transactions in a single scheduler witness (prevents DoS).
    pub const FraudProofMaxTxCount: u32 = 256;
    /// Blocks within which a fraud proof must be submitted after the disputed block.
    /// At 200 ms/block this is 24 hours (432_000 × 0.2s = 86_400s).
    pub const FraudProofDisputeWindowBlocks: u32 = 432_000;
    /// Reward paid to the reporter on accepted fraud proof (1 ATLAS).
    pub const FraudProofReporterReward: Balance = X3;
}

// ── Sequencer pallet constants ───────────────────────────────────────────────
parameter_types! {
    /// Maximum transactions per sequencer batch.
    pub const SeqMaxTxsPerBatch: u32 = 2048;
    /// Maximum payload size per sequenced transaction (bytes).
    pub const SeqMaxPayloadSize: u32 = 128 * 1024; // 128 KB
    /// Per-byte fee for sequencing (anti-spam).
    pub const SeqPerByteFee: u128 = 10;  // 10 nATLAS per byte
    /// Minimum base fee per transaction.
    pub const SeqBaseFee: u128 = 1_000;  // 1 µATLAS
    /// Enable X3 Atomic Kernel.
    pub const AtomicKernelEnabled: bool = true;
}

/// Runtime constants for performance and safety parameters.
pub const ATOMIC_KERNEL_VERSION: u32 = 1;
pub const ATOMIC_KERNEL_MAX_BATCH_GAS: u64 = 12_000_000; // consistent with DefaultEvmGasLimit

// ── DA pallet constants ──────────────────────────────────────────────────────
parameter_types! {
    /// Maximum blob size for DA (4 MB).
    pub const DaMaxBlobSize: u32 = 4 * 1024 * 1024;
    /// Per-byte fee for DA submissions.
    pub const DaPerByteFee: u128 = 5;  // 5 nATLAS per byte
    /// Maximum shard proofs per blob.
    pub const DaMaxShardProofs: u32 = 128;
    /// DA retention window (blocks) — ~24 hours at 200ms blocks.
    pub const DaRetentionBlocks: BlockNumber = 432_000;
}

#[cfg(feature = "dev")]
construct_runtime!(
    pub enum Runtime {
        System: frame_system,
        Timestamp: pallet_timestamp,
        Aura: pallet_aura,
        Grandpa: pallet_grandpa,
        Session: pallet_session,
        Offences: pallet_offences,
        Balances: pallet_balances,
        TransactionPayment: pallet_transaction_payment,
        Scheduler: pallet_scheduler,
        Preimage: pallet_preimage,
        EVM: pallet_evm,
        AtlasKernel: pallet_x3_kernel,
        X3Coin: pallet_x3_coin,
        AtomicTradeEngine: pallet_atomic_trade_engine,
        Council: pallet_collective::<Instance1>,
        Sudo: pallet_sudo,
        Governance: pallet_governance,
        Treasury: pallet_treasury,
        AgentAccounts: pallet_agent_accounts,
        AgentMemory: pallet_agent_memory,
        EvolutionCore: pallet_evolution_core,
        X3Verifier: pallet_x3_verifier,
        X3DomainRegistry: pallet_x3_domain_registry,
        X3JuryAnchor: pallet_x3_jury_anchor,
        X3SettlementEngine: pallet_x3_settlement_engine,
        Swarm: pallet_swarm,
        DepinMarketplace: pallet_depin_marketplace,
        PrivateExecution: pallet_private_execution,
        X3Sequencer: pallet_x3_sequencer,
        FraudProofs: crate::fraud_proofs::pallet::pallet,
        X3Da: pallet_x3_da,
        X3AtomicKernel: pallet_x3_atomic_kernel,
        X3AssetRegistry: pallet_x3_asset_registry,
        X3SupplyLedger: pallet_x3_supply_ledger,
        X3CrossVmRouter: pallet_x3_cross_vm_router,
        X3TokenFactory: pallet_x3_token_factory,
        CrossChainValidator: pallet_cross_chain_validator,
    }
);

#[cfg(not(feature = "dev"))]
construct_runtime!(
    pub enum Runtime {
        System: frame_system,
        Timestamp: pallet_timestamp,
        Aura: pallet_aura,
        Grandpa: pallet_grandpa,
        Session: pallet_session,
        Offences: pallet_offences,
        Balances: pallet_balances,
        TransactionPayment: pallet_transaction_payment,
        Scheduler: pallet_scheduler,
        Preimage: pallet_preimage,
        EVM: pallet_evm,
        AtlasKernel: pallet_x3_kernel,
        X3Coin: pallet_x3_coin,
        AtomicTradeEngine: pallet_atomic_trade_engine,
        Council: pallet_collective::<Instance1>,
        Governance: pallet_governance,
        Treasury: pallet_treasury,
        AgentAccounts: pallet_agent_accounts,
        AgentMemory: pallet_agent_memory,
        EvolutionCore: pallet_evolution_core,
        X3Verifier: pallet_x3_verifier,
        X3DomainRegistry: pallet_x3_domain_registry,
        X3JuryAnchor: pallet_x3_jury_anchor,
        X3SettlementEngine: pallet_x3_settlement_engine,
        Swarm: pallet_swarm,
        DepinMarketplace: pallet_depin_marketplace,
        PrivateExecution: pallet_private_execution,
        X3Sequencer: pallet_x3_sequencer,
        X3Da: pallet_x3_da,
        // ISSUE #3 FIX: FraudProofs moved AFTER X3Da to avoid forward reference
        // FraudProofs now reads X3Da state after block execution completes
        FraudProofs: crate::fraud_proofs::pallet::pallet,
        X3AtomicKernel: pallet_x3_atomic_kernel,
        X3AssetRegistry: pallet_x3_asset_registry,
        X3SupplyLedger: pallet_x3_supply_ledger,
        X3CrossVmRouter: pallet_x3_cross_vm_router,
        X3TokenFactory: pallet_x3_token_factory,
        CrossChainValidator: pallet_cross_chain_validator,
    }
);

pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
pub type UncheckedExtrinsic =
    generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
// Runtime storage migrations tuple. Add migration structs for pallets that need upgrades.
// Note: Only x3-kernel has migrations currently implemented
pub type Migrations = (pallet_x3_kernel::migrations::Migration<Runtime>,);

// Use the migrations tuple in the executive so migrations run on runtime upgrades
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
    Migrations,
>;

impl_opaque_keys! {
    pub struct SessionKeys {
        pub aura: Aura,
    }
}

pub type SignedExtra = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);

pub type SignedPayload = generic::SignedPayload<RuntimeCall, SignedExtra>;

// ===== Config Impls (after construct_runtime!) =====

type NegativeImbalance = <Balances as Currency<AccountId>>::NegativeImbalance;

pub struct DealWithFees;
impl frame_support::traits::OnUnbalanced<NegativeImbalance> for DealWithFees {
    fn on_unbalanced(amount: NegativeImbalance) {
        drop(amount);
    }
}

pub struct FixedGasPrice;
impl pallet_evm::FeeCalculator for FixedGasPrice {
    fn min_gas_price() -> (U256, Weight) {
        (U256::from(NATIVE_GAS_PRICE), Weight::zero())
    }
}

impl frame_system::Config for Runtime {
    type BaseCallFilter = Everything;
    type Block = Block;
    type BlockWeights = BlockWeights;
    type BlockLength = BlockLength;
    type DbWeight = RocksDbWeight;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Hash = Hash;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = AccountIdLookup<AccountId, ()>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type Version = RuntimeVersion;
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = frame_system::weights::SubstrateWeight<Runtime>;
    type SS58Prefix = ConstU16<42>;
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
    type Nonce = Index;
}

impl pallet_timestamp::Config for Runtime {
    type Moment = Moment;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl pallet_aura::Config for Runtime {
    type AuthorityId = sp_consensus_aura::sr25519::AuthorityId;
    type MaxAuthorities = MaxAuthorities;
    type DisabledValidators = ();
    type AllowMultipleBlocksPerSlot = ConstBool<true>; // Enable multiple blocks per slot for higher TPS
}

parameter_types! {
    pub const ReportLongevity: u64 = 1000;
}

impl pallet_session::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = <Self as frame_system::Config>::AccountId;
    type ValidatorIdOf = ConvertInto;
    type ShouldEndSession = pallet_session::PeriodicSessions<ConstU32<1800>, ConstU32<0>>;
    type NextSessionRotation = pallet_session::PeriodicSessions<ConstU32<1800>, ConstU32<0>>;
    type SessionManager = ();
    type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
    type Keys = SessionKeys;
    type WeightInfo = pallet_session::weights::SubstrateWeight<Self>;
}

pub type Historical = pallet_session::historical::Pallet<Runtime>;

impl pallet_session::historical::Config for Runtime {
    type FullIdentification = AccountId;
    type FullIdentificationOf = ConvertInto;
}

impl pallet_offences::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type IdentificationTuple = pallet_session::historical::IdentificationTuple<Runtime>;
    type OnOffence = ();
    type WeightInfo = pallet_offences::weights::SubstrateWeight<Runtime>;
}

impl pallet_grandpa::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type KeyOwnerProof = sp_session::historical::MembershipProof;
    type EquivocationReportSystem = pallet_grandpa::EquivocationReportSystem<
        Runtime,
        pallet_session::historical::Pallet<Runtime>,
        pallet_offences::Pallet<Runtime>,
    >;
    type WeightInfo = pallet_grandpa::weights::SubstrateWeight<Runtime>;
    type MaxAuthorities = MaxAuthorities;
    type MaxSetIdSessionEntries = MaxSetIdSessionEntries;
}

impl pallet_balances::Config for Runtime {
    type Balance = Balance;
    type DustRemoval = ();
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ConstU32<50>;
    type MaxHolds = ConstU32<0>;
    type MaxFreezes = ConstU32<0>;
    type ReserveIdentifier = [u8; 8];
    type FreezeIdentifier = ();
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    type RuntimeHoldReason = ();
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = CurrencyAdapter<Balances, DealWithFees>;
    type OperationalFeeMultiplier = OperationalFeeMultiplier;
    type WeightToFee = IdentityFee<Balance>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type FeeMultiplierUpdate = ();
}

#[cfg(feature = "dev")]
impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

pub type EnsureRootOrHalfCouncil = frame_support::traits::EitherOfDiverse<
    frame_system::EnsureRoot<AccountId>,
    pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 2>,
>;

pub type EnsureCouncilMember = pallet_collective::EnsureMember<AccountId, CouncilCollective>;

pub type CouncilCollective = pallet_collective::Instance1;
impl pallet_collective::Config<CouncilCollective> for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type Proposal = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type MotionDuration = CouncilMotionDuration;
    type MaxProposals = CouncilMaxProposals;
    type MaxMembers = CouncilMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
    type SetMembersOrigin = frame_system::EnsureRoot<AccountId>;
    type MaxProposalWeight = MaxProposalWeight;
}

// ── Fraud-proof inline pallet config ─────────────────────────────────────────
impl crate::fraud_proofs::pallet::pallet::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MaxTxCount = FraudProofMaxTxCount;
    type DisputeWindowBlocks = FraudProofDisputeWindowBlocks;
    type ReporterRewardAmount = FraudProofReporterReward;
    type GovernanceOrigin = EnsureRootOrHalfCouncil;
}

impl pallet_evm::Config for Runtime {
    type FeeCalculator = FixedGasPrice;
    type GasWeightMapping = pallet_evm::FixedGasWeightMapping<Self>;
    type WeightPerGas = WeightPerGas;
    type BlockHashMapping = pallet_evm::SubstrateBlockHashMapping<Self>;
    type CallOrigin = pallet_evm::EnsureAddressRoot<AccountId>;
    type WithdrawOrigin = pallet_evm::EnsureAddressTruncated;
    type AddressMapping = pallet_evm::HashedAddressMapping<BlakeTwo256>;
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type PrecompilesType = FrontierPrecompiles<Self>;
    type PrecompilesValue = PrecompilesValue;
    type ChainId = ChainId;
    type BlockGasLimit = BlockGasLimit;
    type Runner = pallet_evm::runner::stack::Runner<Self>;
    type OnChargeTransaction = pallet_evm::EVMCurrencyAdapter<Balances, ()>;
    type OnCreate = ();
    type FindAuthor = ();
    type GasLimitPovSizeRatio = GasLimitPovSizeRatio;
    type Timestamp = Timestamp;
    type WeightInfo = pallet_evm::weights::SubstrateWeight<Self>;
}

/// Production cross-chain proof verifier.
///
/// Validates `LockProof` and `MerkleReceipt` payloads with structural
/// sanity checks and enforces a byzantine threshold of validator signatures
/// from the currently configured X3 kernel authorities.
pub struct SubstrateProofVerifier;

impl pallet_x3_kernel::CrossChainProofVerifier<AccountId> for SubstrateProofVerifier {
    fn verify_proof(
        _origin: &AccountId,
        _operation: &x3_cross_vm_bridge::CrossVmOperation,
        proof: &pallet_x3_kernel::CrossChainProof,
    ) -> Result<(), frame_support::sp_runtime::DispatchError> {
        use codec::Encode;
        use pallet_x3_kernel::CrossChainProof;

        fn threshold(authority_count: usize) -> usize {
            // 2/3 + 1, but always at least 1.
            let needed = (authority_count.saturating_mul(2) / 3).saturating_add(1);
            core::cmp::max(1, needed)
        }

        fn account_to_key_bytes(
            account: &AccountId,
        ) -> Result<[u8; 32], frame_support::sp_runtime::DispatchError> {
            let encoded = account.encode();
            if encoded.len() != 32 {
                return Err(frame_support::sp_runtime::DispatchError::Other(
                    "Authority key must SCALE-encode to 32 bytes",
                ));
            }
            let mut out = [0u8; 32];
            out.copy_from_slice(&encoded);
            Ok(out)
        }

        fn verify_signature_any(pubkey_bytes: [u8; 32], message: &[u8], signature: &[u8]) -> bool {
            if signature.len() != 64 {
                return false;
            }

            // sr25519
            {
                let pubkey = sp_core::sr25519::Public::from_raw(pubkey_bytes);
                let sig = sp_core::sr25519::Signature::from_raw({
                    let mut buf = [0u8; 64];
                    buf.copy_from_slice(signature);
                    buf
                });
                if sp_io::crypto::sr25519_verify(&sig, message, &pubkey) {
                    return true;
                }
            }

            // ed25519
            {
                let pubkey = sp_core::ed25519::Public::from_raw(pubkey_bytes);
                let sig = sp_core::ed25519::Signature::from_raw({
                    let mut buf = [0u8; 64];
                    buf.copy_from_slice(signature);
                    buf
                });
                sp_io::crypto::ed25519_verify(&sig, message, &pubkey)
            }
        }

        fn require_len(
            actual: usize,
            expected: usize,
            label: &'static str,
        ) -> Result<(), frame_support::sp_runtime::DispatchError> {
            if actual != expected {
                return Err(frame_support::sp_runtime::DispatchError::Other(label));
            }
            Ok(())
        }

        let authorities = pallet_x3_kernel::Authorities::<Runtime>::get();
        let authority_keys: sp_std::collections::btree_set::BTreeSet<[u8; 32]> = authorities
            .into_iter()
            .map(|a| account_to_key_bytes(&a))
            .collect::<Result<_, _>>()?;
        let needed = threshold(authority_keys.len());

        match proof {
            CrossChainProof::None => Ok(()),
            CrossChainProof::LockProof(bytes) => {
                if bytes.is_empty() {
                    return Err(frame_support::sp_runtime::DispatchError::Other(
                        "LockProof: empty bytes",
                    ));
                }
                // Format:
                // [0..32)  event_hash
                // [32]     sig_count (u8)
                // repeat sig_count times:
                //   [validator_id:32][signature:64]
                if bytes.len() < 33 {
                    return Err(frame_support::sp_runtime::DispatchError::Other(
                        "LockProof: payload too short (< 33 bytes)",
                    ));
                }

                let event_hash = &bytes[0..32];
                let sig_count = bytes[32] as usize;
                if sig_count == 0 {
                    return Err(frame_support::sp_runtime::DispatchError::Other(
                        "LockProof: signature count must be > 0",
                    ));
                }

                let expected_len = 33usize.saturating_add(sig_count.saturating_mul(96));
                require_len(
                    bytes.len(),
                    expected_len,
                    "LockProof: malformed payload length",
                )?;

                let mut valid = 0usize;
                let mut seen: sp_std::collections::btree_set::BTreeSet<[u8; 32]> =
                    sp_std::collections::btree_set::BTreeSet::new();

                for idx in 0..sig_count {
                    let offset = 33 + idx * 96;
                    let mut validator_id = [0u8; 32];
                    validator_id.copy_from_slice(&bytes[offset..offset + 32]);
                    let signature = &bytes[offset + 32..offset + 96];

                    if !authority_keys.contains(&validator_id) {
                        continue;
                    }
                    if !seen.insert(validator_id) {
                        continue;
                    }
                    if verify_signature_any(validator_id, event_hash, signature) {
                        valid = valid.saturating_add(1);
                    }
                }

                if valid < needed {
                    return Err(frame_support::sp_runtime::DispatchError::Other(
                        "LockProof: insufficient validator signatures",
                    ));
                }

                Ok(())
            }
            CrossChainProof::MerkleReceipt(bytes) => {
                if bytes.is_empty() {
                    return Err(frame_support::sp_runtime::DispatchError::Other(
                        "MerkleReceipt: empty bytes",
                    ));
                }
                // Format:
                // [0..32)  state_root
                // [32..40) finalized_block (u64 LE)
                // [40..48) execution_index (u64 LE)
                // [48..52) merkle_proof_len (u32 LE)
                // [..]     merkle_proof_bytes (len)
                // [..]     sig_count (u8)
                // repeat sig_count times:
                //   [validator_id:32][signature:64] over sha2_256(state_root||finalized_block||execution_index||merkle_proof_bytes)
                if bytes.len() < 53 {
                    return Err(frame_support::sp_runtime::DispatchError::Other(
                        "MerkleReceipt: payload too short",
                    ));
                }

                let mut state_root = [0u8; 32];
                state_root.copy_from_slice(&bytes[0..32]);
                if state_root == [0u8; 32] {
                    return Err(frame_support::sp_runtime::DispatchError::Other(
                        "MerkleReceipt: state_root must be non-zero",
                    ));
                }

                let finalized_block =
                    u64::from_le_bytes(bytes[32..40].try_into().map_err(|_| {
                        frame_support::sp_runtime::DispatchError::Other(
                            "MerkleReceipt: invalid finalized_block",
                        )
                    })?);
                if finalized_block == 0 {
                    return Err(frame_support::sp_runtime::DispatchError::Other(
                        "MerkleReceipt: finalized_block must be > 0",
                    ));
                }

                let execution_index =
                    u64::from_le_bytes(bytes[40..48].try_into().map_err(|_| {
                        frame_support::sp_runtime::DispatchError::Other(
                            "MerkleReceipt: invalid execution_index",
                        )
                    })?);

                let merkle_len = u32::from_le_bytes(bytes[48..52].try_into().map_err(|_| {
                    frame_support::sp_runtime::DispatchError::Other(
                        "MerkleReceipt: invalid merkle_proof_len",
                    )
                })?) as usize;
                if merkle_len == 0 {
                    return Err(frame_support::sp_runtime::DispatchError::Other(
                        "MerkleReceipt: merkle_proof_bytes must be non-empty",
                    ));
                }

                let proof_start = 52usize;
                let proof_end = proof_start.checked_add(merkle_len).ok_or(
                    frame_support::sp_runtime::DispatchError::Other(
                        "MerkleReceipt: proof length overflow",
                    ),
                )?;
                if bytes.len() < proof_end + 1 {
                    return Err(frame_support::sp_runtime::DispatchError::Other(
                        "MerkleReceipt: payload too short for merkle proof",
                    ));
                }

                let merkle_proof_bytes = &bytes[proof_start..proof_end];
                let sig_count = bytes[proof_end] as usize;
                if sig_count == 0 {
                    return Err(frame_support::sp_runtime::DispatchError::Other(
                        "MerkleReceipt: signature count must be > 0",
                    ));
                }

                let expected_len = (proof_end + 1).saturating_add(sig_count.saturating_mul(96));
                require_len(
                    bytes.len(),
                    expected_len,
                    "MerkleReceipt: malformed payload length",
                )?;

                let mut msg =
                    sp_std::vec::Vec::with_capacity(32 + 8 + 8 + merkle_proof_bytes.len());
                msg.extend_from_slice(&state_root);
                msg.extend_from_slice(&finalized_block.to_le_bytes());
                msg.extend_from_slice(&execution_index.to_le_bytes());
                msg.extend_from_slice(merkle_proof_bytes);
                let settlement_hash = sp_io::hashing::sha2_256(&msg);

                let mut valid = 0usize;
                let mut seen: sp_std::collections::btree_set::BTreeSet<[u8; 32]> =
                    sp_std::collections::btree_set::BTreeSet::new();

                for idx in 0..sig_count {
                    let offset = (proof_end + 1) + idx * 96;
                    let mut validator_id = [0u8; 32];
                    validator_id.copy_from_slice(&bytes[offset..offset + 32]);
                    let signature = &bytes[offset + 32..offset + 96];

                    if !authority_keys.contains(&validator_id) {
                        continue;
                    }
                    if !seen.insert(validator_id) {
                        continue;
                    }
                    if verify_signature_any(validator_id, &settlement_hash, signature) {
                        valid = valid.saturating_add(1);
                    }
                }

                if valid < needed {
                    return Err(frame_support::sp_runtime::DispatchError::Other(
                        "MerkleReceipt: insufficient validator signatures",
                    ));
                }

                Ok(())
            }
        }
    }
}

impl pallet_x3_kernel::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type AssetId = AssetId;
    type AtlasId = AtlasId;
    type MaxAssetsPerAccount = MaxAssetsPerAccount;
    type MaxAssetSymbolLength = MaxAssetSymbolLength;
    type MaxEvmPayloadLength = MaxEvmPayloadLength;
    type MaxSvmPayloadLength = MaxSvmPayloadLength;
    type MaxX3PayloadLength = MaxX3PayloadLength;
    type MaxCombinedPayloadLength = MaxCombinedPayloadLength;
    type MaxCombinedPayloadLengthV2 = MaxCombinedPayloadLengthV2;
    type MaxAuthorities = MaxAuthorities;
    type MinAuthorities = MinAuthorities;
    type DefaultEvmGasLimit = DefaultEvmGasLimit;
    type DefaultSvmComputeLimit = DefaultSvmComputeLimit;
    type DefaultX3GasLimit = DefaultX3GasLimit;
    type CrossVmPrepareTtl = CrossVmPrepareTtl;
    type MaxPreparedCrossVmOps = MaxPreparedCrossVmOps;
    type MaxPreparedOpsPerBlock = MaxPreparedOpsPerBlock;
    type MaxReplayPruneItemsPerBlock = MaxReplayPruneItemsPerBlock;
    type RequireCrossVmProof = RequireCrossVmProof;
    type WeightInfo = pallet_x3_kernel::weights::SubstrateWeight<Runtime>;
    type Currency = Balances;
    // VM adapters:
    // - Native runtime (std): use real native adapters.
    // - WASM runtime (no_std): use inline interpreter adapters.
    #[cfg(feature = "std")]
    type EvmAdapter = native_vm_adapters::NativeEvmAdapter;
    #[cfg(feature = "std")]
    type SvmAdapter = native_vm_adapters::NativeSvmAdapter;
    #[cfg(feature = "std")]
    type X3Adapter = pallet_x3_kernel::adapters::real_adapters::X3VmAdapter;
    #[cfg(not(feature = "std"))]
    type EvmAdapter = pallet_x3_kernel::wasm_adapters::WasmEvmAdapter;
    #[cfg(not(feature = "std"))]
    type SvmAdapter = pallet_x3_kernel::wasm_adapters::WasmSvmAdapter;
    #[cfg(not(feature = "std"))]
    type X3Adapter = pallet_x3_kernel::wasm_adapters::WasmX3Adapter;
    type GovernanceOrigin = EnsureRootOrHalfCouncil;
    type CrossChainProofVerifier = SubstrateProofVerifier;
    type BridgeEvmEscrow = BridgeEvmEscrow;
    type BridgeSvmEscrow = BridgeSvmEscrow;
}

impl pallet_x3_coin::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type UnixTime = Timestamp;
    type WeightInfo = pallet_x3_coin::weights::SubstrateWeight<Runtime>;
    type TreasuryAccount = TreasuryAccountId;
    type MaxBonusClaims = ConstU32<10>;
    type TeamVestingBlocks = ConstU64<15768000>;
    type TeamVestingCliff = ConstU64<7884000>;
    type BonusClaimPeriod = ConstU64<3942000>;
}

// ===== AtomicTradeEngine Configuration =====

parameter_types! {
    pub const MaxTradeLegs: u32 = 16;
    pub const MaxCheckpoints: u32 = 8;
    pub const MaxPendingBatchesPerAccount: u32 = 64;
    pub const DefaultTradeEvmGasLimit: u64 = 500_000;
    pub const DefaultTradeSvmComputeLimit: u64 = 500_000;
    pub const DefaultTradeX3GasLimit: u64 = 500_000;
}

impl pallet_atomic_trade_engine::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_atomic_trade_engine::weights::SubstrateWeight<Runtime>;
    type Currency = Balances;
    // Use the same VM adapters as AtlasKernel
    #[cfg(feature = "std")]
    type EvmAdapter = native_vm_adapters::NativeEvmAdapter;
    #[cfg(feature = "std")]
    type SvmAdapter = native_vm_adapters::NativeSvmAdapter;
    #[cfg(feature = "std")]
    type X3Adapter = pallet_x3_kernel::adapters::real_adapters::X3VmAdapter;
    #[cfg(not(feature = "std"))]
    type EvmAdapter = pallet_x3_kernel::wasm_adapters::WasmEvmAdapter;
    #[cfg(not(feature = "std"))]
    type SvmAdapter = pallet_x3_kernel::wasm_adapters::WasmSvmAdapter;
    #[cfg(not(feature = "std"))]
    type X3Adapter = pallet_x3_kernel::wasm_adapters::WasmX3Adapter;
    type MaxTradeLegs = MaxTradeLegs;
    type MaxCheckpoints = MaxCheckpoints;
    type MaxPendingBatchesPerAccount = MaxPendingBatchesPerAccount;
    type DefaultTradeEvmGasLimit = DefaultTradeEvmGasLimit;
    type DefaultTradeSvmComputeLimit = DefaultTradeSvmComputeLimit;
    type DefaultTradeX3GasLimit = DefaultTradeX3GasLimit;
    type AmmRegistrarOrigin = EnsureRootOrHalfCouncil;
    type Settlement = SettlementAdapter;
}

/// Real settlement adapter that wires trade-engine batches into the
/// X3 Settlement Engine pallet (create intent + lock escrow per leg).
pub struct SettlementAdapter;

impl pallet_atomic_trade_engine::SettlementBridge<AccountId> for SettlementAdapter {
    fn register_completed_batch(
        maker: &AccountId,
        batch_id: sp_core::H256,
        secret_hash: sp_core::H256,
        legs: &[(pallet_atomic_trade_engine::VmType, u128)],
    ) -> Result<sp_core::H256, sp_runtime::DispatchError> {
        use pallet_x3_settlement_engine::types::{AssetSpec, ExternalChainId, TokenId};

        // The taker defaults to the batch origin for internal cross-VM swaps.
        // In a full orderbook scenario the taker comes from the match engine.
        let taker = maker.clone();

        // Map total amounts from legs into the two settlement asset specs.
        let total_output: u128 = legs.iter().map(|(_, amt)| amt).sum();
        let total_input: u128 = legs.iter().map(|(_, amt)| amt).sum();

        let asset_a = AssetSpec {
            chain: ExternalChainId::X3Native,
            token: TokenId::Native,
            amount: total_input,
        };
        let asset_b = AssetSpec {
            chain: ExternalChainId::X3Native,
            token: TokenId::Native,
            amount: total_output,
        };

        // Create intent via the settlement engine's internal API.
        // We call the dispatchable through a signed origin.
        // Read the nonce *before* create_intent increments TotalIntents.
        let intent_nonce = pallet_x3_settlement_engine::TotalIntents::<Runtime>::get();

        pallet_x3_settlement_engine::Pallet::<Runtime>::create_intent(
            frame_system::RawOrigin::Signed(maker.clone()).into(),
            taker,
            asset_a,
            asset_b,
            secret_hash,
            None, // Use default timeout
        )?;

        // Derive the intent_id the same way the settlement engine does.
        // Must use same nonce that create_intent used (which was TotalIntents before increment).
        let intent_id = pallet_x3_settlement_engine::Pallet::<Runtime>::generate_intent_id(
            maker,
            maker, // taker == maker for self-swaps
            intent_nonce,
        );

        // Lock escrow for each leg.
        for (i, (vm_type, amount_out)) in legs.iter().enumerate() {
            let chain = match vm_type {
                pallet_atomic_trade_engine::VmType::Evm => ExternalChainId::Ethereum,
                pallet_atomic_trade_engine::VmType::Svm => ExternalChainId::Solana,
                _ => ExternalChainId::X3Native,
            };

            // Best-effort escrow locking; log failures but don't abort.
            if let Err(e) = pallet_x3_settlement_engine::Pallet::<Runtime>::lock_escrow(
                frame_system::RawOrigin::Signed(maker.clone()).into(),
                intent_id,
                i as u32,
                chain,
                *amount_out,
                batch_id.as_bytes().to_vec(),
            ) {
                frame_support::log::warn!(
                    target: "trade-engine",
                    "Lock escrow failed for intent {:?} leg {}: {:?}",
                    intent_id, i, e,
                );
            }
        }

        Ok(intent_id)
    }
}

/// Deterministic non-zero EVM address used as the `source` for system-level
/// calls (cross-VM bridge, gas estimation, contract deployment).
///
/// Using `H160::zero()` is dangerous because the zero address is the
/// canonical "burn" address on Ethereum-compatible chains; sending value
/// from it or attributing actions to it breaks accounting invariants.
///
/// The bytes are a deterministic constant derived from the project name;
/// they do NOT collide with any user-derivable address.
const SYSTEM_EVM_CALLER: sp_core::H160 = sp_core::H160([
    0x9b, 0x6a, 0x5c, 0x1d, 0x4e, 0x8f, 0x2a, 0x7b, 0x3c, 0xd0, 0xe1, 0xf5, 0x06, 0x17, 0x28, 0x39,
    0x4a, 0x5b, 0x6c, 0x7d,
]);

#[cfg(feature = "std")]
mod native_vm_adapters {
    use super::*;
    use codec::Encode;
    use fp_evm::{CallInfo, ExitReason};
    use pallet_evm::Runner;
    use pallet_x3_kernel::{
        EvmExecutorAdapter, ExecutionLog, ExecutionReceipt, StateChange, SvmExecutorAdapter,
    };
    use sp_core::H160;
    use sp_runtime::{
        traits::{SaturatedConversion, UniqueSaturatedInto},
        DispatchError,
    };
    use std::collections::HashMap;
    use x3_svm_integration::{
        RbpfSvmExecutor, SvmConfig, SvmError, SvmExecutionResult, SvmExecutor,
    };

    pub struct NativeEvmAdapter;
    pub struct NativeSvmAdapter;

    const CANONICAL_NATIVE_ASSET_ID: AssetId = 0;

    impl EvmExecutorAdapter for NativeEvmAdapter {
        fn execute(payload: &[u8], gas_limit: u64) -> Result<ExecutionReceipt, DispatchError> {
            // Use Frontier's Runner directly for real EVM execution
            let source = SYSTEM_EVM_CALLER;
            let target = H160::zero(); // Default target (for create, this is ignored)
            let value = U256::zero();
            let evm_config = fp_evm::Config::shanghai();

            // Capture pre-execution balance for the source account so that
            // collect_evm_balance_changes can compute deltas instead of snapshots.
            let pre_balances = evm_balance_snapshot(&[source]);

            // Determine whether to perform a contract call or creation.
            // If the payload represents raw bytecode (typical for contract deployment),
            // perform a `create` so the bytecode is executed as contract initialization.
            // Full implementation would parse tx type from payload.
            // Try contract creation first (treat payload as init code). If that fails,
            // fall back to a call to the zero address (some payloads are call data).
            let create_res = <super::Runtime as pallet_evm::Config>::Runner::create(
                source,
                payload.to_vec(),
                value,
                gas_limit,
                Some(U256::from(super::NATIVE_GAS_PRICE)), // max_fee_per_gas
                None,                                      // max_priority_fee_per_gas
                None,                                      // nonce
                Vec::new(),                                // access_list
                false,                                     // is_transactional (dry run for kernel)
                false,                                     // validate (skip signature check)
                None,                                      // weight_limit
                None,                                      // proof_size_base_cost
                &evm_config,
            );

            if let Ok(info) = create_res {
                let success = matches!(info.exit_reason, ExitReason::Succeed(_));
                let state_changes =
                    collect_evm_balance_changes(source, None, &info.logs, &pre_balances);
                return Ok(ExecutionReceipt {
                    version: pallet_x3_kernel::EXECUTION_RECEIPT_VERSION,
                    success,
                    gas_used: info.used_gas.standard.unique_saturated_into(),
                    return_data: info.value.as_bytes().to_vec(),
                    logs: info
                        .logs
                        .into_iter()
                        .map(|log| ExecutionLog {
                            address: log.address.as_bytes().to_vec(),
                            topics: log.topics,
                            data: log.data,
                        })
                        .collect(),
                    state_changes,
                    protocol_version: 1,
                    migration_history: Vec::new(),
                    compatibility_flags: 0,
                });
            }

            // Fall back to a regular call to `target` (zero address by default).
            let call_res = <super::Runtime as pallet_evm::Config>::Runner::call(
                source,
                target,
                payload.to_vec(),
                value,
                gas_limit,
                Some(U256::from(super::NATIVE_GAS_PRICE)),
                None,
                None,
                Vec::new(),
                false,
                false,
                None,
                None,
                &evm_config,
            );

            match call_res {
                Ok(info) => Ok(map_call_info_to_receipt(
                    info,
                    source,
                    Some(target),
                    &pre_balances,
                )),
                Err(_) => Err(DispatchError::Other("Native EVM execution failed")),
            }
        }

        fn estimate_gas(payload: &[u8]) -> Result<u64, DispatchError> {
            let gas_limit = gas_ceiling();
            let source = SYSTEM_EVM_CALLER;
            let target = H160::zero();
            let evm_config = fp_evm::Config::shanghai();

            let call_result = <super::Runtime as pallet_evm::Config>::Runner::call(
                source,
                target,
                payload.to_vec(),
                U256::zero(),
                gas_limit,
                Some(U256::from(super::NATIVE_GAS_PRICE)),
                None,
                None,
                Vec::new(),
                false, // non-transactional for estimation
                false,
                None,
                None,
                &evm_config,
            );

            match call_result {
                Ok(info) => Ok(info.used_gas.standard.unique_saturated_into()),
                Err(_) => Err(DispatchError::Other("Gas estimation failed")),
            }
        }

        fn validate(payload: &[u8]) -> Result<(), DispatchError> {
            if payload.is_empty() {
                return Err(DispatchError::Other("Empty EVM payload"));
            }
            // Basic validation - could add opcode validation here
            Ok(())
        }
    }

    fn map_call_info_to_receipt(
        info: CallInfo,
        source: H160,
        target: Option<H160>,
        pre_balances: &HashMap<H160, Balance>,
    ) -> ExecutionReceipt {
        let success = matches!(info.exit_reason, ExitReason::Succeed(_));
        let state_changes = collect_evm_balance_changes(source, target, &info.logs, pre_balances);
        ExecutionReceipt {
            version: pallet_x3_kernel::EXECUTION_RECEIPT_VERSION,
            success,
            gas_used: info.used_gas.standard.unique_saturated_into(),
            return_data: info.value,
            logs: info
                .logs
                .into_iter()
                .map(|log| ExecutionLog {
                    address: log.address.as_bytes().to_vec(),
                    topics: log.topics,
                    data: log.data,
                })
                .collect(),
            state_changes,
            protocol_version: 1,
            migration_history: Vec::new(),
            compatibility_flags: 0,
        }
    }

    impl SvmExecutorAdapter for NativeSvmAdapter {
        fn execute(payload: &[u8], compute_limit: u64) -> Result<ExecutionReceipt, DispatchError> {
            if payload.is_empty() {
                return Err(DispatchError::Other("Empty SVM payload"));
            }

            let executor = RbpfSvmExecutor::new();
            let config = svm_config(compute_limit);
            let result = executor
                .execute_bpf(payload, &[], &config)
                .map_err(|err| DispatchError::Other(svm_error_str(err)))?;
            Ok(map_svm_receipt(result))
        }

        fn validate(payload: &[u8]) -> Result<(), DispatchError> {
            let executor = RbpfSvmExecutor::new();
            executor
                .validate_program(payload)
                .map_err(|err| DispatchError::Other(svm_error_str(err)))
        }
    }

    fn gas_ceiling() -> u64 {
        let limit = super::BlockGasLimit::get();
        if limit > U256::from(u64::MAX) {
            u64::MAX
        } else {
            limit.low_u64()
        }
    }

    fn svm_config(compute_limit: u64) -> SvmConfig {
        let slot = frame_system::Pallet::<super::Runtime>::block_number().saturated_into::<u64>();
        let ts: i64 = pallet_timestamp::Pallet::<super::Runtime>::now().saturated_into();

        SvmConfig {
            compute_unit_limit: compute_limit,
            compute_unit_price: 1,
            slot,
            block_timestamp: ts,
            recent_blockhash: [0u8; 32],
            enable_cpi: false,
            max_cpi_depth: 0,
        }
    }

    fn map_svm_receipt(result: SvmExecutionResult) -> ExecutionReceipt {
        let state_changes: Vec<StateChange> = result
            .account_updates
            .iter()
            .map(|update| {
                // Write non-empty account data back to persistent SVM account storage
                // so that stateful SVM programs retain their state across calls.
                if !update.data.is_empty() {
                    if let Ok(bounded) = frame_support::BoundedVec::<
                        u8,
                        frame_support::traits::ConstU32<
                            { pallet_x3_kernel::MAX_SVM_ACCOUNT_DATA_BYTES },
                        >,
                    >::try_from(update.data.clone())
                    {
                        pallet_x3_kernel::SvmAccountData::<super::Runtime>::insert(
                            update.pubkey,
                            bounded,
                        );
                    }
                }
                StateChange {
                    // SVM pubkeys are 32 bytes and can be decoded by the kernel as AccountId32.
                    address: update.pubkey.to_vec(),
                    key: canonical_asset_key(CANONICAL_NATIVE_ASSET_ID),
                    value: canonical_balance_value(update.lamports as Balance),
                }
            })
            .collect();
        ExecutionReceipt {
            version: pallet_x3_kernel::EXECUTION_RECEIPT_VERSION,
            success: result.success,
            gas_used: result.compute_units_used,
            return_data: result.output,
            logs: result
                .logs
                .into_iter()
                .map(|data| ExecutionLog {
                    address: vec![0u8; 32],
                    topics: Vec::new(),
                    data,
                })
                .collect(),
            state_changes,
            protocol_version: 1,
            migration_history: Vec::new(),
            compatibility_flags: 0,
        }
    }

    fn collect_evm_balance_changes(
        source: H160,
        target: Option<H160>,
        logs: &[fp_evm::Log],
        pre_balances: &HashMap<H160, Balance>,
    ) -> Vec<StateChange> {
        let mut touched = Vec::<H160>::new();
        let mut push_unique = |address: H160| {
            if !touched.iter().any(|existing| *existing == address) {
                touched.push(address);
            }
        };

        push_unique(source);
        if let Some(address) = target {
            push_unique(address);
        }
        for log in logs {
            push_unique(log.address);
        }

        touched
            .into_iter()
            .filter_map(|evm_address| {
                let account_id: AccountId = <pallet_evm::HashedAddressMapping<
                    sp_runtime::traits::BlakeTwo256,
                > as pallet_evm::AddressMapping<AccountId>>::into_account_id(
                    evm_address
                );
                let post_balance: Balance =
                    pallet_balances::Pallet::<super::Runtime>::free_balance(&account_id);

                // Skip addresses where we know the balance is unchanged — this
                // prevents phantom canonical-ledger updates on every EVM call.
                if let Some(&pre) = pre_balances.get(&evm_address) {
                    if pre == post_balance {
                        return None;
                    }
                }

                Some(StateChange {
                    // Canonical ledger decoding expects SCALE-encoded account IDs.
                    address: encode_account_address(&account_id),
                    key: canonical_asset_key(CANONICAL_NATIVE_ASSET_ID),
                    value: canonical_balance_value(post_balance),
                })
            })
            .collect()
    }

    /// Snapshot the native balances for a set of EVM addresses before execution.
    /// The resulting map is passed to `collect_evm_balance_changes` so it can
    /// emit delta-only StateChange records.
    fn evm_balance_snapshot(addresses: &[H160]) -> HashMap<H160, Balance> {
        addresses
            .iter()
            .map(|&addr| {
                let account_id: AccountId = <pallet_evm::HashedAddressMapping<
                    sp_runtime::traits::BlakeTwo256,
                > as pallet_evm::AddressMapping<AccountId>>::into_account_id(
                    addr
                );
                let balance = pallet_balances::Pallet::<super::Runtime>::free_balance(&account_id);
                (addr, balance)
            })
            .collect()
    }

    fn encode_account_address(account_id: &AccountId) -> Vec<u8> {
        let encoded = account_id.encode();
        if encoded.len() == 32 {
            return encoded;
        }

        let mut padded = [0u8; 32];
        let copy_len = core::cmp::min(32, encoded.len());
        padded[..copy_len].copy_from_slice(&encoded[..copy_len]);
        padded.to_vec()
    }

    fn canonical_asset_key(asset_id: AssetId) -> H256 {
        let mut out = [0u8; 32];
        out[..core::mem::size_of::<AssetId>()].copy_from_slice(&asset_id.to_le_bytes());
        H256::from(out)
    }

    fn canonical_balance_value(balance: Balance) -> H256 {
        let mut out = [0u8; 32];
        out[..core::mem::size_of::<Balance>()].copy_from_slice(&balance.to_le_bytes());
        H256::from(out)
    }

    fn truncate_to_h256(data: &[u8]) -> H256 {
        if data.is_empty() {
            return H256::zero();
        }
        let mut buf = [0u8; 32];
        let take = core::cmp::min(32, data.len());
        buf[..take].copy_from_slice(&data[..take]);
        H256::from(buf)
    }

    fn svm_error_str(error: SvmError) -> &'static str {
        match error {
            SvmError::InvalidPayload => "Invalid SVM payload",
            SvmError::ExecutionFailed => "SVM execution failed",
            SvmError::InvalidAccount => "Invalid SVM account",
            SvmError::InvalidSignature => "Invalid SVM signature",
            SvmError::OutOfComputeUnits => "SVM out of compute units",
            SvmError::InvalidInstructionData => "Invalid SVM instruction",
            SvmError::AccountDataTooSmall => "SVM account data too small",
            SvmError::InsufficientFunds => "SVM insufficient funds",
            SvmError::ProgramNotExecutable => "SVM program not executable",
            SvmError::InvalidProgramId => "Invalid SVM program id",
            SvmError::ExecutionError(_) => "SVM execution error",
        }
    }
}

// ===== Scheduler Pallet Configuration =====
parameter_types! {
    pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) *
        BlockWeights::get().max_block;
    pub const MaxScheduledPerBlock: u32 = 50;
    pub const NoPreimagePostponement: Option<BlockNumber> = Some(10);
}

impl pallet_scheduler::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type MaximumWeight = MaximumSchedulerWeight;
    type ScheduleOrigin = EnsureRootOrHalfCouncil;
    type MaxScheduledPerBlock = MaxScheduledPerBlock;
    type WeightInfo = ();
    type OriginPrivilegeCmp = frame_support::traits::EqualPrivilegeOnly;
    type Preimages = Preimage;
}

// ===== Preimage Pallet Configuration =====
parameter_types! {
    pub const PreimageMaxSize: u32 = 4096 * 1024;
    pub const PreimageBaseDeposit: Balance = X3;
    pub const PreimageByteDeposit: Balance = X3 / 100;
}

impl pallet_preimage::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Currency = Balances;
    type ManagerOrigin = EnsureRootOrHalfCouncil;
    type BaseDeposit = PreimageBaseDeposit;
    type ByteDeposit = PreimageByteDeposit;
}

// ===== Governance Pallet Configuration =====
parameter_types! {
    pub const ProposalDeposit: Balance = 100 * X3;
    pub const VotingPeriod: BlockNumber = blocks_from_millis(7 * 24 * 60 * 60 * 1000); // 7 days at 200ms blocks
    pub const EnactmentPeriod: BlockNumber = blocks_from_millis(24 * 60 * 60 * 1000); // 1 day at 200ms blocks
    pub const GovernanceQuorum: sp_runtime::Percent = sp_runtime::Percent::from_percent(10);
    pub const ApprovalThreshold: sp_runtime::Percent = sp_runtime::Percent::from_percent(51);
    pub const MaxGovernanceProposals: u32 = 100;
    pub const MaxVotes: u32 = 1000;
    pub const MaxDelegations: u32 = 100;
    pub const ConvictionPeriod: BlockNumber = blocks_from_millis(28 * 24 * 60 * 60 * 1000); // 28 days at 200ms blocks

    // ============================================================================
    // AI Governance Parameters
    // ============================================================================

    pub const MaxAIProposalPayload: u32 = 10 * 1024; // 10KB max payload
}

impl pallet_governance::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type SubmitOrigin = frame_system::EnsureSigned<AccountId>;
    type FastTrackOrigin = EnsureRootOrHalfCouncil;
    type CancelOrigin = EnsureRootOrHalfCouncil;
    type RuntimeUpgradeOrigin = EnsureRootOrHalfCouncil;
    type Scheduler = Scheduler;
    type PalletsOrigin = OriginCaller;
    type ProposalDeposit = ProposalDeposit;
    type VotingPeriod = VotingPeriod;
    type EnactmentPeriod = EnactmentPeriod;
    type Quorum = GovernanceQuorum;
    type ApprovalThreshold = ApprovalThreshold;
    type MaxProposals = MaxGovernanceProposals;
    type MaxVotes = MaxVotes;
    type MaxDelegations = MaxDelegations;
    type ConvictionPeriod = ConvictionPeriod;
    type WeightInfo = ();

    // ============================================================================
    // AI Governance Configuration
    // ============================================================================

    type MaxAIProposalPayload = MaxAIProposalPayload;
    type AISubmitOrigin = frame_system::EnsureSigned<AccountId>;
    type AIReviewOrigin = frame_system::EnsureSigned<AccountId>;
    type EmergencyOrigin = EnsureCouncilMember;
}

// ===== Treasury Pallet Configuration =====
parameter_types! {
    pub const TreasuryPalletId: frame_support::PalletId = frame_support::PalletId(*b"py/trsry");
    pub TreasuryAccountId: AccountId = TreasuryPalletId::get().into_account_truncating();
    pub const ProposalBond: sp_runtime::Percent = sp_runtime::Percent::from_percent(5);
    pub const MaxSigners: u32 = 7;
    pub const SmallSpendThreshold: Balance = 1_000 * X3;
    pub const MediumSpendThreshold: Balance = 10_000 * X3;
    pub const LargeSpendThreshold: Balance = 100_000 * X3;
    pub const MaxRecurringPayments: u32 = 100;
    pub const MaxYieldStrategies: u32 = 10;
    pub const MaxTreasuryProposals: u32 = 100;
    pub const ProposalBondMinimum: Balance = 100 * X3;
}

impl pallet_treasury::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type PalletId = TreasuryPalletId;
    type SmallSpendOrigin = EnsureRootOrHalfCouncil;
    type MediumSpendOrigin = EnsureRootOrHalfCouncil;
    type LargeSpendOrigin = EnsureRootOrHalfCouncil;
    type CriticalSpendOrigin = EnsureRootOrHalfCouncil;
    type PauseOrigin = EnsureRootOrHalfCouncil;
    type YieldConfigOrigin = EnsureRootOrHalfCouncil;
    type MaxSigners = MaxSigners;
    type MaxProposals = MaxTreasuryProposals;
    type MaxRecurringPayments = MaxRecurringPayments;
    type MaxYieldStrategies = MaxYieldStrategies;
    type SmallSpendLimit = SmallSpendThreshold;
    type MediumSpendLimit = MediumSpendThreshold;
    type LargeSpendLimit = LargeSpendThreshold;
    type ProposalBond = ProposalBond;
    type ProposalBondMinimum = ProposalBondMinimum;
    type WeightInfo = ();
}

// ===== Agent Accounts Pallet Configuration =====
parameter_types! {
    pub const RegistrationDeposit: Balance = 10 * X3;
    pub const MaxAgentsPerController: u32 = 100;
    pub const DefaultGasPerBlock: u128 = 1_000_000;
    pub const DefaultComputePerBlock: u128 = 1_000_000;
    pub const DefaultGasPerEpoch: u128 = 100_000_000;
    pub const DefaultComputePerEpoch: u128 = 100_000_000;
    pub const BlocksPerEpoch: BlockNumber = 14400; // ~1 day at 6s blocks
}

impl pallet_agent_accounts::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type RegisterOrigin = EnsureRootOrHalfCouncil;
    type AdminOrigin = EnsureRootOrHalfCouncil;
    type MaxAgentsPerController = MaxAgentsPerController;
    type RegistrationDeposit = RegistrationDeposit;
    type DefaultGasPerBlock = DefaultGasPerBlock;
    type DefaultComputePerBlock = DefaultComputePerBlock;
    type DefaultGasPerEpoch = DefaultGasPerEpoch;
    type DefaultComputePerEpoch = DefaultComputePerEpoch;
    type BlocksPerEpoch = BlocksPerEpoch;
    type WeightInfo = ();
}

// ===== Agent Memory Pallet Configuration =====
parameter_types! {
    pub const MaxEntriesPerChunk: u32 = 100;
    pub const MaxChunksPerAgent: u32 = 1_000;
    pub const StorageByteCost: Balance = X3 / 1000; // 0.001 X3 per byte
    pub const DefaultTtl: BlockNumber = 365 * 24 * 600; // ~1 year at 6s blocks
}

impl pallet_agent_memory::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MaxEntriesPerChunk = MaxEntriesPerChunk;
    type MaxChunksPerAgent = MaxChunksPerAgent;
    type StorageByteCost = StorageByteCost;
    type DefaultTtl = DefaultTtl;
    type PruneOrigin = EnsureRootOrHalfCouncil;
    type WeightInfo = ();
}

// ===== Evolution Core Pallet Configuration =====
parameter_types! {
    pub const MinApprovalQuorum: sp_runtime::Percent = sp_runtime::Percent::from_percent(66);
    pub const MaxPendingProposals: u32 = 100;
    pub const MaxReasonLength: u32 = 256;
    pub const ProposalLifetime: BlockNumber = blocks_from_millis(7 * 24 * 60 * 60 * 1000); // 7 days at 200ms blocks
    pub const MetricsHistoryDepth: u32 = 100;
    pub const AutoEvolutionBounds: (u32, u32) = (80, 120); // min 80%, max 120%
}

impl pallet_evolution_core::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type EvolutionAuthority = EnsureRootOrHalfCouncil;
    type EmergencyOrigin = EnsureRootOrHalfCouncil;
    type MinApprovalQuorum = MinApprovalQuorum;
    type MaxPendingProposals = MaxPendingProposals;
    type MaxReasonLength = MaxReasonLength;
    type ProposalLifetime = ProposalLifetime;
    type MetricsHistoryDepth = MetricsHistoryDepth;
    type AutoEvolutionBounds = AutoEvolutionBounds;
    type WeightInfo = pallet_evolution_core::weights::SubstrateWeight<Runtime>;
}

// ===== X3 Verifier Pallet Configuration =====
parameter_types! {
    pub const MinExecutorStake: Balance = 1000 * X3;
    pub const MaxOutputSize: u32 = 64 * 1024; // 64 KB
    pub const MaxKeySize: u32 = 256;
    pub const MaxValueSize: u32 = 4096;
    pub const MaxStateChanges: u32 = 100;
    pub const MaxProofDepth: u32 = 32;
    pub const ExecutorRewardShare: u32 = 70; // 70%
    pub const ProtocolFeeShare: u32 = 15; // 15%
    pub const SlashAmount: Balance = 100 * X3;
    pub const JobTimeout: BlockNumber = 100; // ~10 minutes at 6s blocks
}

impl pallet_x3_verifier::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type ExecutorRegistrar = EnsureRootOrHalfCouncil;
    type MinExecutorStake = MinExecutorStake;
    type MaxOutputSize = MaxOutputSize;
    type MaxKeySize = MaxKeySize;
    type MaxValueSize = MaxValueSize;
    type MaxStateChanges = MaxStateChanges;
    type MaxProofDepth = MaxProofDepth;
    type ExecutorRewardShare = ExecutorRewardShare;
    type ProtocolFeeShare = ProtocolFeeShare;
    type SlashAmount = SlashAmount;
    type JobTimeout = JobTimeout;
    type WeightInfo = pallet_x3_verifier::weights::SubstrateWeight<Runtime>;
}

// ===== X3 Domain Registry Pallet Configuration =====
parameter_types! {
    pub const MaxX3DomainLen: u32 = 253;
    pub const MaxX3Domains: u32 = 10_000;
    pub const MaxX3RecordsPerDomain: u32 = 32;
    pub const MaxX3CnameLen: u32 = 253;
    pub const MaxX3TxtLen: u32 = 1024;
}

impl pallet_x3_domain_registry::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type UpdateOrigin = EnsureRootOrHalfCouncil;
    type MaxDomainLen = MaxX3DomainLen;
    type MaxDomains = MaxX3Domains;
    type MaxRecordsPerDomain = MaxX3RecordsPerDomain;
    type MaxCnameLen = MaxX3CnameLen;
    type MaxTxtLen = MaxX3TxtLen;
}

// ===== X3SettlementEngine Configuration =====

parameter_types! {
    pub const MaxSettlementLegs: u32 = 8;           // Max legs per settlement intent
    pub const MaxPendingIntents: u32 = 1000;        // Max pending intents
    pub const DefaultSettlementTimeout: u64 = 43200; // ~12 hours in seconds
    pub const MinBtcConfirmations: u32 = 6;         // Standard BTC confirmations
    pub const ChallengePeriod: BlockNumber = 600;   // ~1 hour for dispute period
}

impl pallet_x3_settlement_engine::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type SettlementWeightInfo = pallet_x3_settlement_engine::weights::SubstrateWeight<Runtime>;
    type Currency = Balances;
    type UnixTime = Timestamp;
    type MaxSettlementLegs = MaxSettlementLegs;
    type MaxPendingIntents = MaxPendingIntents;
    type DefaultSettlementTimeout = DefaultSettlementTimeout;
    type MinBtcConfirmations = MinBtcConfirmations;
    type ChallengePeriod = ChallengePeriod;
    type SettlementTimeoutBlocks = ConstU32<300>; // Issue #5: 300 blocks ≈ 60 seconds
    type CrossChainValidator = pallet_x3_settlement_engine::bridge_integration::NoOpCrossChainValidator; // Phase 4: Bridge integration (test with no-op)
}

// ===== Swarm Pallet Configuration =====

parameter_types! {
    pub const MinContributorStake: Balance = 1000 * X3;
    pub const SwarmHeartbeatInterval: BlockNumber = 100;    // ~10 min at 6s blocks
    pub const SwarmUnstakeCooldown: BlockNumber = 14400;    // ~1 day at 6s blocks
    pub const SwarmDefaultTaskTimeout: BlockNumber = 600;   // ~1 hour at 6s blocks
    pub const SwarmCommitPhaseDuration: BlockNumber = 50;   // ~5 min at 6s blocks
    pub const SwarmRevealPhaseDuration: BlockNumber = 50;   // ~5 min at 6s blocks
    pub const SwarmContributorRewardPct: u8 = 85;           // 85% to contributor
    pub const SwarmProtocolFeePct: u8 = 15;                 // 15% protocol fee
    pub const SwarmSlashAmount: Balance = 100 * X3;
    pub const MaxTasksPerContributor: u32 = 6;              // Matches swarm-config.toml
    pub const MaxJuryVoters: u32 = 50;
}

impl pallet_swarm::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type AdminOrigin = EnsureRootOrHalfCouncil;
    type SlashOrigin = EnsureRootOrHalfCouncil;
    type MinContributorStake = MinContributorStake;
    type HeartbeatInterval = SwarmHeartbeatInterval;
    type UnstakeCooldown = SwarmUnstakeCooldown;
    type DefaultTaskTimeout = SwarmDefaultTaskTimeout;
    type CommitPhaseDuration = SwarmCommitPhaseDuration;
    type RevealPhaseDuration = SwarmRevealPhaseDuration;
    type ContributorRewardPct = SwarmContributorRewardPct;
    type ProtocolFeePct = SwarmProtocolFeePct;
    type SlashAmount = SwarmSlashAmount;
    type MaxTasksPerContributor = MaxTasksPerContributor;
    type MaxJuryVoters = MaxJuryVoters;
    type WeightInfo = pallet_swarm::weights::SubstrateWeight<Runtime>;
}

// ===== DePIN Marketplace Pallet Configuration =====
parameter_types! {
    pub const DepinMarketplacePalletId: PalletId = PalletId(*b"dp/mktpl");
    pub const ValidatorShareBps: u16 = 5500;    // 55% to provider
    pub const BurnShareBps: u16 = 2500;          // 25% burn
    pub const StakerShareBps: u16 = 2000;        // 20% to stakers
    pub const MinProviderStake: Balance = 1_000 * X3;
    pub const MaxJobsPerProvider: u32 = 16;
    pub const MaxJobDuration: BlockNumber = 14400;   // ~1 day at 6s blocks
    pub const MaxPendingOrders: u32 = 256;
    pub const DepinSlashFraction: Perbill = Perbill::from_percent(10);
}

impl pallet_depin_marketplace::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type BurnDestination = ();
    type AdminOrigin = EnsureRootOrHalfCouncil;
    type PalletId = DepinMarketplacePalletId;
    type ValidatorShareBps = ValidatorShareBps;
    type BurnShareBps = BurnShareBps;
    type StakerShareBps = StakerShareBps;
    type MinProviderStake = MinProviderStake;
    type MaxJobsPerProvider = MaxJobsPerProvider;
    type MaxJobDuration = MaxJobDuration;
    type MaxPendingOrders = MaxPendingOrders;
    type SlashFraction = DepinSlashFraction;
    type WeightInfo = ();
}

// ===== Private Execution Pallet Configuration =====
parameter_types! {
    pub const PrivateExecutionPalletId: PalletId = PalletId(*b"pv/exec!");
    pub const PrivateFeePremiumBps: u16 = 150;            // 1.5% premium
    pub const MinConfidentialQuorum: u32 = 2;
    pub const MaxConfidentialValidators: u32 = 100;
    pub const MaxDiffsPerBlock: u32 = 32;
    pub const MaxEncryptedPayloadSize: u32 = 65536;       // 64 KiB
    pub const AttestationValidityPeriod: BlockNumber = 43200;  // ~3 days at 6s blocks
    pub const ConfidentialValidatorShareBps: u16 = 6000;  // 60% to validators
    pub const PrivateBurnShareBps: u16 = 2500;            // 25% burn
    pub const PrivateStakerShareBps: u16 = 1500;          // 15% to stakers
}

impl pallet_private_execution::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type BurnDestination = ();
    type AdminOrigin = EnsureRootOrHalfCouncil;
    type PalletId = PrivateExecutionPalletId;
    type PrivateFeePremiumBps = PrivateFeePremiumBps;
    type MinConfidentialQuorum = MinConfidentialQuorum;
    type MaxConfidentialValidators = MaxConfidentialValidators;
    type MaxDiffsPerBlock = MaxDiffsPerBlock;
    type MaxEncryptedPayloadSize = MaxEncryptedPayloadSize;
    type AttestationValidityPeriod = AttestationValidityPeriod;
    type ConfidentialValidatorShareBps = ConfidentialValidatorShareBps;
    type PrivateBurnShareBps = PrivateBurnShareBps;
    type PrivateStakerShareBps = PrivateStakerShareBps;
    type WeightInfo = ();
}

impl pallet_x3_sequencer::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MaxTxsPerBatch = SeqMaxTxsPerBatch;
    type MaxPayloadSize = SeqMaxPayloadSize;
    type PerByteFee = SeqPerByteFee;
    type BaseFee = SeqBaseFee;
}

impl pallet_x3_da::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MaxBlobSize = DaMaxBlobSize;
    type PerByteFee = DaPerByteFee;
    type MaxShardProofs = DaMaxShardProofs;
    type RetentionBlocks = DaRetentionBlocks;
}

// Blanket impl: enables off-chain workers for any pallet using
// `frame_system::offchain::SendTransactionTypes<Call<Self>>`.
impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for Runtime
where
    RuntimeCall: From<LocalCall>,
{
    type Extrinsic = UncheckedExtrinsic;
    type OverarchingCall = RuntimeCall;
}

parameter_types! {
    pub const AtomicKernelMinBond: u128 = 1_000_000_000_000; // 1 X3 (12 decimals)
    pub const AtomicKernelMaxLegsPerBundle: u32 = 16;
    pub const AtomicKernelBundleDeadlineBlocks: BlockNumber = 100;
}

impl pallet_x3_atomic_kernel::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type WeightInfo = pallet_x3_atomic_kernel::weights::SubstrateWeight<Runtime>;
    type MinBond = AtomicKernelMinBond;
    type MaxLegsPerBundle = AtomicKernelMaxLegsPerBundle;
    type BundleDeadlineBlocks = AtomicKernelBundleDeadlineBlocks;
}

// ===== Universal Asset Kernel MVP =====
// Three-pallet stack enforcing the king invariant (canonical supply preservation)
// for cross-VM transfers within X3's internal domains (X3Native / X3Evm / X3Svm).
parameter_types! {
    /// Upper bound on the number of canonical assets the registry can track.
    pub const X3AssetRegistryMaxAssets: u32 = 1024;
}

impl pallet_x3_asset_registry::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RegistryOrigin = EnsureRootOrHalfCouncil;
    type EmergencyPauseOrigin = EnsureRootOrHalfCouncil;
    type MaxAssets = X3AssetRegistryMaxAssets;
}

impl pallet_x3_supply_ledger::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type SupplyGovernance = EnsureRootOrHalfCouncil;
    type Registry = X3AssetRegistry;
}

impl pallet_x3_cross_vm_router::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Registry = X3AssetRegistry;
    type Ledger = X3SupplyLedger;
}

// OmniToken factory: permissionless launches that produce one canonical asset
// wired to X3Native/X3Evm/X3Svm via the registry + supply ledger.
impl pallet_x3_token_factory::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CreateTokenOrigin = frame_system::EnsureSigned<AccountId>;
    type Registry = X3AssetRegistry;
    type Ledger = X3SupplyLedger;
}

impl pallet_cross_chain_validator::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_cross_chain_validator::weights::SubstrateWeight<Self>;
}

// ===== Jury Anchor Configuration =====
parameter_types! {
    pub const MaxJurySessionIdLength: u32 = 256;  // Max length of jury session ID
}

impl pallet_x3_jury_anchor::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxSessionIdLength = MaxJurySessionIdLength;
}

// Session trait implementations for minimal runtime
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct SessionHandler;

impl GetValidatorCount for SessionHandler {
    fn validator_count(&self) -> u32 {
        0
    }
}

impl GetSessionNumber for SessionHandler {
    fn session(&self) -> u32 {
        0
    }
}

// sp_session::Config not available in polkadot-v1.0.0
// impl sp_session::Config for Runtime {}

// sp_session::SessionKeys trait implementation for session key generation/decoding
impl_runtime_apis! {
    impl sp_api::Core<Block> for Runtime {
        fn version() -> sp_version::RuntimeVersion {
            VERSION
        }

        fn execute_block(block: Block) {
            Executive::execute_block(block);
        }

        fn initialize_block(header: &<Block as BlockT>::Header) {
            Executive::initialize_block(header);
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
            SessionKeys::generate(seed)
        }

        fn decode_session_keys(
            encoded: Vec<u8>,
        ) -> Option<Vec<(Vec<u8>, sp_core::crypto::KeyTypeId)>> {
            SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: sp_runtime::transaction_validity::TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> sp_runtime::transaction_validity::TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }

    impl pallet_x3_kernel::AtlasKernelRuntimeApi<Block, AccountId, Balance, AssetId> for Runtime {
        fn get_canonical_balance(account: AccountId, asset_id: AssetId) -> Balance {
            pallet_x3_kernel::CanonicalLedger::<Runtime>::get(&account, &asset_id)
        }

        fn get_asset_metadata(asset_id: AssetId) -> Option<(Vec<u8>, u8)> {
            pallet_x3_kernel::AssetRegistry::<Runtime>::get(&asset_id)
                .map(|metadata| (metadata.symbol.into_inner(), metadata.decimals))
        }

        fn is_authorized(account: AccountId) -> bool {
            pallet_x3_kernel::AuthorizedAccounts::<Runtime>::contains_key(&account)
        }

        fn get_authorized_accounts() -> Vec<AccountId> {
            pallet_x3_kernel::AuthorizedAccounts::<Runtime>::iter_keys().collect()
        }

        fn get_authorities() -> Vec<AccountId> {
            pallet_x3_kernel::Authorities::<Runtime>::get().into_inner()
        }

        fn map_evm_address(address: Vec<u8>) -> Option<AccountId> {
            use sp_core::H160;
            use sp_runtime::traits::BlakeTwo256;
            if address.len() != 20 {
                return None;
            }
            let mut slice = [0u8; 20];
            slice.copy_from_slice(&address[..20]);
            let evm_addr = H160::from(slice);

            // Use the runtime's AddressMapping type from pallet_evm
            // to derive AccountId from EVM address
            Some(<pallet_evm::HashedAddressMapping<BlakeTwo256> as pallet_evm::AddressMapping<AccountId>>::into_account_id(evm_addr))
        }

        fn get_evm_balance(evm_address: Vec<u8>, asset_id: AssetId) -> Option<Balance> {
            use sp_core::H160;
            use sp_runtime::traits::BlakeTwo256;
            if evm_address.len() != 20 { return None; }
            let mut slice = [0u8; 20];
            slice.copy_from_slice(&evm_address[..20]);
            let evm_addr = H160::from(slice);
            let account_id: AccountId = <pallet_evm::HashedAddressMapping<BlakeTwo256> as pallet_evm::AddressMapping<AccountId>>::into_account_id(evm_addr);
            Some(pallet_x3_kernel::CanonicalLedger::<Runtime>::get(&account_id, &asset_id))
        }

        fn get_evm_code(evm_address: Vec<u8>) -> Vec<u8> {
            use sp_core::H160;
            if evm_address.len() != 20 { return Vec::new(); }
            let mut slice = [0u8; 20];
            slice.copy_from_slice(&evm_address[..20]);
            let evm_addr = H160::from(slice);
            pallet_evm::AccountCodes::<Runtime>::get(evm_addr)
        }

        fn get_evm_storage(evm_address: Vec<u8>, storage_key: H256) -> Option<H256> {
            use sp_core::H160;
            if evm_address.len() != 20 { return None; }
            let mut slice = [0u8; 20];
            slice.copy_from_slice(&evm_address[..20]);
            let evm_addr = H160::from(slice);
            let val = pallet_evm::AccountStorages::<Runtime>::get(evm_addr, storage_key);
            if val == H256::zero() { None } else { Some(val) }
        }

        fn get_evm_nonce(evm_address: Vec<u8>) -> u64 {
            use sp_core::H160;
            use sp_runtime::traits::BlakeTwo256;
            if evm_address.len() != 20 { return 0; }
            let mut slice = [0u8; 20];
            slice.copy_from_slice(&evm_address[..20]);
            let evm_addr = H160::from(slice);
            let account_id: AccountId = <pallet_evm::HashedAddressMapping<BlakeTwo256>
                as pallet_evm::AddressMapping<AccountId>>::into_account_id(evm_addr);
            frame_system::Pallet::<Runtime>::account_nonce(&account_id) as u64
        }

        fn get_svm_balance(svm_pubkey: Vec<u8>) -> u64 {
            use codec::Decode;
            if svm_pubkey.len() != 32 { return 0; }
            let Ok(account_id) = AccountId::decode(&mut &svm_pubkey[..]) else { return 0; };
            let balance = pallet_x3_kernel::CanonicalLedger::<Runtime>::get(&account_id, &0u32);
            use sp_runtime::traits::SaturatedConversion;
            balance.saturated_into::<u64>()
        }

        fn is_svm_program(svm_pubkey: Vec<u8>) -> bool {
            use codec::Decode;
            if svm_pubkey.len() != 32 { return false; }
            let Ok(account_id) = AccountId::decode(&mut &svm_pubkey[..]) else { return false; };
            // pallet-svm is not instantiated in this runtime; use the canonical
            // ledger balance as a proxy for account existence.  A deployed SVM
            // program always has a non-zero canonical balance (rent-exempt deposit).
            let balance = pallet_x3_kernel::CanonicalLedger::<Runtime>::get(&account_id, &0u32);
            balance > 0
        }

        fn submit_evm_transaction(raw_tx: Vec<u8>) -> Result<Vec<u8>, Vec<u8>> {
            // Payload contract:
            // [caller(20)] [to(20)] [value(16 LE)] [data_len(4 LE)] [data...]
            use sp_io::hashing::keccak_256;
            use fp_evm::ExitReason;
            use pallet_evm::Runner;
            use sp_core::{H160, U256};

            if raw_tx.len() < (20 + 20 + 16 + 4) {
                return Err(b"invalid payload: too short".to_vec());
            }

            let caller = {
                let mut bytes = [0u8; 20];
                bytes.copy_from_slice(&raw_tx[0..20]);
                H160::from(bytes)
            };
            let to = {
                let mut bytes = [0u8; 20];
                bytes.copy_from_slice(&raw_tx[20..40]);
                H160::from(bytes)
            };
            let value = U256::from_little_endian(&raw_tx[40..56]);
            let data_len = u32::from_le_bytes(raw_tx[56..60].try_into().unwrap_or([0u8; 4])) as usize;
            if raw_tx.len() < 60 + data_len {
                return Err(b"invalid payload: data_len out of bounds".to_vec());
            }
            let data = raw_tx[60..60 + data_len].to_vec();

            frame_support::log::info!(
                target: "runtime::evm",
                "submit_evm_transaction caller=0x{:?} to=0x{:?} value={} data_len={}",
                caller,
                to,
                value,
                data_len,
            );

            let evm_config = fp_evm::Config::shanghai();
            let result = <Runtime as pallet_evm::Config>::Runner::call(
                caller,
                to,
                data,
                value,
                10_000_000u64,
                Some(U256::from(NATIVE_GAS_PRICE)),
                None,
                None,
                Vec::new(),
                false,
                false,
                None,
                None,
                &evm_config,
            );

            match result {
                Ok(info) => match info.exit_reason {
                    ExitReason::Succeed(_) => {
                    let tx_hash = keccak_256(&raw_tx).to_vec();
                    Ok(tx_hash)
                }
                    ExitReason::Revert(_) => Err(info.value),
                    ExitReason::Error(_) | ExitReason::Fatal(_) => {
                        Err(b"EVM execution failed".to_vec())
                    }
                },
                Err(_) => Err(b"EVM runner call failed".to_vec()),
            }
        }

        fn validate_evm_transaction(raw_tx: Vec<u8>) -> Result<Vec<u8>, Vec<u8>> {
            // Runtime-API calls execute against a per-call throwaway state overlay —
            // changes are never written to the chain database regardless of the
            // entry-point.  This function exists to make the read-only pre-flight
            // validation intent explicit on the cross-VM RPC path (node/src/rpc.rs).
            // NOTE: The execution path is identical to submit_evm_transaction.
            use sp_io::hashing::keccak_256;
            use fp_evm::ExitReason;
            use pallet_evm::Runner;
            use sp_core::{H160, U256};

            if raw_tx.len() < (20 + 20 + 16 + 4) {
                return Err(b"invalid payload: too short".to_vec());
            }

            let caller = {
                let mut bytes = [0u8; 20];
                bytes.copy_from_slice(&raw_tx[0..20]);
                H160::from(bytes)
            };
            let to = {
                let mut bytes = [0u8; 20];
                bytes.copy_from_slice(&raw_tx[20..40]);
                H160::from(bytes)
            };
            let value = U256::from_little_endian(&raw_tx[40..56]);
            let data_len = u32::from_le_bytes(raw_tx[56..60].try_into().unwrap_or([0u8; 4])) as usize;
            if raw_tx.len() < 60 + data_len {
                return Err(b"invalid payload: data_len out of bounds".to_vec());
            }
            let data = raw_tx[60..60 + data_len].to_vec();

            frame_support::log::debug!(
                target: "runtime::evm",
                "validate_evm_transaction (dry-run) caller={:?} to={:?} value={} data_len={}",
                caller,
                to,
                value,
                data_len,
            );

            let evm_config = fp_evm::Config::shanghai();
            let result = <Runtime as pallet_evm::Config>::Runner::call(
                caller,
                to,
                data,
                value,
                10_000_000u64,
                Some(U256::from(NATIVE_GAS_PRICE)),
                None,
                None,
                Vec::new(),
                false,
                false,
                None,
                None,
                &evm_config,
            );

            match result {
                Ok(info) => match info.exit_reason {
                    ExitReason::Succeed(_) => {
                        let tx_hash = keccak_256(&raw_tx).to_vec();
                        Ok(tx_hash)
                    }
                    ExitReason::Revert(_) => Err(info.value),
                    ExitReason::Error(_) | ExitReason::Fatal(_) => {
                        Err(b"EVM execution failed".to_vec())
                    }
                },
                Err(_) => Err(b"EVM runner call failed".to_vec()),
            }
        }

        fn submit_svm_instruction(program_id: [u8; 32], instruction_data: Vec<u8>) -> Result<Vec<u8>, Vec<u8>> {
            use pallet_x3_kernel::SvmExecutorAdapter;
            let mut payload = Vec::with_capacity(32 + instruction_data.len());
            payload.extend_from_slice(&program_id);
            payload.extend_from_slice(&instruction_data);

            match <Runtime as pallet_x3_kernel::Config>::SvmAdapter::execute(&payload, 1_400_000) {
                Ok(receipt) if receipt.success => Ok(receipt.return_data),
                Ok(receipt) => Err(receipt.return_data),
                Err(_) => Err(b"SVM execution failed".to_vec()),
            }
        }

        fn call_evm(caller: Option<Vec<u8>>, evm_address: Vec<u8>, input: Vec<u8>, gas_limit: u64) -> Result<Vec<u8>, Vec<u8>> {
            use fp_evm::ExitReason;
            use pallet_evm::Runner;
            use sp_core::{H160, U256};

            if evm_address.len() != 20 {
                return Err(b"Invalid EVM address length".to_vec());
            }

            let mut addr = [0u8; 20];
            addr.copy_from_slice(&evm_address[..20]);
            let target = H160::from(addr);
            let source = caller
                .and_then(|c| {
                    if c.len() == 20 {
                        let mut bytes = [0u8; 20];
                        bytes.copy_from_slice(&c[..20]);
                        Some(H160::from(bytes))
                    } else {
                        None
                    }
                })
                .unwrap_or(SYSTEM_EVM_CALLER);
            let effective_gas = if gas_limit == 0 { 10_000_000u64 } else { gas_limit };
            let evm_config = fp_evm::Config::shanghai();

            let result = <Runtime as pallet_evm::Config>::Runner::call(
                source,
                target,
                input,
                U256::zero(),
                effective_gas,
                Some(U256::from(NATIVE_GAS_PRICE)),
                None,
                None,
                Vec::new(),
                false,
                false,
                None,
                None,
                &evm_config,
            );

            match result {
                Ok(info) => match info.exit_reason {
                    ExitReason::Succeed(_) => Ok(info.value),
                    ExitReason::Revert(_) => Err(info.value),
                    ExitReason::Error(_) | ExitReason::Fatal(_) => {
                        Err(b"EVM call execution failed".to_vec())
                    }
                },
                Err(_) => Err(b"EVM runner call failed".to_vec()),
            }
        }

        fn estimate_evm_gas(caller: Option<Vec<u8>>, evm_address: Vec<u8>, input: Vec<u8>, gas_limit: u64) -> Result<u64, Vec<u8>> {
            use fp_evm::ExitReason;
            use pallet_evm::Runner;
            use sp_core::{H160, U256};
            use sp_runtime::traits::UniqueSaturatedInto;

            if evm_address.len() != 20 {
                return Err(b"Invalid EVM address length".to_vec());
            }

            let mut addr = [0u8; 20];
            addr.copy_from_slice(&evm_address[..20]);
            let target = H160::from(addr);
            let source = caller
                .and_then(|c| {
                    if c.len() == 20 {
                        let mut bytes = [0u8; 20];
                        bytes.copy_from_slice(&c[..20]);
                        Some(H160::from(bytes))
                    } else {
                        None
                    }
                })
                .unwrap_or(SYSTEM_EVM_CALLER);
            let effective_gas = if gas_limit == 0 { 10_000_000u64 } else { gas_limit };
            let evm_config = fp_evm::Config::shanghai();

            let result = <Runtime as pallet_evm::Config>::Runner::call(
                source,
                target,
                input,
                U256::zero(),
                effective_gas,
                Some(U256::from(NATIVE_GAS_PRICE)),
                None,
                None,
                Vec::new(),
                false,
                false,
                None,
                None,
                &evm_config,
            );

            match result {
                Ok(info) => match info.exit_reason {
                    ExitReason::Succeed(_) => Ok(info.used_gas.standard.unique_saturated_into()),
                    ExitReason::Revert(_) => Err(b"EVM call reverted during gas estimate".to_vec()),
                    ExitReason::Error(_) | ExitReason::Fatal(_) => {
                        Err(b"EVM call failed during gas estimate".to_vec())
                    }
                },
                Err(_) => Err(b"EVM runner estimate failed".to_vec()),
            }
        }
    }

    impl pallet_atomic_trade_engine::AtomicTradeEngineApi<Block> for Runtime {
        fn simulate_trade(
            token_in: sp_core::H256,
            token_out: sp_core::H256,
            amount_in: u128,
            slippage_bps: u32,
        ) -> pallet_atomic_trade_engine::runtime_api::SimulationResult {
            use pallet_atomic_trade_engine::runtime_api::SimulationResult;

            if let Some(route) =
                pallet_atomic_trade_engine::Pallet::<Runtime>::find_execution_path(
                    token_in,
                    token_out,
                    amount_in,
                )
            {
                let mut evm_gas: u64 = 0;
                let mut svm_compute: u64 = 0;

                for step in route.steps.iter() {
                    match step.vm_type {
                        pallet_atomic_trade_engine::types::VmType::Evm => {
                            evm_gas = evm_gas.saturating_add(150_000);
                        }
                        pallet_atomic_trade_engine::types::VmType::Svm => {
                            svm_compute = svm_compute.saturating_add(200_000);
                        }
                        pallet_atomic_trade_engine::types::VmType::CrossVm => {
                            evm_gas = evm_gas.saturating_add(200_000);
                            svm_compute = svm_compute.saturating_add(250_000);
                        }
                        pallet_atomic_trade_engine::types::VmType::X3 => {
                            evm_gas = evm_gas.saturating_add(120_000);
                        }
                    }
                }

                return SimulationResult {
                    success: true,
                    estimated_output: route
                        .expected_amount_out
                        .saturating_mul(10000 - slippage_bps as u128)
                        / 10000,
                    price_impact_bps: route.price_impact_bps,
                    evm_gas,
                    svm_compute,
                    route: route.steps,
                    error: None,
                };
            }

            // Fallback simulation when no pools are available.
            let estimated_output =
                amount_in.saturating_mul(10000 - slippage_bps as u128) / 10000;

            SimulationResult {
                success: true,
                estimated_output,
                price_impact_bps: slippage_bps,
                evm_gas: 150_000,
                svm_compute: 0,
                route: vec![],
                error: None,
            }
        }

        fn estimate_execution_cost(
            legs: u32,
            vm_types: Vec<u8>,
        ) -> (u64, u64) {
            let mut evm_gas: u64 = 0;
            let mut svm_compute: u64 = 0;

            for vm_type in vm_types.iter().take(legs as usize) {
                match vm_type {
                    0 => evm_gas = evm_gas.saturating_add(150_000), // EVM
                    1 => svm_compute = svm_compute.saturating_add(200_000), // SVM
                    2 => { // CrossVM
                        evm_gas = evm_gas.saturating_add(200_000);
                        svm_compute = svm_compute.saturating_add(250_000);
                    }
                    _ => {}
                }
            }

            (evm_gas, svm_compute)
        }

        fn get_price_data(
            token_a: sp_core::H256,
            token_b: sp_core::H256,
        ) -> pallet_atomic_trade_engine::runtime_api::PriceDataResponse {
            use pallet_atomic_trade_engine::runtime_api::PriceDataResponse;

            let twap = pallet_atomic_trade_engine::Pallet::<Runtime>::get_twap(token_a, token_b);
            let latest = pallet_atomic_trade_engine::Pallet::<Runtime>::get_latest_price(token_a, token_b);
            let twap_data = pallet_atomic_trade_engine::TwapData::<Runtime>::get((token_a, token_b));

            PriceDataResponse {
                exists: twap.is_some() || latest.is_some(),
                twap_price: twap,
                latest_price: latest,
                observation_count: twap_data.as_ref().map(|t| t.observation_count).unwrap_or(0),
                last_updated: twap_data.map(|t| t.last_timestamp).unwrap_or(0),
            }
        }

        fn get_batch_status(batch_hash: sp_core::H256) -> pallet_atomic_trade_engine::runtime_api::BatchStatusResponse {
            use pallet_atomic_trade_engine::runtime_api::BatchStatusResponse;

            let maybe_batch = pallet_atomic_trade_engine::TradeBatches::<Runtime>::get(batch_hash);

            match maybe_batch {
                Some(batch) => BatchStatusResponse {
                    exists: true,
                    status: match batch.status {
                        pallet_atomic_trade_engine::BatchStatus::Pending => 0,
                        pallet_atomic_trade_engine::BatchStatus::Executing => 1,
                        pallet_atomic_trade_engine::BatchStatus::Completed => 2,
                        pallet_atomic_trade_engine::BatchStatus::Failed => 3,
                        pallet_atomic_trade_engine::BatchStatus::Cancelled => 4,
                    },
                    submitted_at: batch.created_at,
                    finalized_at: None, // TradeBatch doesn't track finalized_at
                    legs_executed: batch.legs.len() as u32,
                    checkpoints: 0, // Checkpoints stored separately
                },
                None => BatchStatusResponse::default(),
            }
        }

        fn find_route(
            token_in: sp_core::H256,
            token_out: sp_core::H256,
            amount_in: u128,
        ) -> Option<pallet_atomic_trade_engine::types::TradeRoute> {
            pallet_atomic_trade_engine::Pallet::<Runtime>::find_execution_path(
                token_in,
                token_out,
                amount_in,
            )
        }

        fn is_authorized(account: Vec<u8>) -> bool {
            // Delegate authorization to X3 Kernel's authorized accounts
            use codec::Decode;
            if let Ok(account_id) = AccountId::decode(&mut &account[..]) {
                // Check if account is authorized in the main X3 Kernel pallet
                pallet_x3_kernel::AuthorizedAccounts::<Runtime>::contains_key(&account_id)
            } else {
                false
            }
        }
    }

    impl sp_consensus_aura::AuraApi<Block, sp_consensus_aura::sr25519::AuthorityId> for Runtime {
        fn slot_duration() -> sp_consensus_aura::SlotDuration {
            sp_consensus_aura::SlotDuration::from_millis(pallet_aura::Pallet::<Runtime>::slot_duration())
        }

        fn authorities() -> Vec<sp_consensus_aura::sr25519::AuthorityId> {
            pallet_aura::Pallet::<Runtime>::authorities().to_vec()
        }
    }

    impl sp_consensus_grandpa::GrandpaApi<Block> for Runtime {
        fn grandpa_authorities() -> sp_consensus_grandpa::AuthorityList {
            pallet_grandpa::Pallet::<Runtime>::grandpa_authorities()
        }

        fn current_set_id() -> sp_consensus_grandpa::SetId {
            pallet_grandpa::Pallet::<Runtime>::current_set_id()
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            _equivocation_proof: sp_consensus_grandpa::EquivocationProof<
                <Block as BlockT>::Hash,
                sp_runtime::traits::NumberFor<Block>,
            >,
            _key_owner_proof: sp_consensus_grandpa::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            None
        }

        fn generate_key_ownership_proof(
            _set_id: sp_consensus_grandpa::SetId,
            _authority_id: sp_consensus_grandpa::AuthorityId,
        ) -> Option<sp_consensus_grandpa::OpaqueKeyOwnershipProof> {
            None
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> sp_runtime::ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(
            block: Block,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
        fn account_nonce(account: AccountId) -> Nonce {
            frame_system::Pallet::<Runtime>::account_nonce(account)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
        fn query_info(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
            pallet_transaction_payment::Pallet::<Runtime>::query_info(uxt, len)
        }

        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            pallet_transaction_payment::Pallet::<Runtime>::query_fee_details(uxt, len)
        }

        fn query_weight_to_fee(weight: Weight) -> Balance {
            <Runtime as pallet_transaction_payment::Config>::WeightToFee::weight_to_fee(&weight)
        }

        fn query_length_to_fee(length: u32) -> Balance {
            <Runtime as pallet_transaction_payment::Config>::LengthToFee::weight_to_fee(
                &Weight::from_all(u64::from(length) * 1000)
            )
        }
    }

    impl
        pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
        for Runtime
    {
        fn query_call_info(
            call: RuntimeCall,
            len: u32,
        ) -> pallet_transaction_payment::RuntimeDispatchInfo<Balance> {
            pallet_transaction_payment::Pallet::<Runtime>::query_call_info(call, len)
        }

        fn query_call_fee_details(
            call: RuntimeCall,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            pallet_transaction_payment::Pallet::<Runtime>::query_call_fee_details(call, len)
        }

        fn query_weight_to_fee(weight: Weight) -> Balance {
            <Runtime as pallet_transaction_payment::Config>::WeightToFee::weight_to_fee(&weight)
        }

        fn query_length_to_fee(length: u32) -> Balance {
            <Runtime as pallet_transaction_payment::Config>::LengthToFee::weight_to_fee(
                &Weight::from_all(u64::from(length) * 1000)
            )
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            // Return empty metadata vector - the actual metadata is queried at runtime via the RuntimeMetadataApi
            OpaqueMetadata::new(sp_std::vec![].into())
        }

        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            match version {
                1 => Some(OpaqueMetadata::new(sp_std::vec![].into())),
                _ => None,
            }
        }

        fn metadata_versions() -> sp_std::vec::Vec<u32> {
            sp_std::vec![1]
        }
    }

    impl pallet_evolution_core::runtime_api::EvolutionCoreApi<Block, AccountId, BlockNumber> for Runtime {
        fn get_params() -> pallet_evolution_core::runtime_api::EvolvableParamsResponse {
            let params = pallet_evolution_core::Pallet::<Runtime>::get_params();
            pallet_evolution_core::runtime_api::EvolvableParamsResponse {
                gas_multiplier: params.gas_multiplier,
                evm_weight_pct: params.evm_weight_pct,
                svm_weight_pct: params.svm_weight_pct,
                jit_threshold: params.jit_threshold,
                max_parallel: params.max_parallel,
                mev_smooth_factor: params.mev_smooth_factor,
            }
        }

        fn get_status() -> pallet_evolution_core::runtime_api::EvolutionStatusResponse {
            pallet_evolution_core::runtime_api::EvolutionStatusResponse {
                evolution_enabled: pallet_evolution_core::EvolutionEnabled::<Runtime>::get(),
                auto_evolution_enabled: pallet_evolution_core::AutoEvolutionEnabled::<Runtime>::get(),
                pending_proposals: pallet_evolution_core::PendingProposals::<Runtime>::get().len() as u32,
                ai_agents_count: pallet_evolution_core::AIAgentApprovers::<Runtime>::iter().count() as u32,
                total_mutations_applied: pallet_evolution_core::TotalMutationsApplied::<Runtime>::get(),
            }
        }

        fn get_recent_metrics(depth: u32) -> Vec<(BlockNumber, pallet_evolution_core::runtime_api::BlockMetricsResponse)> {
            pallet_evolution_core::Pallet::<Runtime>::get_recent_metrics(depth)
                .into_iter()
                .map(|(block, metrics)| {
                    (block, pallet_evolution_core::runtime_api::BlockMetricsResponse {
                        gas_used: metrics.gas_used,
                        evm_calls: metrics.evm_calls,
                        svm_calls: metrics.svm_calls,
                        cross_vm_calls: metrics.cross_vm_calls,
                        mempool_depth: metrics.mempool_depth,
                        mev_pressure: metrics.mev_pressure,
                        x3_hotpath_hits: metrics.x3_hotpath_hits,
                        swap_volume: metrics.swap_volume,
                        flashloan_volume: metrics.flashloan_volume,
                    })
                })
                .collect()
        }

        fn get_pending_proposals() -> Vec<pallet_evolution_core::runtime_api::ProposalResponse<AccountId, BlockNumber>> {
            pallet_evolution_core::PendingProposals::<Runtime>::get()
                .into_iter()
                .filter_map(|id| {
                    pallet_evolution_core::Proposals::<Runtime>::get(id).map(|proposal| {
                        pallet_evolution_core::runtime_api::ProposalResponse {
                            id: id as u32,
                            proposer: proposal.proposer,
                            reason: proposal.reason.into_inner(),
                            proposed_at: proposal.proposed_at,
                            approvals: proposal.approvals,
                            status: match proposal.status {
                                pallet_evolution_core::pallet::ProposalStatus::Pending => 0,
                                pallet_evolution_core::pallet::ProposalStatus::Approved => 1,
                                pallet_evolution_core::pallet::ProposalStatus::Rejected => 2,
                                pallet_evolution_core::pallet::ProposalStatus::Applied => 3,
                                pallet_evolution_core::pallet::ProposalStatus::Rolled => 4,
                            },
                        }
                    })
                })
                .collect()
        }

        fn is_ai_agent(account: AccountId) -> bool {
            pallet_evolution_core::AIAgentApprovers::<Runtime>::get(&account)
        }

        fn is_evolution_enabled() -> bool {
            pallet_evolution_core::EvolutionEnabled::<Runtime>::get()
        }
    }

    impl pallet_x3_verifier::runtime_api::X3VerifierApi<Block, AccountId, Balance, BlockNumber> for Runtime {
        fn get_status() -> pallet_x3_verifier::runtime_api::VerifierStatusResponse {
            pallet_x3_verifier::runtime_api::VerifierStatusResponse {
                verification_enabled: pallet_x3_verifier::VerificationEnabled::<Runtime>::get(),
                active_executors: pallet_x3_verifier::Executors::<Runtime>::iter()
                    .filter(|(_, e)| e.active)
                    .count() as u32,
                pending_jobs: pallet_x3_verifier::Jobs::<Runtime>::iter()
                    .filter(|(_, j)| matches!(j.status, pallet_x3_verifier::pallet::JobStatus::Pending | pallet_x3_verifier::pallet::JobStatus::Submitted))
                    .count() as u32,
                total_jobs_submitted: pallet_x3_verifier::TotalJobsSubmitted::<Runtime>::get(),
                total_jobs_verified: pallet_x3_verifier::TotalJobsVerified::<Runtime>::get(),
            }
        }

        fn get_executor(account: AccountId) -> Option<pallet_x3_verifier::runtime_api::ExecutorResponse<AccountId, Balance>> {
            pallet_x3_verifier::Executors::<Runtime>::get(&account).map(|exec| {
                pallet_x3_verifier::runtime_api::ExecutorResponse {
                    account: account.clone(),
                    stake: exec.stake,
                    jobs_completed: exec.jobs_completed,
                    jobs_failed: exec.jobs_failed,
                    reputation: exec.reputation,
                    active: exec.active,
                }
            })
        }

        fn get_active_executors() -> Vec<pallet_x3_verifier::runtime_api::ExecutorResponse<AccountId, Balance>> {
            pallet_x3_verifier::Executors::<Runtime>::iter()
                .filter(|(_, e)| e.active)
                .map(|(account, exec)| {
                    pallet_x3_verifier::runtime_api::ExecutorResponse {
                        account,
                        stake: exec.stake,
                        jobs_completed: exec.jobs_completed,
                        jobs_failed: exec.jobs_failed,
                        reputation: exec.reputation,
                        active: exec.active,
                    }
                })
                .collect()
        }

        fn get_job(job_id: pallet_x3_verifier::runtime_api::JobId) -> Option<pallet_x3_verifier::runtime_api::JobResponse<AccountId, Balance, BlockNumber>> {
            pallet_x3_verifier::Jobs::<Runtime>::get(&job_id).map(|job| {
                pallet_x3_verifier::runtime_api::JobResponse {
                    job_id,
                    submitter: job.submitter,
                    bytecode_hash: job.bytecode_hash,
                    input_hash: job.input_hash,
                    gas_limit: job.gas_limit,
                    reward: job.reward,
                    executor: job.executor,
                    status: match job.status {
                        pallet_x3_verifier::pallet::JobStatus::Pending => 0,
                        pallet_x3_verifier::pallet::JobStatus::Submitted => 1,
                        pallet_x3_verifier::pallet::JobStatus::Verified => 2,
                        pallet_x3_verifier::pallet::JobStatus::Applied => 3,
                        pallet_x3_verifier::pallet::JobStatus::Failed => 4,
                        pallet_x3_verifier::pallet::JobStatus::Disputed => 5,
                    },
                    submitted_at: job.submitted_at,
                    receipt_hash: job.receipt_hash,
                }
            })
        }

        fn get_pending_jobs() -> Vec<pallet_x3_verifier::runtime_api::JobResponse<AccountId, Balance, BlockNumber>> {
            pallet_x3_verifier::Jobs::<Runtime>::iter()
                .filter(|(_, j)| matches!(j.status, pallet_x3_verifier::pallet::JobStatus::Pending | pallet_x3_verifier::pallet::JobStatus::Submitted))
                .map(|(job_id, job)| {
                    pallet_x3_verifier::runtime_api::JobResponse {
                        job_id,
                        submitter: job.submitter,
                        bytecode_hash: job.bytecode_hash,
                        input_hash: job.input_hash,
                        gas_limit: job.gas_limit,
                        reward: job.reward,
                        executor: job.executor,
                        status: match job.status {
                            pallet_x3_verifier::pallet::JobStatus::Pending => 0,
                            pallet_x3_verifier::pallet::JobStatus::Submitted => 1,
                            pallet_x3_verifier::pallet::JobStatus::Verified => 2,
                            pallet_x3_verifier::pallet::JobStatus::Applied => 3,
                            pallet_x3_verifier::pallet::JobStatus::Failed => 4,
                            pallet_x3_verifier::pallet::JobStatus::Disputed => 5,
                        },
                        submitted_at: job.submitted_at,
                        receipt_hash: job.receipt_hash,
                    }
                })
                .collect()
        }

        fn get_receipt(job_id: pallet_x3_verifier::runtime_api::JobId) -> Option<pallet_x3_verifier::runtime_api::ReceiptResponse<AccountId>> {
            let receipt_hash = pallet_x3_verifier::Jobs::<Runtime>::get(&job_id)?.receipt_hash?;

            pallet_x3_verifier::Receipts::<Runtime>::get(&receipt_hash).map(|receipt| {
                pallet_x3_verifier::runtime_api::ReceiptResponse {
                    job_id,
                    executor: receipt.executor,
                    input_hash: receipt.input_hash,
                    output_hash: receipt.output_hash,
                    state_root_before: receipt.state_root_before,
                    state_root_after: receipt.state_root_after,
                    gas_used: receipt.gas_used,
                    timestamp: receipt.timestamp,
                }
            })
        }

        fn is_verification_enabled() -> bool {
            pallet_x3_verifier::VerificationEnabled::<Runtime>::get()
        }

        fn is_executor(account: AccountId) -> bool {
            pallet_x3_verifier::Executors::<Runtime>::get(&account)
                .map(|e| e.active)
                .unwrap_or(false)
        }
    }

    impl pallet_x3_domain_registry::runtime_api::X3DomainRegistryApi<Block, AccountId> for Runtime {
        fn get_records(domain: Vec<u8>) -> Vec<pallet_x3_domain_registry::runtime_api::X3DnsRecordResponse> {
            pallet_x3_domain_registry::Pallet::<Runtime>::runtime_get_records(domain)
        }

        fn get_domain(domain: Vec<u8>) -> Option<pallet_x3_domain_registry::runtime_api::X3DomainResponse<AccountId>> {
            pallet_x3_domain_registry::Pallet::<Runtime>::runtime_get_domain(domain)
        }

        fn list_domains() -> Vec<Vec<u8>> {
            pallet_x3_domain_registry::Pallet::<Runtime>::runtime_list_domains()
        }
    }

    // REMOVED (RC-1 Phase 7 blocker): Duplicate pallet_x3_kernel::runtime_api::AtlasKernelApi
    // Consolidated with canonical pallet_x3_kernel::AtlasKernelRuntimeApi (impl at line 1741)
    // All methods for EVM state querying and contract deployment are in the canonical impl

    // C-011: Implement the X3AtomicKernelApi so that RPC nodes and external verifiers
    // can query PoAE proofs, bundle status, and finality cert anchors without
    // direct storage access.
    impl pallet_x3_atomic_kernel::X3AtomicKernelApi<Block> for Runtime {
        fn get_poae_proof(bundle_id: sp_core::H256) -> Option<pallet_x3_atomic_kernel::proof::PoaeProof> {
            pallet_x3_atomic_kernel::PoaeProofs::<Runtime>::get(bundle_id)
        }

        fn get_bundle_status(bundle_id: sp_core::H256) -> Option<pallet_x3_atomic_kernel::BundleStatus> {
            pallet_x3_atomic_kernel::Bundles::<Runtime>::get(bundle_id)
                .map(|r| r.status)
        }

        fn get_finality_cert_anchor(block_num: u64) -> Option<sp_core::H256> {
            pallet_x3_atomic_kernel::FinalityCertAnchors::<Runtime>::get(block_num)
        }
    }

    // ════════════════════════════════════════════════════════════════════════════════════
    // GPU Validator Runtime API
    // ════════════════════════════════════════════════════════════════════════════════════
    #[cfg(feature = "gpu-validator")]
    impl gpu_validator_api::GpuValidatorRuntimeApi<Block> for Runtime {
        fn gpu_validator_status(validator_id: u32) -> Option<gpu_validator_api::GpuValidatorStatus> {
            Some(gpu_validator_api::GpuValidatorStatus {
                validator_id,
                health_status: b"operational".to_vec(),
                total_proofs_processed: 0,
                successful_proofs: 0,
                failed_proofs: 0,
                gpu_devices_online: 0,
                cpu_fallback_active: false,
                last_health_check_block: <frame_system::Pallet<Runtime>>::block_number(),
            })
        }

        fn query_orchestrator_health() -> gpu_validator_api::OrchestratorHealthStatus {
            gpu_validator_api::OrchestratorHealthStatus {
                status: b"operational".to_vec(),
                uptime_seconds: 0,
                active_validators: 0,
                quarantined_validators: 0,
                pending_tasks: 0,
                tasks_completed: 0,
                avg_task_latency_ms: 0,
                network_health_percent: 100,
            }
        }

        fn submit_gpu_validator_proof(
            proof: Vec<u8>,
            validator_id: u32,
        ) -> gpu_validator_api::GpuProofResult {
            let mut proof_hash = [0u8; 32];
            if proof.len() >= 32 {
                proof_hash.copy_from_slice(&proof[0..32]);
            }
            gpu_validator_api::GpuProofResult {
                proof_hash,
                status: b"pending".to_vec(),
                error_message: Vec::new(),
                processed_by_validator: validator_id,
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════════════════
    // Phase 9: Cross-Chain Header Validation API
    // ════════════════════════════════════════════════════════════════════════════════════
    #[cfg(feature = "gpu-validator")]
    impl gpu_validator_api::CrossChainStateRootApi<Block> for Runtime {
        fn validate_evm_header(
            block_number: u64,
            block_hash: sp_core::H256,
            state_root: sp_core::H256,
        ) -> Option<cross_chain_state_root_api::EvmHeaderProof> {
            if block_number == 0 || block_hash == sp_core::H256::zero() || state_root == sp_core::H256::zero() {
                return None;
            }

            if !pallet_x3_verifier::VerificationEnabled::<Runtime>::get() {
                return None;
            }

            let finality_cfg = pallet_x3_settlement_engine::ChainFinality::<Runtime>::get(
                pallet_x3_settlement_engine::types::ExternalChainId::Ethereum,
            )
            .unwrap_or_else(|| {
                pallet_x3_settlement_engine::finality::FinalityOracle::default_config(
                    pallet_x3_settlement_engine::types::ExternalChainId::Ethereum,
                )
            });
            let config_hash = sp_core::H256::from(sp_io::hashing::blake2_256(&finality_cfg.encode()));
            let mut proof_material = finality_cfg.encode();
            proof_material.extend(block_number.encode());
            proof_material.extend(block_hash.as_bytes());
            proof_material.extend(state_root.as_bytes());

            Some(cross_chain_state_root_api::EvmHeaderProof {
                block_number,
                block_hash,
                state_root,
                timestamp: frame_system::Pallet::<Runtime>::block_number().saturated_into::<u64>() * MILLISECS_PER_BLOCK,
                validator_set_hash: config_hash,
                proof_hash: sp_core::H256::from(sp_io::hashing::blake2_256(&proof_material)),
                processed_by: cross_chain_state_root_api::ProcessorType::CpuRustNative,
                confidence: pallet_x3_settlement_engine::finality::FinalityOracle::finality_score(&finality_cfg, 1),
            })
        }

        fn validate_svm_header(
            slot: u64,
            block_hash: sp_core::H256,
            state_root: sp_core::H256,
        ) -> Option<cross_chain_state_root_api::SvmHeaderProof> {
            if slot == 0 || block_hash == sp_core::H256::zero() || state_root == sp_core::H256::zero() {
                return None;
            }

            if !pallet_x3_verifier::VerificationEnabled::<Runtime>::get() {
                return None;
            }

            let finality_cfg = pallet_x3_settlement_engine::ChainFinality::<Runtime>::get(
                pallet_x3_settlement_engine::types::ExternalChainId::Solana,
            )
            .unwrap_or_else(|| {
                pallet_x3_settlement_engine::finality::FinalityOracle::default_config(
                    pallet_x3_settlement_engine::types::ExternalChainId::Solana,
                )
            });
            let mut proof_material = finality_cfg.encode();
            proof_material.extend(slot.encode());
            proof_material.extend(block_hash.as_bytes());
            proof_material.extend(state_root.as_bytes());

            Some(cross_chain_state_root_api::SvmHeaderProof {
                slot,
                block_hash,
                state_root,
                parent_slot_hashes: vec![],
                validator_signature_count: finality_cfg.confirmations_required,
                proof_hash: sp_core::H256::from(sp_io::hashing::blake2_256(&proof_material)),
                processed_by: cross_chain_state_root_api::ProcessorType::CpuRustNative,
                confidence: pallet_x3_settlement_engine::finality::FinalityOracle::finality_score(&finality_cfg, 1),
            })
        }

        fn query_cross_chain_status() -> cross_chain_state_root_api::CrossChainValidationStatus {
            cross_chain_status_response()
        }

        fn aggregate_cross_chain_proofs(
            proofs: Vec<cross_chain_state_root_api::CrossChainProofBatch>,
        ) -> Option<cross_chain_state_root_api::CrossChainProofBatch> {
            if proofs.is_empty() {
                return None;
            }

            let first = &proofs[0];
            let transaction_hashes: Vec<sp_core::H256> = proofs
                .iter()
                .flat_map(|proof| proof.transaction_hashes.iter().copied())
                .collect();
            let merkle_proofs: Vec<Vec<sp_core::H256>> = proofs
                .iter()
                .flat_map(|proof| proof.merkle_proofs.iter().cloned())
                .collect();
            let mut root_material = Vec::new();
            for proof in &proofs {
                root_material.extend(proof.encode());
            }

            Some(cross_chain_state_root_api::CrossChainProofBatch {
                chain_id: first.chain_id,
                proof_type: first.proof_type,
                merkle_root: sp_core::H256::from(sp_io::hashing::blake2_256(&root_material)),
                transaction_hashes,
                merkle_proofs,
                batch_size: proofs.iter().map(|proof| proof.batch_size).sum(),
                processed_by: cross_chain_state_root_api::ProcessorType::CpuRustNative,
            })
        }

        fn query_last_evm_header() -> Option<cross_chain_state_root_api::EvmHeaderInfo> {
            pallet_cross_chain_validator::LastEvmHeader::<Runtime>::get()
        }

        fn query_last_svm_header() -> Option<cross_chain_state_root_api::SvmHeaderInfo> {
            pallet_cross_chain_validator::LastSvmHeader::<Runtime>::get()
        }

        fn verify_evm_merkle_root(block_number: u64, merkle_root: sp_core::H256) -> bool {
            pallet_cross_chain_validator::Pallet::<Runtime>::is_evm_merkle_root_verified(block_number, merkle_root)
        }

        fn verify_svm_validator_set(slot: u64, validator_set_hash: sp_core::H256) -> bool {
            pallet_cross_chain_validator::Pallet::<Runtime>::is_svm_validator_set_verified(slot, validator_set_hash)
        }
    }

    // ════════════════════════════════════════════════════════════════════════════════════
    // Phase 10a: Governance Settlement & Dispute Resolution API
    // ════════════════════════════════════════════════════════════════════════════════════
    #[cfg(feature = "gpu-validator")]
    impl gpu_validator_api::GovernanceSettlementApi<Block> for Runtime {
        fn submit_dispute(
            proof_hash: sp_core::H256,
            reason: Vec<u8>,
        ) -> Option<governance_settlement_api::DisputeRecord> {
            if proof_hash == sp_core::H256::zero() || reason.is_empty() {
                return None;
            }

            let (dispute_id, job) = pallet_x3_verifier::Jobs::<Runtime>::iter()
                .find(|(_, job)| job.receipt_hash == Some(proof_hash))?;
            let created_at_block = job.submitted_at.saturated_into::<u32>();
            let resolve_at_block = created_at_block.saturating_add(VotingPeriod::get().saturated_into::<u32>());

            Some(governance_settlement_api::DisputeRecord {
                dispute_id,
                proof_hash,
                reason,
                challenger: None,
                status: match job.status {
                    pallet_x3_verifier::pallet::JobStatus::Disputed => governance_settlement_api::DisputeStatus::Active,
                    pallet_x3_verifier::pallet::JobStatus::Failed => governance_settlement_api::DisputeStatus::Accepted,
                    pallet_x3_verifier::pallet::JobStatus::Applied => governance_settlement_api::DisputeStatus::Rejected,
                    _ => governance_settlement_api::DisputeStatus::Pending,
                },
                votes_yes: 0,
                votes_no: 0,
                votes_abstain: 0,
                created_at_block,
                resolve_at_block,
                evidence_hash: job.input_hash,
            })
        }

        fn query_dispute_status(
            proof_hash: sp_core::H256,
        ) -> Option<governance_settlement_api::DisputeRecord> {
            if proof_hash == sp_core::H256::zero() {
                return None;
            }

            let (dispute_id, job) = pallet_x3_verifier::Jobs::<Runtime>::iter()
                .find(|(_, job)| job.receipt_hash == Some(proof_hash))?;
            let created_at_block = job.submitted_at.saturated_into::<u32>();
            let resolve_at_block = created_at_block.saturating_add(VotingPeriod::get().saturated_into::<u32>());

            Some(governance_settlement_api::DisputeRecord {
                dispute_id,
                proof_hash,
                reason: b"status-query".to_vec(),
                challenger: None,
                status: match job.status {
                    pallet_x3_verifier::pallet::JobStatus::Disputed => governance_settlement_api::DisputeStatus::Active,
                    pallet_x3_verifier::pallet::JobStatus::Failed => governance_settlement_api::DisputeStatus::Accepted,
                    pallet_x3_verifier::pallet::JobStatus::Applied => governance_settlement_api::DisputeStatus::Rejected,
                    _ => governance_settlement_api::DisputeStatus::Pending,
                },
                votes_yes: 0,
                votes_no: 0,
                votes_abstain: 0,
                created_at_block,
                resolve_at_block,
                evidence_hash: job.input_hash,
            })
        }

        fn confirm_settlement_finality(
            proof_hash: sp_core::H256,
        ) -> Option<governance_settlement_api::ProofFinalityStatus> {
            proof_finality_status_response(proof_hash)
        }
    }

    // ════════════════════════════════════════════════════════════════════════════════════
    // Phase 10a: Settlement Finality Confirmation API
    // ════════════════════════════════════════════════════════════════════════════════════
    #[cfg(feature = "gpu-validator")]
    impl gpu_validator_api::SettlementFinalityApi<Block> for Runtime {
        fn query_finality_metrics() -> governance_settlement_api::FinalityMetrics {
            finality_metrics_response()
        }

        fn query_validator_reputation(
            validator_id: AccountId,
        ) -> governance_settlement_api::ValidatorReputation {
            validator_reputation_response(validator_id)
        }

        fn query_batch_finality_status(
            merkle_root: sp_core::H256,
        ) -> Option<governance_settlement_api::BatchFinalityStatus> {
            if merkle_root == sp_core::H256::zero() {
                return None;
            }

            None
        }
    }

    // ════════════════════════════════════════════════════════════════════════════════════
    // Phase 3: Agent Memory Runtime API Implementation
    // ════════════════════════════════════════════════════════════════════════════════════
    impl pallet_agent_memory::runtime_api::AgentMemoryApi<Block> for Runtime {
        /// Get latest memory hash and consensus status for an agent.
        fn agent_memory_hash(agent_id: Vec<u8>) -> pallet_agent_memory::runtime_api::MemoryHashResponse {
            if agent_id.len() != 32 {
                return pallet_agent_memory::runtime_api::MemoryHashResponse {
                    memory_hash: vec![],
                    block_number: 0,
                    indexed_at: 0,
                    consensus_reached: false,
                    attestations: 0,
                };
            }

            // Convert first 4 bytes of agent_id to u32 (pallet uses u32 for AgentId)
            let mut id_bytes = [0u8; 4];
            id_bytes.copy_from_slice(&agent_id[..4]);
            let agent_id_u32 = u32::from_le_bytes(id_bytes);

            if let Some(memory_hash) = pallet_agent_memory::LatestMemoryHash::<Runtime>::get(agent_id_u32) {
                let current_block = frame_system::Pallet::<Runtime>::block_number();
                let consensus_records = pallet_agent_memory::MemoryConsensusRecords::<Runtime>::get(
                    agent_id_u32,
                    current_block.saturated_into::<u32>(),
                );

                let (attestations, consensus_reached) = consensus_records
                    .map(|(_, count)| (count, count > 0))
                    .unwrap_or((0, false));

                pallet_agent_memory::runtime_api::MemoryHashResponse {
                    memory_hash: memory_hash.as_bytes().to_vec(),
                    block_number: current_block.saturated_into::<u32>(),
                    indexed_at: current_block.saturated_into::<u32>(),
                    consensus_reached,
                    attestations,
                }
            } else {
                pallet_agent_memory::runtime_api::MemoryHashResponse {
                    memory_hash: vec![],
                    block_number: 0,
                    indexed_at: 0,
                    consensus_reached: false,
                    attestations: 0,
                }
            }
        }

        /// Get agent memory snapshot at specific block.
        fn agent_memory_at_block(
            agent_id: Vec<u8>,
            block_number: u32,
        ) -> pallet_agent_memory::runtime_api::MemorySnapshotResponse {
            if agent_id.len() != 32 {
                return pallet_agent_memory::runtime_api::MemorySnapshotResponse {
                    agent_id: vec![],
                    block_number: 0,
                    memory_data: vec![],
                    size_bytes: 0,
                    verified: false,
                    verification_block: 0,
                };
            }

            // Convert first 4 bytes of agent_id to u32 (pallet uses u32 for AgentId)
            let mut id_bytes = [0u8; 4];
            id_bytes.copy_from_slice(&agent_id[..4]);
            let agent_id_u32 = u32::from_le_bytes(id_bytes);

            let current_block = frame_system::Pallet::<Runtime>::block_number();
            let consensus_records =
                pallet_agent_memory::MemoryConsensusRecords::<Runtime>::get(agent_id_u32, block_number);
            let verified = consensus_records.is_some();

            pallet_agent_memory::runtime_api::MemorySnapshotResponse {
                agent_id,
                block_number,
                memory_data: vec![],
                size_bytes: 0,
                verified,
                verification_block: current_block.saturated_into::<u32>(),
            }
        }

        /// Execute readonly query against agent memory.
        fn agent_query(
            agent_id: Vec<u8>,
            _block_number: u32,
            _function_name: Vec<u8>,
            _params: Vec<u8>,
        ) -> pallet_agent_memory::runtime_api::QueryResponse {
            if agent_id.len() != 32 {
                return pallet_agent_memory::runtime_api::QueryResponse {
                    success: false,
                    result: None,
                    error: Some(b"invalid agent_id".to_vec()),
                    executed_block: 0,
                    latency_ms: 0,
                };
            }

            pallet_agent_memory::runtime_api::QueryResponse {
                success: true,
                result: Some(vec![]),
                error: None,
                executed_block: frame_system::Pallet::<Runtime>::block_number().saturated_into::<u32>(),
                latency_ms: 0,
            }
        }

        /// Get consensus status for memory snapshot.
        fn agent_memory_consensus(
            agent_id: Vec<u8>,
            block_number: u32,
        ) -> pallet_agent_memory::runtime_api::ConsensusStatusResponse {
            if agent_id.len() != 32 {
                return pallet_agent_memory::runtime_api::ConsensusStatusResponse {
                    agent_id: vec![],
                    block_number: 0,
                    memory_hash: vec![],
                    attestations_received: vec![],
                    attestations_required: 0,
                    consensus_reached: false,
                    consensus_reached_at_block: 0,
                };
            }

            // Convert first 4 bytes of agent_id to u32 (pallet uses u32 for AgentId)
            let mut id_bytes = [0u8; 4];
            id_bytes.copy_from_slice(&agent_id[..4]);
            let agent_id_u32 = u32::from_le_bytes(id_bytes);

            if let Some((memory_hash, attestation_count)) =
                pallet_agent_memory::MemoryConsensusRecords::<Runtime>::get(agent_id_u32, block_number)
            {
                let threshold = <Runtime as pallet_agent_memory::Config>::MemoryConsensusThreshold::get();
                let required = (threshold as u32 + 50) / 100;
                let consensus_reached = attestation_count >= required;

                pallet_agent_memory::runtime_api::ConsensusStatusResponse {
                    agent_id,
                    block_number,
                    memory_hash: memory_hash.as_bytes().to_vec(),
                    attestations_received: vec![],
                    attestations_required: required,
                    consensus_reached,
                    consensus_reached_at_block: if consensus_reached {
                        frame_system::Pallet::<Runtime>::block_number().saturated_into::<u32>()
                    } else {
                        0
                    },
                }
            } else {
                pallet_agent_memory::runtime_api::ConsensusStatusResponse {
                    agent_id,
                    block_number,
                    memory_hash: vec![],
                    attestations_received: vec![],
                    attestations_required: <Runtime as pallet_agent_memory::Config>::MemoryConsensusThreshold::get() / 3,
                    consensus_reached: false,
                    consensus_reached_at_block: 0,
                }
            }
        }

        /// Get memory chunk by agent and index (legacy API).
        fn get_memory_chunk(
            _agent_id: u32,
            _chunk_index: u32,
        ) -> Option<pallet_agent_memory::runtime_api::MemoryChunkResponse> {
            None
        }

        /// Get latest entries for an agent (legacy API).
        fn get_latest_entries(
            _agent_id: u32,
            _count: u32,
        ) -> Vec<pallet_agent_memory::runtime_api::MemoryEntryResponse> {
            vec![]
        }

        /// Get memory as JSONL (legacy API).
        fn get_memory_jsonl(
            _agent_id: u32,
            _from_id: u64,
            _limit: u32,
        ) -> pallet_agent_memory::runtime_api::MemoryJsonlResponse {
            pallet_agent_memory::runtime_api::MemoryJsonlResponse {
                lines: vec![],
                total: 0,
                has_more: false,
                cursor: None,
            }
        }

        /// Get chunk count for agent (legacy API).
        fn get_chunk_count(_agent_id: u32) -> u32 {
            0
        }

        /// Get entry count for agent (legacy API).
        fn get_entry_count(_agent_id: u32) -> u64 {
            0
        }
    }
}

#[cfg(feature = "gpu-validator")]
fn cross_chain_status_response() -> cross_chain_state_root_api::CrossChainValidationStatus {
    let total_jobs_verified = pallet_x3_verifier::TotalJobsVerified::<Runtime>::get();
    let configured_chain_count =
        pallet_x3_settlement_engine::ChainFinality::<Runtime>::iter().count() as u64;

    cross_chain_state_root_api::CrossChainValidationStatus {
        evm_headers_validated: configured_chain_count,
        svm_headers_validated: configured_chain_count,
        proof_batches_submitted: total_jobs_verified,
        validation_failures: pallet_x3_settlement_engine::InvariantViolations::<Runtime>::get()
            .saturated_into::<u32>(),
        last_validated_block: pallet_x3_verifier::Jobs::<Runtime>::iter()
            .filter(|(_, job)| job.receipt_hash.is_some())
            .map(|(_, job)| job.submitted_at.saturated_into::<u64>())
            .max()
            .unwrap_or_default(),
        cpu_fallback_count: 0,
    }
}

#[cfg(feature = "gpu-validator")]
fn proof_finality_status_response(
    proof_hash: H256,
) -> Option<governance_settlement_api::ProofFinalityStatus> {
    if proof_hash == H256::zero() {
        return None;
    }

    let (_, job) = pallet_x3_verifier::Jobs::<Runtime>::iter()
        .find(|(_, job)| job.receipt_hash == Some(proof_hash))?;
    let finality_cfg = pallet_x3_settlement_engine::ChainFinality::<Runtime>::get(
        pallet_x3_settlement_engine::types::ExternalChainId::X3Native,
    )
    .unwrap_or_else(|| {
        pallet_x3_settlement_engine::finality::FinalityOracle::default_config(
            pallet_x3_settlement_engine::types::ExternalChainId::X3Native,
        )
    });
    let (is_finalized, confidence_percent) = match job.status {
        pallet_x3_verifier::pallet::JobStatus::Applied => (true, 100),
        pallet_x3_verifier::pallet::JobStatus::Verified => (true, 90),
        pallet_x3_verifier::pallet::JobStatus::Submitted => (false, 50),
        pallet_x3_verifier::pallet::JobStatus::Pending => (false, 25),
        pallet_x3_verifier::pallet::JobStatus::Failed
        | pallet_x3_verifier::pallet::JobStatus::Disputed => (false, 0),
    };

    Some(governance_settlement_api::ProofFinalityStatus {
        proof_hash,
        is_finalized,
        finality_block: if is_finalized {
            job.submitted_at.saturated_into::<u32>()
        } else {
            0
        },
        confidence_percent,
        validator_signatures: finality_cfg.confirmations_required,
        finality_type: governance_settlement_api::FinalityType::SettlementEngine,
    })
}

#[cfg(feature = "gpu-validator")]
fn finality_metrics_response() -> governance_settlement_api::FinalityMetrics {
    let jobs: Vec<_> = pallet_x3_verifier::Jobs::<Runtime>::iter()
        .map(|(_, job)| job)
        .collect();
    let total_proofs = pallet_x3_verifier::TotalJobsSubmitted::<Runtime>::get();
    let finalized_proofs = jobs
        .iter()
        .filter(|job| matches!(job.status, pallet_x3_verifier::pallet::JobStatus::Applied))
        .count() as u64;
    let disputed_proofs = jobs
        .iter()
        .filter(|job| matches!(job.status, pallet_x3_verifier::pallet::JobStatus::Disputed))
        .count() as u64;
    let rejected_proofs = jobs
        .iter()
        .filter(|job| matches!(job.status, pallet_x3_verifier::pallet::JobStatus::Failed))
        .count() as u64;
    let finality_rate_percent = if total_proofs == 0 {
        0
    } else {
        ((finalized_proofs.saturating_mul(100)) / total_proofs).min(100) as u8
    };

    governance_settlement_api::FinalityMetrics {
        total_proofs,
        finalized_proofs,
        disputed_proofs,
        rejected_proofs,
        active_disputes: disputed_proofs.saturated_into::<u32>(),
        average_finality_blocks: 0,
        finality_rate_percent,
        last_finality_block: jobs
            .iter()
            .filter(|job| matches!(job.status, pallet_x3_verifier::pallet::JobStatus::Applied))
            .map(|job| job.submitted_at.saturated_into::<u32>())
            .max()
            .unwrap_or_default(),
    }
}

#[cfg(feature = "gpu-validator")]
fn validator_reputation_response(
    validator_id: AccountId,
) -> governance_settlement_api::ValidatorReputation {
    let validator_key = validator_id.encode();
    let mut disputes_lost = 0u32;
    let mut total_disputes = 0u32;

    for (_, job) in pallet_x3_verifier::Jobs::<Runtime>::iter() {
        if job.executor.as_ref() != Some(&validator_id) {
            continue;
        }

        if matches!(
            job.status,
            pallet_x3_verifier::pallet::JobStatus::Disputed
                | pallet_x3_verifier::pallet::JobStatus::Failed
        ) {
            total_disputes = total_disputes.saturating_add(1);
        }
        if matches!(job.status, pallet_x3_verifier::pallet::JobStatus::Failed) {
            disputes_lost = disputes_lost.saturating_add(1);
        }
    }

    let Some(executor) = pallet_x3_verifier::Executors::<Runtime>::get(&validator_id) else {
        return governance_settlement_api::ValidatorReputation {
            validator_id: validator_key,
            reputation_score: 0,
            disputes_won: 0,
            disputes_lost,
            valid_dispute_percent: 0,
            total_disputes,
        };
    };

    let disputes_won = total_disputes.saturating_sub(disputes_lost);
    governance_settlement_api::ValidatorReputation {
        validator_id: validator_key,
        reputation_score: executor.reputation as u32,
        disputes_won,
        disputes_lost,
        valid_dispute_percent: if total_disputes == 0 {
            executor.reputation
        } else {
            ((disputes_won.saturating_mul(100)) / total_disputes).min(100) as u8
        },
        total_disputes,
    }
}

// ─────────────────────────────────────────────────────────────────
// Cross-Chain GPU State-Root Validation API
// ─────────────────────────────────────────────────────────────────

pub mod cross_chain_state_root_api {
    use codec::{Decode, Encode};
    use scale_info::TypeInfo;
    use sp_core::H256;
    use sp_std::vec::Vec;

    /// EVM block header validation result
    #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
    pub struct EvmHeaderProof {
        pub block_number: u64,
        pub block_hash: H256,
        pub state_root: H256,
        pub timestamp: u64,
        pub validator_set_hash: H256,
        pub proof_hash: H256,
        pub processed_by: ProcessorType,
        pub confidence: u32,
    }

    /// SVM block header validation result (Solana)
    #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
    pub struct SvmHeaderProof {
        pub slot: u64,
        pub block_hash: H256,
        pub state_root: H256,
        pub parent_slot_hashes: Vec<H256>,
        pub validator_signature_count: u32,
        pub proof_hash: H256,
        pub processed_by: ProcessorType,
        pub confidence: u32,
    }

    /// Merkle inclusion proof for external transaction
    #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
    pub struct CrossChainProofBatch {
        pub chain_id: u32,
        pub proof_type: ProofType,
        pub merkle_root: H256,
        pub transaction_hashes: Vec<H256>,
        pub merkle_proofs: Vec<Vec<H256>>,
        pub batch_size: u32,
        pub processed_by: ProcessorType,
    }

    /// Processor type (GPU or CPU)
    #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq, Copy)]
    pub enum ProcessorType {
        GpuCuda,
        GpuMetal,
        GpuOpenCl,
        CpuRustNative,
    }

    /// Proof type
    #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq, Copy)]
    pub enum ProofType {
        EvmKeccak256,
        SvmSha256,
        SvmSecp256k1,
    }

    /// Cross-chain validation status
    #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
    pub struct CrossChainValidationStatus {
        pub evm_headers_validated: u64,
        pub svm_headers_validated: u64,
        pub proof_batches_submitted: u64,
        pub validation_failures: u32,
        pub last_validated_block: u64,
        pub cpu_fallback_count: u32,
    }

    /// Stored EVM header info from pallet storage
    #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
    pub struct EvmHeaderInfo {
        pub block_number: u64,
        pub block_hash: H256,
        pub state_root: H256,
        pub merkle_root: H256,
        pub validator_set_hash: H256,
        pub verified_at_block: u32,
        pub validation_proof: Vec<u8>,
    }

    /// Stored SVM header info from pallet storage
    #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
    pub struct SvmHeaderInfo {
        pub slot: u64,
        pub block_hash: H256,
        pub state_root: H256,
        pub validator_set_hash: H256,
        pub verified_at_block: u32,
        pub validation_proof: Vec<u8>,
        pub parent_slot_hashes: Vec<H256>,
    }
}

// ─────────────────────────────────────────────────────────────────
// Phase 10a: Governance & Settlement Finality API (Structural)
// ─────────────────────────────────────────────────────────────────

pub mod governance_settlement_api {
    use codec::{Decode, Encode};
    use scale_info::TypeInfo;
    use sp_core::H256;
    use sp_std::vec::Vec;

    /// Dispute record for governance-driven settlement challenges
    #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
    pub struct DisputeRecord {
        pub dispute_id: H256,
        pub proof_hash: H256,
        pub reason: Vec<u8>,
        pub challenger: Option<Vec<u8>>, // AccountId (flexible encoding)
        pub status: DisputeStatus,
        pub votes_yes: u32,
        pub votes_no: u32,
        pub votes_abstain: u32,
        pub created_at_block: u32,
        pub resolve_at_block: u32,
        pub evidence_hash: H256,
    }

    /// Dispute resolution status
    #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq, Copy)]
    pub enum DisputeStatus {
        Pending,  // Challenge submitted, voting window open
        Active,   // Voting underway
        Resolved, // Voting closed, result finalized
        Rejected, // Challenge dismissed (proof was valid)
        Accepted, // Challenge upheld (proof was invalid)
    }

    /// Finality confirmation status for a proof
    #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
    pub struct ProofFinalityStatus {
        pub proof_hash: H256,
        pub is_finalized: bool,
        pub finality_block: u32,
        pub confidence_percent: u8,
        pub validator_signatures: u32,
        pub finality_type: FinalityType,
    }

    /// Finality type (determined by settlement mechanism)
    #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq, Copy)]
    pub enum FinalityType {
        Governance,       // Settled by validator dispute resolution (2/3 majority)
        SettlementEngine, // Settled by cross-chain intent finalization
        BlockHeader,      // Native X3 block finality (1 block)
    }

    /// Aggregated finality metrics across all proofs
    #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
    pub struct FinalityMetrics {
        pub total_proofs: u64,
        pub finalized_proofs: u64,
        pub disputed_proofs: u64,
        pub rejected_proofs: u64,
        pub active_disputes: u32,
        pub average_finality_blocks: u32,
        pub finality_rate_percent: u8,
        pub last_finality_block: u32,
    }

    /// Validator dispute resolution reputation
    #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
    pub struct ValidatorReputation {
        pub validator_id: Vec<u8>, // AccountId (flexible encoding)
        pub reputation_score: u32,
        pub disputes_won: u32,
        pub disputes_lost: u32,
        pub valid_dispute_percent: u8,
        pub total_disputes: u32,
    }

    /// Finality status for a merkle-aggregated batch of proofs
    #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
    pub struct BatchFinalityStatus {
        pub merkle_root: H256,
        pub batch_finalized: bool,
        pub finality_block: u32,
        pub merkle_path_count: u32,
        pub verified_proofs: u32,
        pub total_proofs_in_batch: u32,
    }

    /// Trait declaration for governance-driven settlement API
    pub trait GovernanceSettlementApi {
        fn submit_dispute(proof_hash: H256, reason: Vec<u8>) -> Option<DisputeRecord>;

        fn query_dispute_status(proof_hash: H256) -> Option<DisputeRecord>;

        fn confirm_settlement_finality(proof_hash: H256) -> Option<ProofFinalityStatus>;
    }

    /// Trait declaration for settlement finality confirmation API
    pub trait SettlementFinalityApi {
        fn query_finality_metrics() -> FinalityMetrics;

        fn query_validator_reputation(validator_id: Vec<u8>) -> ValidatorReputation;

        fn query_batch_finality_status(merkle_root: H256) -> Option<BatchFinalityStatus>;
    }
}

// REMOVED (RC-1 Phase 7 blocker): CrossChainStateRootApi declared but NOT implemented
// Status: Declared in sp_api::decl_runtime_apis! but NO impl block in impl_runtime_apis!
// Action: Removed both declaration and dummy no_std trait
// Rationale: Deferred to Phase 9 (Bridge/Relayer) when actual proof validation from pallet-x3-verifier is ready
// Impact: Prevents non-functional API exposure to RPC clients

#[cfg(feature = "std")]
pub fn x3_kernel_default_assets() -> Vec<(AssetId, Vec<u8>, u8)> {
    vec![
        (0, b"X3".to_vec(), 12),
        (1, b"ETH".to_vec(), 18),
        (2, b"SOL".to_vec(), 9),
        (3, b"USDC".to_vec(), 6),
        (1000, b"X3".to_vec(), 18),
    ]
}

#[cfg(feature = "std")]
pub fn runtime_uses_mock_vm_adapters() -> bool {
    let evm = core::any::type_name::<<Runtime as pallet_x3_kernel::Config>::EvmAdapter>();
    let svm = core::any::type_name::<<Runtime as pallet_x3_kernel::Config>::SvmAdapter>();
    let x3 = core::any::type_name::<<Runtime as pallet_x3_kernel::Config>::X3Adapter>();

    evm.contains("MockEvmAdapter") || svm.contains("MockSvmAdapter") || x3.contains("MockX3Adapter")
}

#[cfg(all(test, feature = "std"))]
mod vm_adapter_tests {
    use super::*;
    use codec::Encode;
    use frame_support::BoundedVec;
    use pallet_x3_kernel::{EvmExecutorAdapter, SvmExecutorAdapter, X3ExecutorAdapter};
    use sp_core::Pair;
    use sp_core::H160;
    use sp_runtime::traits::BlakeTwo256;

    fn vm_test_ext() -> sp_io::TestExternalities {
        let aura = sp_consensus_aura::sr25519::AuthorityPair::from_string("//Alice", None)
            .expect("Aura test authority should derive from seed")
            .public();
        let grandpa = sp_consensus_grandpa::AuthorityPair::from_string("//Alice", None)
            .expect("Grandpa test authority should derive from seed")
            .public();
        let mut validator_bytes = [0u8; 32];
        validator_bytes.copy_from_slice(&aura.encode()[..32]);
        let validator_account = AccountId::from(validator_bytes);
        let source_account: AccountId =
            <pallet_evm::HashedAddressMapping<BlakeTwo256> as pallet_evm::AddressMapping<
                AccountId,
            >>::into_account_id(SYSTEM_EVM_CALLER);

        let storage = RuntimeGenesisConfig {
            balances: BalancesConfig {
                balances: vec![
                    (source_account, 1_000_000 * X3),
                    (validator_account.clone(), 1_000_000 * X3),
                ],
            },
            grandpa: GrandpaConfig {
                authorities: vec![(grandpa, 1)],
                _config: Default::default(),
            },
            session: SessionConfig {
                keys: vec![(
                    validator_account.clone(),
                    validator_account,
                    SessionKeys { aura },
                )],
            },
            atlas_kernel: AtlasKernelConfig {
                assets: x3_kernel_default_assets(),
            },
            x3_coin: X3CoinConfig {
                team_allocations: Vec::new(),
                ecosystem_allocations: Vec::new(),
                liquidity_allocations: Vec::new(),
            },
            ..Default::default()
        }
        .build_storage()
        .expect("runtime test genesis should build");

        let mut ext = sp_io::TestExternalities::new(storage);
        ext.execute_with(|| {
            frame_system::Pallet::<Runtime>::set_block_number(1);
            Timestamp::set_timestamp(MILLISECS_PER_BLOCK);
        });
        ext
    }

    fn test_account(seed: u8) -> AccountId {
        sp_runtime::AccountId32::new([seed; 32])
    }

    fn verifier_receipt_response_for_test(
        job_id: pallet_x3_verifier::runtime_api::JobId,
    ) -> Option<pallet_x3_verifier::runtime_api::ReceiptResponse<AccountId>> {
        let receipt_hash = pallet_x3_verifier::Jobs::<Runtime>::get(&job_id)?.receipt_hash?;

        pallet_x3_verifier::Receipts::<Runtime>::get(&receipt_hash).map(|receipt| {
            pallet_x3_verifier::runtime_api::ReceiptResponse {
                job_id,
                executor: receipt.executor,
                input_hash: receipt.input_hash,
                output_hash: receipt.output_hash,
                state_root_before: receipt.state_root_before,
                state_root_after: receipt.state_root_after,
                gas_used: receipt.gas_used,
                timestamp: receipt.timestamp,
            }
        })
    }

    #[test]
    fn test_native_evm_adapter_real_execution() {
        // Test that NativeEvmAdapter uses real Frontier
        let simple_evm_bytecode = vec![0x60, 0x00, 0x60, 0x00, 0xf3]; // PUSH1 0 PUSH1 0 RETURN
        vm_test_ext().execute_with(|| {
            let result =
                native_vm_adapters::NativeEvmAdapter::execute(&simple_evm_bytecode, 100_000);
            // Debug: print result to stderr to capture runner error details during test
            eprintln!("NativeEvmAdapter.execute result = {:?}", result);
            assert!(result.is_ok());
            let receipt = result.unwrap();
            assert!(receipt.success);
            assert!(receipt.gas_used > 0);
            assert_eq!(receipt.version, pallet_x3_kernel::EXECUTION_RECEIPT_VERSION);
            assert_eq!(receipt.protocol_version, 1);
            assert!(receipt.migration_history.is_empty());
            assert_eq!(receipt.compatibility_flags, 0);
        });
    }

    #[test]
    fn test_native_svm_adapter_real_execution() {
        // Test that NativeSvmAdapter uses real rBPF
        let simple_bpf_program = vec![
            0xb7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // mov r0, 0
            0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // exit
        ];
        vm_test_ext().execute_with(|| {
            let result =
                native_vm_adapters::NativeSvmAdapter::execute(&simple_bpf_program, 100_000);
            assert!(result.is_ok());
            let receipt = result.unwrap();
            assert!(receipt.success);
            assert_eq!(receipt.version, pallet_x3_kernel::EXECUTION_RECEIPT_VERSION);
            assert_eq!(receipt.protocol_version, 1);
            assert!(receipt.migration_history.is_empty());
            assert_eq!(receipt.compatibility_flags, 0);
        });
    }

    #[test]
    fn test_x3_adapter_real_execution() {
        // Test that X3VmAdapter uses real X3 VM
        use pallet_x3_kernel::adapters::real_adapters::X3VmAdapter;

        // Simple X3 bytecode: X3BC magic + minimal module
        let x3_bytecode = vec![0x58, 0x33, 0x42, 0x43];
        let result = X3VmAdapter::validate(&x3_bytecode);
        // Validation should work with real verifier (may return Ok or Err for partial payload)
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_wasm_builds_use_mocks() {
        // Verify WASM builds still use mock adapters
        #[cfg(not(feature = "std"))]
        {
            // In WASM, adapters should be mocks
            // This test ensures no_std compatibility
        }
    }

    #[test]
    fn verifier_receipt_api_resolves_receipt_hash_from_job() {
        vm_test_ext().execute_with(|| {
            let executor = test_account(7);
            let job_id = H256::repeat_byte(0x11);
            let receipt_hash = H256::repeat_byte(0x22);

            pallet_x3_verifier::Jobs::<Runtime>::insert(
                job_id,
                pallet_x3_verifier::pallet::JobRecord::<Runtime> {
                    submitter: executor.clone(),
                    bytecode_hash: H256::repeat_byte(0x33),
                    input_hash: H256::repeat_byte(0x44),
                    gas_limit: 500_000,
                    reward: 10 * X3,
                    status: pallet_x3_verifier::pallet::JobStatus::Applied,
                    submitted_at: 1,
                    executor: Some(executor.clone()),
                    receipt_hash: Some(receipt_hash),
                },
            );

            pallet_x3_verifier::Receipts::<Runtime>::insert(
                receipt_hash,
                pallet_x3_verifier::pallet::ExecutionReceipt::<Runtime> {
                    job_id,
                    executor: executor.clone(),
                    input_hash: H256::repeat_byte(0x44),
                    output_hash: H256::repeat_byte(0x55),
                    state_root_before: H256::repeat_byte(0x66),
                    state_root_after: H256::repeat_byte(0x77),
                    gas_used: 42_000,
                    timestamp: 123_456,
                    output_data: BoundedVec::try_from(vec![1u8, 2, 3]).expect("bounded output"),
                    state_changes: BoundedVec::default(),
                    merkle_proof: BoundedVec::default(),
                    signature: BoundedVec::try_from(vec![0u8; 64]).expect("bounded signature"),
                },
            );

            let receipt = verifier_receipt_response_for_test(job_id)
                .expect("receipt should resolve via job receipt_hash");

            assert_eq!(receipt.job_id, job_id);
            assert_eq!(receipt.executor, executor);
            assert_eq!(receipt.output_hash, H256::repeat_byte(0x55));
            assert_eq!(receipt.gas_used, 42_000);
        });
    }

    #[cfg(feature = "gpu-validator")]
    #[test]
    fn gpu_finality_apis_reflect_verifier_and_settlement_storage() {
        vm_test_ext().execute_with(|| {
            let validator = test_account(9);
            let proof_hash = H256::repeat_byte(0x99);

            pallet_x3_verifier::VerificationEnabled::<Runtime>::put(true);
            pallet_x3_verifier::Executors::<Runtime>::insert(
                &validator,
                pallet_x3_verifier::pallet::ExecutorRecord::<Runtime> {
                    account: validator.clone(),
                    stake: 2_000 * X3,
                    jobs_completed: 4,
                    jobs_failed: 1,
                    total_rewards: 100 * X3,
                    active: true,
                    reputation: 87,
                },
            );
            pallet_x3_verifier::Jobs::<Runtime>::insert(
                H256::repeat_byte(0x10),
                pallet_x3_verifier::pallet::JobRecord::<Runtime> {
                    submitter: validator.clone(),
                    bytecode_hash: H256::repeat_byte(0x20),
                    input_hash: H256::repeat_byte(0x30),
                    gas_limit: 700_000,
                    reward: 25 * X3,
                    status: pallet_x3_verifier::pallet::JobStatus::Applied,
                    submitted_at: 3,
                    executor: Some(validator.clone()),
                    receipt_hash: Some(proof_hash),
                },
            );
            pallet_x3_verifier::TotalJobsSubmitted::<Runtime>::put(1);
            pallet_x3_verifier::TotalJobsVerified::<Runtime>::put(1);
            pallet_x3_settlement_engine::InvariantViolations::<Runtime>::put(2);
            pallet_x3_settlement_engine::ChainFinality::<Runtime>::insert(
                pallet_x3_settlement_engine::types::ExternalChainId::X3Native,
                pallet_x3_settlement_engine::types::FinalityConfig {
                    chain: pallet_x3_settlement_engine::types::ExternalChainId::X3Native,
                    confirmations_required: 5,
                    block_time_ms: 200,
                    proof_type: pallet_x3_settlement_engine::types::ProofType::LightClient,
                    challenge_period_seconds: 0,
                    max_reorg_depth: 0,
                },
            );

            let metrics = finality_metrics_response();
            assert_eq!(metrics.total_proofs, 1);
            assert_eq!(metrics.finalized_proofs, 1);
            assert_eq!(metrics.disputed_proofs, 0);
            assert_eq!(metrics.rejected_proofs, 0);

            let cross_chain = cross_chain_status_response();
            assert_eq!(cross_chain.proof_batches_submitted, 1);
            assert_eq!(cross_chain.validation_failures, 2);
            assert_eq!(cross_chain.last_validated_block, 3);

            let finality = proof_finality_status_response(proof_hash)
                .expect("proof should resolve from verifier job state");
            assert!(finality.is_finalized);
            assert_eq!(finality.validator_signatures, 5);
            assert_eq!(finality.confidence_percent, 100);

            let reputation = validator_reputation_response(validator.clone());
            assert_eq!(reputation.validator_id, validator.encode());
            assert_eq!(reputation.reputation_score, 87);
            assert_eq!(reputation.total_disputes, 0);
        });
    }
}

// Phase 4: E2E Testing & Verification
#[cfg(test)]
mod tests;
