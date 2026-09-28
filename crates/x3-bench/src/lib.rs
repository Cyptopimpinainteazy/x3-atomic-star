#![allow(
    dead_code,
    unused_imports,
    unused_variables,
    unused_mut,
    non_snake_case,
    unexpected_cfgs,
    unused_parens,
    non_camel_case_types,
    clippy::all
)]

//! X3 Benchmark Suite
//! Benchmark infrastructure for measuring optimizer effectiveness.

pub mod comparator;
pub mod job;
pub mod pipeline;
pub mod runner;
pub mod samples;

pub use comparator::{
    compare_reports, read_report, write_report, GlobalMetrics, Report, SampleMetrics,
};
pub use job::{
    report_digest, BenchmarkJob, JobRequest, Placement, PublishRefusal, PublishedReport,
    ReportRegistry, RunAttestation, ATTESTATION_DOMAIN, JOB_DOMAIN, REPORT_DOMAIN,
};
