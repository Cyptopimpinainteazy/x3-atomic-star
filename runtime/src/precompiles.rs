use core::marker::PhantomData;
use pallet_evm::{
    ExitRevert, IsPrecompileResult, Precompile, PrecompileFailure, PrecompileHandle,
    PrecompileResult, PrecompileSet,
};
use pallet_evm_precompile_modexp::Modexp;
use pallet_evm_precompile_sha3fips::Sha3FIPS256;
use pallet_evm_precompile_simple::{ECRecover, ECRecoverPublicKey, Identity, Ripemd160, Sha256};
use sp_core::H160;

// =====================================================================
// X3 CUSTOM PRECOMPILES
// =====================================================================
// The following precompiles provide EVM ↔ X3 integration at addresses
// 0xf001–0xf004. They are wired into the dispatch table and return a
// standard EVM revert (not a Rust panic) until Phase F pallet dispatch
// is implemented. EVM callers receive a revert with a descriptive message.
//
// Phase F wiring plan (per IMPLEMENTATION_PLAN.md):
//   0xf001 X3Verifier   → pallet_x3_verifier::submit_receipt()
//   0xf002 X3Bridge     → pallet_x3_cross_vm_router::xvm_transfer()
//   0xf003 X3Governance → pallet_governance::propose() / vote()
//   0xf004 X3AssetReg   → pallet_x3_asset_registry::register_asset()
// =====================================================================

/// X3Verifier precompile — GPU proof verification dispatcher
/// Address: 0xf001 (61441)
pub struct X3VerifierPrecompile;

impl Precompile for X3VerifierPrecompile {
    fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        // Guard: reject zero-length calldata early.
        if handle.input().len() < 4 {
            return Err(PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: b"X3Verifier: calldata too short (need 4-byte selector)".to_vec(),
            });
        }
        // Phase F: route by function selector and dispatch to pallet_x3_verifier.
        // Until then, return a clean revert so EVM contracts can handle the failure.
        Err(PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: b"X3Verifier: pallet dispatch not yet enabled (Phase F)".to_vec(),
        })
    }
}

/// X3Bridge precompile — Cross-VM asset bridging
/// Address: 0xf002 (61442)
pub struct X3BridgePrecompile;

impl Precompile for X3BridgePrecompile {
    fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        if handle.input().len() < 4 {
            return Err(PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: b"X3Bridge: calldata too short (need 4-byte selector)".to_vec(),
            });
        }
        // Phase F: dispatch to pallet_x3_cross_vm_router::xvm_transfer().
        Err(PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: b"X3Bridge: pallet dispatch not yet enabled (Phase F)".to_vec(),
        })
    }
}

/// X3Governance precompile — Governance proposal submission
/// Address: 0xf003 (61443)
pub struct X3GovernancePrecompile;

impl Precompile for X3GovernancePrecompile {
    fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        if handle.input().len() < 4 {
            return Err(PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: b"X3Governance: calldata too short (need 4-byte selector)".to_vec(),
            });
        }
        // Phase F: dispatch to pallet_governance::propose() / vote().
        Err(PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: b"X3Governance: pallet dispatch not yet enabled (Phase F)".to_vec(),
        })
    }
}

/// X3AssetRegistry precompile — Asset metadata management
/// Address: 0xf004 (61444)
pub struct X3AssetRegistryPrecompile;

impl Precompile for X3AssetRegistryPrecompile {
    fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        if handle.input().len() < 4 {
            return Err(PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: b"X3AssetRegistry: calldata too short (need 4-byte selector)".to_vec(),
            });
        }
        // Phase F: dispatch to pallet_x3_asset_registry::register_asset().
        Err(PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: b"X3AssetRegistry: pallet dispatch not yet enabled (Phase F)".to_vec(),
        })
    }
}

pub struct FrontierPrecompiles<R>(PhantomData<R>);

impl<R> FrontierPrecompiles<R>
where
    R: pallet_evm::Config,
{
    pub fn new() -> Self {
        Self(Default::default())
    }

    pub fn used_addresses() -> [H160; 11] {
        [
            hash(1),      // ECRecover (Ethereum standard)
            hash(2),      // SHA256 (Ethereum standard)
            hash(3),      // RIPEMD160 (Ethereum standard)
            hash(4),      // Identity (Ethereum standard)
            hash(5),      // ModExp (Ethereum standard)
            hash(1024),   // SHA3FIPS256 (X3 extension)
            hash(1025),   // ECRecoverPublicKey (X3 extension)
            // Reserved addresses for X3 custom precompiles (Phase 2):
            hash(61441),  // 0xf001 - x3_verifier (TODO)
            hash(61442),  // 0xf002 - x3_bridge (TODO)
            hash(61443),  // 0xf003 - x3_governance (TODO)
            hash(61444),  // 0xf004 - x3_asset_registry (TODO)
        ]
    }
}

impl<R> PrecompileSet for FrontierPrecompiles<R>
where
    R: pallet_evm::Config,
{
    fn execute(&self, handle: &mut impl PrecompileHandle) -> Option<PrecompileResult> {
        match handle.code_address() {
            // Ethereum standard precompiles
            a if a == hash(1) => Some(ECRecover::execute(handle)),
            a if a == hash(2) => Some(Sha256::execute(handle)),
            a if a == hash(3) => Some(Ripemd160::execute(handle)),
            a if a == hash(4) => Some(Identity::execute(handle)),
            a if a == hash(5) => Some(Modexp::execute(handle)),
            // X3 additional precompiles
            a if a == hash(1024) => Some(Sha3FIPS256::execute(handle)),
            a if a == hash(1025) => Some(ECRecoverPublicKey::execute(handle)),
            // X3 custom precompiles — wired to struct implementations
            // Each returns PrecompileFailure::Revert until Phase F pallet dispatch lands.
            a if a == hash(61441) => Some(X3VerifierPrecompile::execute(handle)),
            a if a == hash(61442) => Some(X3BridgePrecompile::execute(handle)),
            a if a == hash(61443) => Some(X3GovernancePrecompile::execute(handle)),
            a if a == hash(61444) => Some(X3AssetRegistryPrecompile::execute(handle)),
            _ => None,
        }
    }

    fn is_precompile(&self, address: H160, _gas: u64) -> IsPrecompileResult {
        IsPrecompileResult::Answer {
            is_precompile: Self::used_addresses().contains(&address),
            extra_cost: 0,
        }
    }
}

fn hash(id: u64) -> H160 {
    H160::from_low_u64_be(id)
}
