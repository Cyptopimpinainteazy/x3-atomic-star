//! Benchmarking setup for pallet-x3-coin.
//!
//! NOTE: This is a minimal placeholder to keep the runtime-benchmarks feature
//! wired. Add real benchmarks before running production-grade weight derivation.

use super::*;
use frame_benchmarking::v2::*;

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn placeholder() -> Result<(), BenchmarkError> {
        Ok(())
    }
}
