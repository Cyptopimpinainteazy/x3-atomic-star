//! Benchmark job submission and report publication.
//!
//! The Reactor release item asks for a benchmark *submission* path and a
//! benchmark *publication* path. The crate had neither: it could run a suite and
//! write a JSON file, and `X3-REACTOR-001` recorded the consequence — the two
//! names the registry used to cite, `benchmark_job_submits` and
//! `benchmark_result_publishes`, never existed anywhere in the repository.
//!
//! This is the smallest honest version of both. A [`JobRequest`] has a
//! deterministic identity, so the same request always names the same job and a
//! different revision never collides with it. A [`ReportRegistry`] is
//! append-only: a job's report can be published once, the recorded digest is the
//! one a later reader can re-check, and a report that does not belong to the
//! job's sample set is refused rather than stored.
//!
//! The digest deliberately excludes [`crate::comparator::Report::timestamp`].
//! That field is wall-clock, so digesting it would make two runs of identical
//! content disagree — the digest would then be measuring the clock, not the
//! report. Every other field is hashed.

use crate::comparator::Report;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Domain separator for job identities.
pub const JOB_DOMAIN: &[u8] = b"x3-bench-job-v1";
/// Domain separator for report digests. Distinct from the job domain so a job id
/// can never be mistaken for a report digest.
pub const REPORT_DOMAIN: &[u8] = b"x3-bench-report-v1";

/// Absorb a length-prefixed field, so `("ab", "c")` and `("a", "bc")` cannot
/// commit to the same bytes.
fn absorb(hasher: &mut Sha256, field: &[u8]) {
    hasher.update((field.len() as u64).to_le_bytes());
    hasher.update(field);
}

/// What a benchmark job asks for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobRequest {
    /// Named suite the job must produce samples for.
    pub suite: String,
    /// Revision the measurement is claimed to be of.
    pub git_revision: String,
    /// Optimizer iteration bound the run was configured with.
    pub max_opt_iters: usize,
    /// The exact sample names the report must contain, in the order requested.
    pub sample_names: Vec<String>,
}

impl JobRequest {
    /// The job's deterministic identity.
    ///
    /// The sample names are sorted before hashing, so two requests that name the
    /// same set in a different order are the same job. Everything else is
    /// order-sensitive by construction.
    pub fn id(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        absorb(&mut hasher, JOB_DOMAIN);
        absorb(&mut hasher, self.suite.as_bytes());
        absorb(&mut hasher, self.git_revision.as_bytes());
        absorb(&mut hasher, &self.max_opt_iters.to_le_bytes());
        let mut names: Vec<&String> = self.sample_names.iter().collect();
        names.sort();
        for name in names {
            absorb(&mut hasher, name.as_bytes());
        }
        hasher.finalize().into()
    }

    /// Refuse a request that could not be satisfied or checked.
    fn validate(&self) -> Result<(), PublishRefusal> {
        if self.suite.trim().is_empty() {
            return Err(PublishRefusal::EmptySuite);
        }
        if self.git_revision.trim().is_empty() {
            return Err(PublishRefusal::EmptyRevision);
        }
        if self.sample_names.is_empty() {
            return Err(PublishRefusal::NoSamples);
        }
        let mut seen: Vec<&str> = Vec::new();
        for name in &self.sample_names {
            if name.trim().is_empty() {
                return Err(PublishRefusal::EmptySampleName);
            }
            if seen.contains(&name.as_str()) {
                return Err(PublishRefusal::DuplicateSampleName { name: name.clone() });
            }
            seen.push(name);
        }
        Ok(())
    }
}

/// A submitted job. Submission is the only way to get one; the id is derived,
/// never accepted from the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BenchmarkJob {
    id: [u8; 32],
    request: JobRequest,
}

impl BenchmarkJob {
    /// Submit a job. Refuses a request whose identity would be meaningless.
    pub fn submit(request: JobRequest) -> Result<Self, PublishRefusal> {
        request.validate()?;
        let id = request.id();
        Ok(Self { id, request })
    }

    pub fn id(&self) -> [u8; 32] {
        self.id
    }

    pub fn request(&self) -> &JobRequest {
        &self.request
    }
}

/// A published report and the identity it was published under.
#[derive(Debug)]
pub struct PublishedReport {
    pub job_id: [u8; 32],
    pub report: Report,
    pub digest: [u8; 32],
}

/// Why a submission, a publication or a verification was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublishRefusal {
    EmptySuite,
    EmptyRevision,
    NoSamples,
    EmptySampleName,
    DuplicateSampleName {
        name: String,
    },
    /// A report was published for this job already. The registry is append-only;
    /// a second publication is refused rather than silently replacing evidence.
    AlreadyPublished {
        job_id: [u8; 32],
    },
    /// The report does not cover the samples the job asked for.
    SampleSetMismatch {
        expected: Vec<String>,
        found: Vec<String>,
    },
    /// Nothing is published under this job id.
    NotPublished {
        job_id: [u8; 32],
    },
    /// The report's content does not hash to what was published.
    DigestMismatch {
        published: [u8; 32],
        recomputed: [u8; 32],
    },
}

