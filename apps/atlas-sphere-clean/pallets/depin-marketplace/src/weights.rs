//! Weight info for DePIN marketplace pallet.

use frame_support::weights::Weight;

/// Weight functions for the DePIN marketplace pallet.
pub trait WeightInfo {
    fn register_provider() -> Weight;
    fn deregister_provider() -> Weight;
    fn pause_provider() -> Weight;
    fn resume_provider() -> Weight;
    fn submit_order() -> Weight;
    fn accept_order() -> Weight;
    fn complete_job() -> Weight;
    fn report_job_failure() -> Weight;
    fn cancel_order() -> Weight;
    fn pause_marketplace() -> Weight;
    fn resume_marketplace() -> Weight;
}

/// Default weights (placeholder — replace with benchmark output).
impl WeightInfo for () {
    fn register_provider() -> Weight {
        Weight::from_parts(50_000_000, 0)
    }
    fn deregister_provider() -> Weight {
        Weight::from_parts(40_000_000, 0)
    }
    fn pause_provider() -> Weight {
        Weight::from_parts(20_000_000, 0)
    }
    fn resume_provider() -> Weight {
        Weight::from_parts(20_000_000, 0)
    }
    fn submit_order() -> Weight {
        Weight::from_parts(60_000_000, 0)
    }
    fn accept_order() -> Weight {
        Weight::from_parts(70_000_000, 0)
    }
    fn complete_job() -> Weight {
        Weight::from_parts(80_000_000, 0)
    }
    fn report_job_failure() -> Weight {
        Weight::from_parts(70_000_000, 0)
    }
    fn cancel_order() -> Weight {
        Weight::from_parts(40_000_000, 0)
    }
    fn pause_marketplace() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }
    fn resume_marketplace() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }
}
