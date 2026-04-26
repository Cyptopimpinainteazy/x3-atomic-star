//! Weight info for Private Execution pallet.

use frame_support::weights::Weight;

/// Weight functions for the private execution pallet.
pub trait WeightInfo {
    fn register_confidential_validator() -> Weight;
    fn deregister_confidential_validator() -> Weight;
    fn refresh_attestation() -> Weight;
    fn submit_private_transaction() -> Weight;
    fn commit_encrypted_state_diff() -> Weight;
    fn set_committee_key() -> Weight;
    fn set_enabled() -> Weight;
}

/// Default weights (placeholder — replace with benchmark output).
impl WeightInfo for () {
    fn register_confidential_validator() -> Weight {
        Weight::from_parts(60_000_000, 0)
    }
    fn deregister_confidential_validator() -> Weight {
        Weight::from_parts(40_000_000, 0)
    }
    fn refresh_attestation() -> Weight {
        Weight::from_parts(50_000_000, 0)
    }
    fn submit_private_transaction() -> Weight {
        Weight::from_parts(80_000_000, 0)
    }
    fn commit_encrypted_state_diff() -> Weight {
        Weight::from_parts(100_000_000, 0)
    }
    fn set_committee_key() -> Weight {
        Weight::from_parts(20_000_000, 0)
    }
    fn set_enabled() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }
}
