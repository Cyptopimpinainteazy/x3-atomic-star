//! X3 Atomic Star V0.4 Readiness Report Generator
//!
//! This crate provides infrastructure for gathering and reporting on the production
//! readiness status of the X3 Atomic Star v0.4 kernel and related systems.

pub mod collector;
pub mod consensus_checks;
pub mod formatter;
pub mod gateway_checks;
pub mod invariants;
pub mod kernel_checks;
pub mod types;

pub use collector::Collector;
pub use formatter::{JsonFormatter, TextFormatter};
pub use types::ReadinessReport;

/// Main crate entrypoint used by dashboard/CLI consumers.
pub fn get_readiness() -> ReadinessReport {
	Collector::collect()
}

#[cfg(test)]
mod tests;