impl core::fmt::Display for PublishRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PublishRefusal::EmptySuite => write!(f, "the job request names no suite"),
            PublishRefusal::EmptyRevision => write!(f, "the job request names no revision"),
            PublishRefusal::NoSamples => write!(f, "the job request names no samples"),
            PublishRefusal::EmptySampleName => write!(f, "the job request names an empty sample"),
            PublishRefusal::DuplicateSampleName { name } => {
                write!(f, "the job request names sample {name} twice")
            }
            PublishRefusal::AlreadyPublished { .. } => {
                write!(f, "this job already has a published report")
            }
            PublishRefusal::SampleSetMismatch { expected, found } => write!(
                f,
                "the report covers {found:?} but the job asked for {expected:?}"
            ),
            PublishRefusal::NotPublished { .. } => write!(f, "nothing is published for this job"),
            PublishRefusal::DigestMismatch { .. } => {
                write!(
                    f,
                    "the report's content does not match the published digest"
                )
            }
        }
    }
}

impl std::error::Error for PublishRefusal {}

/// The content digest of a report, excluding its wall-clock timestamp.
pub fn report_digest(report: &Report) -> [u8; 32] {
    let mut hasher = Sha256::new();
    absorb(&mut hasher, REPORT_DOMAIN);
    absorb(&mut hasher, &(report.global.instr as u64).to_le_bytes());
    absorb(&mut hasher, &report.global.gas.to_le_bytes());
    absorb(&mut hasher, &(report.global.bytes as u64).to_le_bytes());
    let mut samples: Vec<(&String, usize, u64, usize)> = report
        .per_sample
        .iter()
        .map(|s| (&s.name, s.instr, s.gas, s.bytes))
        .collect();
    // Order-independent: a report is a set of measurements, and the registry
    // compares sets, not orderings.
    samples.sort();
    for (name, instr, gas, bytes) in samples {
        absorb(&mut hasher, name.as_bytes());
        absorb(&mut hasher, &(instr as u64).to_le_bytes());
        absorb(&mut hasher, &gas.to_le_bytes());
        absorb(&mut hasher, &(bytes as u64).to_le_bytes());
    }
    hasher.finalize().into()
}

/// Append-only record of published benchmark reports.
#[derive(Debug, Default)]
pub struct ReportRegistry {
    published: BTreeMap<[u8; 32], PublishedReport>,
}

impl ReportRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.published.len()
    }

    pub fn is_empty(&self) -> bool {
        self.published.is_empty()
    }

    pub fn get(&self, job_id: &[u8; 32]) -> Option<&PublishedReport> {
        self.published.get(job_id)
    }

    pub fn is_published(&self, job_id: &[u8; 32]) -> bool {
        self.published.contains_key(job_id)
    }

    /// Publish `report` as the result of `job`.
    ///
    /// Returns the digest a reader can re-check. A job publishes once; a report
    /// that does not cover the job's samples is not stored.
    pub fn publish(
        &mut self,
        job: &BenchmarkJob,
        report: Report,
    ) -> Result<[u8; 32], PublishRefusal> {
        if self.published.contains_key(&job.id) {
            return Err(PublishRefusal::AlreadyPublished { job_id: job.id });
        }

        let mut found: Vec<String> = report
            .per_sample
            .iter()
            .map(|sample| sample.name.clone())
            .collect();
        let mut expected: Vec<String> = job.request.sample_names.clone();
        if found.is_empty() {
            return Err(PublishRefusal::NoSamples);
        }
        found.sort();
        expected.sort();
        if found != expected {
            return Err(PublishRefusal::SampleSetMismatch { expected, found });
        }

        let digest = report_digest(&report);
        self.published.insert(
            job.id,
            PublishedReport {
                job_id: job.id,
                report,
                digest,
            },
        );
        Ok(digest)
    }

    /// Re-check a report against what was published for `job`.
    pub fn verify(&self, job_id: &[u8; 32], report: &Report) -> Result<(), PublishRefusal> {
        let published = self
            .published
            .get(job_id)
            .ok_or(PublishRefusal::NotPublished { job_id: *job_id })?;
        let recomputed = report_digest(report);
        if recomputed != published.digest {
            return Err(PublishRefusal::DigestMismatch {
                published: published.digest,
                recomputed,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comparator::SampleMetrics;

    fn sample(name: &str, instr: usize, gas: u64) -> SampleMetrics {
        SampleMetrics {
            name: name.to_string(),
            instr,
            gas,
            bytes: instr / 2,
        }
    }

    fn request(revision: &str, samples: &[&str]) -> JobRequest {
        JobRequest {
            suite: "optimizer-core".to_string(),
            git_revision: revision.to_string(),
            max_opt_iters: 3,
            sample_names: samples.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn report(samples: Vec<SampleMetrics>) -> Report {
        Report::new(samples)
    }

    #[test]
    fn a_submitted_job_has_a_derived_identity() {
        let a = BenchmarkJob::submit(request("abc123", &["one", "two"])).unwrap();
        let b = BenchmarkJob::submit(request("abc123", &["two", "one"])).unwrap();
        let c = BenchmarkJob::submit(request("def456", &["one", "two"])).unwrap();

        assert_eq!(
            a.id(),
            b.id(),
            "sample order must not change a job's identity"
        );
        assert_ne!(
            a.id(),
            c.id(),
            "a different revision must not share an identity"
        );
    }

    #[test]
    fn benchmark_job_submits_and_result_publishes() {
        let job = BenchmarkJob::submit(request("abc123", &["one", "two"])).unwrap();
        let mut registry = ReportRegistry::new();
        assert!(registry.is_empty());

        let digest = registry
            .publish(
                &job,
                report(vec![sample("one", 10, 100), sample("two", 20, 200)]),
            )
            .expect("a report covering the requested samples must publish");

        assert_eq!(registry.len(), 1);
        assert!(registry.is_published(&job.id()));
        let published = registry.get(&job.id()).expect("recorded");
        assert_eq!(published.job_id, job.id());
        assert_eq!(published.digest, digest);
        assert!(registry.verify(&job.id(), &published.report).is_ok());
    }

    #[test]
    fn a_report_for_another_suite_is_refused() {
        let job = BenchmarkJob::submit(request("abc123", &["one", "two"])).unwrap();
        let mut registry = ReportRegistry::new();

        let err = registry
            .publish(&job, report(vec![sample("one", 10, 100)]))
            .unwrap_err();
        assert!(matches!(err, PublishRefusal::SampleSetMismatch { .. }));
        assert!(registry.is_empty(), "a refused report must not be stored");
    }

    #[test]
    fn publishing_the_same_job_twice_is_refused() {
        let job = BenchmarkJob::submit(request("abc123", &["one"])).unwrap();
        let mut registry = ReportRegistry::new();
        let first = registry
            .publish(&job, report(vec![sample("one", 10, 100)]))
            .unwrap();

        let err = registry
            .publish(&job, report(vec![sample("one", 999, 999)]))
            .unwrap_err();
        assert_eq!(err, PublishRefusal::AlreadyPublished { job_id: job.id() });
        assert_eq!(
            registry.get(&job.id()).unwrap().digest,
            first,
            "the first publication must survive the refused overwrite"
        );
    }

    #[test]
    fn a_report_with_no_samples_is_refused() {
        let job = BenchmarkJob::submit(request("abc123", &["one"])).unwrap();
        let mut registry = ReportRegistry::new();
        assert_eq!(
            registry.publish(&job, report(Vec::new())).unwrap_err(),
            PublishRefusal::NoSamples
        );
    }

    #[test]
    fn a_request_with_duplicate_sample_names_is_refused() {
        assert_eq!(
            BenchmarkJob::submit(request("abc123", &["one", "one"])).unwrap_err(),
            PublishRefusal::DuplicateSampleName {
                name: "one".to_string()
            }
        );
    }

    #[test]
    fn verify_detects_a_tampered_report() {
        let job = BenchmarkJob::submit(request("abc123", &["one"])).unwrap();
        let mut registry = ReportRegistry::new();
        registry
            .publish(&job, report(vec![sample("one", 10, 100)]))
            .unwrap();

        let tampered = report(vec![sample("one", 10, 101)]);
        let err = registry.verify(&job.id(), &tampered).unwrap_err();
        assert!(matches!(err, PublishRefusal::DigestMismatch { .. }));
    }

    #[test]
    fn verify_of_an_unpublished_job_is_refused() {
        let job = BenchmarkJob::submit(request("abc123", &["one"])).unwrap();
        let registry = ReportRegistry::new();
        assert_eq!(
            registry
                .verify(&job.id(), &report(vec![sample("one", 10, 100)]))
                .unwrap_err(),
            PublishRefusal::NotPublished { job_id: job.id() }
        );
    }

    #[test]
    fn the_report_digest_ignores_the_wall_clock() {
        let mut a = report(vec![sample("one", 10, 100)]);
        let mut b = report(vec![sample("one", 10, 100)]);
        a.timestamp = "2026-01-01T00:00:00".to_string();
        b.timestamp = "2026-09-27T05:00:00".to_string();
        assert_eq!(
            report_digest(&a),
            report_digest(&b),
            "the digest must measure the report, not the clock"
        );
    }
}
