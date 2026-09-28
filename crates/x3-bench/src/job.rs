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
/// Domain separator for run attestations.
/// v2: the attestation also commits to the backend the run executed on
/// ([`Placement`]). A v1 signature cannot verify as v2.
pub const ATTESTATION_DOMAIN: &[u8] = b"x3-bench-attestation-v2";

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

/// Where a job runs: the backend the Reactor's placement scheduler chose
/// (`northern_swarm::reactor::ScheduleDecision::placement`).
///
/// Plain strings, so this crate does not depend on the scheduler: the scheduler
/// converts its decision into this, and a run attestation commits to it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    /// The backend's stable identity (`cpu-0`, `gpu-0`, or a worker account id).
    pub backend_id: String,
    /// The accelerator class the backend runs on (`cpu`, `gpu`, `npu`, `fpga`).
    pub accelerator: String,
}

impl Placement {
    pub fn new(backend_id: impl Into<String>, accelerator: impl Into<String>) -> Self {
        Self {
            backend_id: backend_id.into(),
            accelerator: accelerator.into(),
        }
    }
}

/// A submitted job. Submission is the only way to get one; the id is derived,
/// never accepted from the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BenchmarkJob {
    id: [u8; 32],
    request: JobRequest,
    /// The backend the scheduler placed the job on, if it was placed. A placed
    /// job's report publishes only under an attestation from that backend.
    placement: Option<Placement>,
}

impl BenchmarkJob {
    /// Submit a job. Refuses a request whose identity would be meaningless.
    pub fn submit(request: JobRequest) -> Result<Self, PublishRefusal> {
        request.validate()?;
        let id = request.id();
        Ok(Self {
            id,
            request,
            placement: None,
        })
    }

    /// Submit a job the scheduler has already placed on a backend.
    ///
    /// The placement is not part of the job id: the id names *what* is measured,
    /// and the registry still publishes one report per job. What the placement
    /// changes is who may attest to it: only a run on this backend.
    pub fn submit_placed(
        request: JobRequest,
        placement: Placement,
    ) -> Result<Self, PublishRefusal> {
        if placement.backend_id.trim().is_empty() || placement.accelerator.trim().is_empty() {
            return Err(PublishRefusal::EmptyPlacement);
        }
        let mut job = Self::submit(request)?;
        job.placement = Some(placement);
        Ok(job)
    }

    /// The backend this job was placed on, if any.
    pub fn placement(&self) -> Option<&Placement> {
        self.placement.as_ref()
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
    /// The run attestation the report was published under, if the registry
    /// required one. `None` means this publication carries no proof about the
    /// run — which is what a registry with no trusted attester produces.
    pub attestation: Option<RunAttestation>,
}

/// What a run attestation claims: which job the measurement belongs to, at which
/// revision, on which host, with which configuration, over which samples — signed
/// by an attester the registry trusts.
///
/// Without this, publication is bound to the *job's* identity and nothing else:
/// a submitter can name any revision it likes, because nothing evidences that the
/// measurement was taken there. Every claim here is committed to by the
/// signature, and [`RunAttestation::verify`] refuses a claim that does not match
/// the job, the report, the revision, or the trusted key set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunAttestation {
    /// The job this measurement belongs to.
    pub job_id: [u8; 32],
    /// The revision the run was performed at.
    pub git_revision: String,
    /// Identity of the host that took the measurement.
    pub host_id: String,
    /// Digest of the build/run configuration the measurement was taken with.
    pub config_digest: [u8; 32],
    /// Digest of the samples the run produced.
    pub samples_digest: [u8; 32],
    /// The backend the run executed on, when the attester knows it. A placed
    /// job refuses an attestation that names another backend, or none.
    pub placement: Option<Placement>,
    /// Public key of the attester.
    pub signer: [u8; 32],
    /// Ed25519 signature over [`RunAttestation::signing_digest`].
    pub signature: Vec<u8>,
}

impl RunAttestation {
    /// The digest an attester signs: every claim, including who is making it, so
    /// a signature cannot be re-attributed to another key.
    pub fn signing_digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        absorb(&mut hasher, ATTESTATION_DOMAIN);
        absorb(&mut hasher, &self.job_id);
        absorb(&mut hasher, self.git_revision.as_bytes());
        absorb(&mut hasher, self.host_id.as_bytes());
        absorb(&mut hasher, &self.config_digest);
        absorb(&mut hasher, &self.samples_digest);
        match &self.placement {
            None => absorb(&mut hasher, &[0]),
            Some(placement) => {
                absorb(&mut hasher, &[1]);
                absorb(&mut hasher, placement.backend_id.as_bytes());
                absorb(&mut hasher, placement.accelerator.as_bytes());
            }
        }
        absorb(&mut hasher, &self.signer);
        hasher.finalize().into()
    }

    /// Build and sign a claim about one run, naming no backend.
    pub fn signed(
        key: &ed25519_dalek::SigningKey,
        job_id: [u8; 32],
        git_revision: impl Into<String>,
        host_id: impl Into<String>,
        config_digest: [u8; 32],
        samples_digest: [u8; 32],
    ) -> Self {
        Self::signed_with_placement(
            key,
            job_id,
            git_revision,
            host_id,
            config_digest,
            samples_digest,
            None,
        )
    }

    /// Build and sign a claim about one run, including the backend it ran on.
    pub fn signed_with_placement(
        key: &ed25519_dalek::SigningKey,
        job_id: [u8; 32],
        git_revision: impl Into<String>,
        host_id: impl Into<String>,
        config_digest: [u8; 32],
        samples_digest: [u8; 32],
        placement: Option<Placement>,
    ) -> Self {
        use ed25519_dalek::Signer;

        let mut attestation = Self {
            job_id,
            git_revision: git_revision.into(),
            host_id: host_id.into(),
            config_digest,
            samples_digest,
            placement,
            signer: key.verifying_key().to_bytes(),
            signature: Vec::new(),
        };
        attestation.signature = key.sign(&attestation.signing_digest()).to_bytes().to_vec();
        attestation
    }

    /// Check this claim against the job it names, the report's content digest, and
    /// the attesters the registry trusts. Every failure names which claim broke.
    pub fn verify(
        &self,
        trusted: &[[u8; 32]],
        job: &BenchmarkJob,
        report_digest: [u8; 32],
    ) -> Result<(), PublishRefusal> {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};

        if self.job_id != job.id {
            return Err(PublishRefusal::AttestedJobMismatch {
                expected: job.id,
                found: self.job_id,
            });
        }
        if self.git_revision != job.request.git_revision {
            return Err(PublishRefusal::RevisionMismatch {
                expected: job.request.git_revision.clone(),
                found: self.git_revision.clone(),
            });
        }
        if self.samples_digest != report_digest {
            return Err(PublishRefusal::SamplesDigestMismatch {
                expected: report_digest,
                found: self.samples_digest,
            });
        }
        if let Some(placed) = &job.placement {
            if self.placement.as_ref() != Some(placed) {
                return Err(PublishRefusal::PlacementMismatch {
                    expected: placed.clone(),
                    found: self.placement.clone(),
                });
            }
        }
        if !trusted.contains(&self.signer) {
            return Err(PublishRefusal::UntrustedSigner {
                signer: self.signer,
            });
        }
        let key =
            VerifyingKey::from_bytes(&self.signer).map_err(|_| PublishRefusal::BadSignature)?;
        let signature =
            Signature::from_slice(&self.signature).map_err(|_| PublishRefusal::BadSignature)?;
        key.verify(&self.signing_digest(), &signature)
            .map_err(|_| PublishRefusal::BadSignature)
    }
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
    /// This registry requires a run attestation and none was supplied.
    Unattested {
        job_id: [u8; 32],
    },
    /// The attestation names a different job than the one being published.
    AttestedJobMismatch {
        expected: [u8; 32],
        found: [u8; 32],
    },
    /// The attestation claims a revision the job did not ask for.
    RevisionMismatch {
        expected: String,
        found: String,
    },
    /// The attestation's sample digest is not the report's content digest.
    SamplesDigestMismatch {
        expected: [u8; 32],
        found: [u8; 32],
    },
    /// The signature is from a key this registry does not trust.
    UntrustedSigner {
        signer: [u8; 32],
    },
    /// The signature does not cover this attestation.
    BadSignature,
    /// A placement names no backend or no accelerator.
    EmptyPlacement,
    /// The job was placed on one backend and the attestation claims a run on
    /// another (or names none).
    PlacementMismatch {
        expected: Placement,
        found: Option<Placement>,
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
            PublishRefusal::Unattested { .. } => write!(
                f,
                "this registry requires a run attestation and none was supplied"
            ),
            PublishRefusal::AttestedJobMismatch { .. } => {
                write!(f, "the attestation names a different job")
            }
            PublishRefusal::RevisionMismatch { expected, found } => write!(
                f,
                "the attestation claims revision {found} but the job asked for {expected}"
            ),
            PublishRefusal::SamplesDigestMismatch { .. } => write!(
                f,
                "the attestation's sample digest is not the report's content digest"
            ),
            PublishRefusal::UntrustedSigner { signer } => write!(
                f,
                "the attestation is signed by {} which this registry does not trust",
                hex::encode(signer)
            ),
            PublishRefusal::BadSignature => {
                write!(f, "the attestation signature does not verify")
            }
            PublishRefusal::EmptyPlacement => {
                write!(f, "the placement names no backend or no accelerator")
            }
            PublishRefusal::PlacementMismatch { expected, found } => match found {
                Some(found) => write!(
                    f,
                    "the job was placed on {} ({}) but the attestation claims {} ({})",
                    expected.backend_id, expected.accelerator, found.backend_id, found.accelerator
                ),
                None => write!(
                    f,
                    "the job was placed on {} ({}) but the attestation names no backend",
                    expected.backend_id, expected.accelerator
                ),
            },
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
    /// Attester keys this registry accepts. Empty means "no attestation is
    /// required", which is what the local/dev path uses; a non-empty set makes an
    /// attestation mandatory, because a registry that knows who may attest has no
    /// reason to accept a report from nobody.
    trusted_attesters: Vec<[u8; 32]>,
}

impl ReportRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// A registry that accepts a report only from one of `attesters`.
    pub fn trusting(attesters: Vec<[u8; 32]>) -> Self {
        Self {
            published: BTreeMap::new(),
            trusted_attesters: attesters,
        }
    }

    /// Whether this registry refuses an unattested publication.
    pub fn requires_attestation(&self) -> bool {
        !self.trusted_attesters.is_empty()
    }

    /// The attester keys this registry accepts.
    pub fn trusted_attesters(&self) -> &[[u8; 32]] {
        &self.trusted_attesters
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
    ///
    /// A registry with trusted attesters **refuses this call**: evidence that
    /// says "somebody ran it" is not evidence of who ran it, and the only way to
    /// publish there is [`ReportRegistry::publish_attested`].
    pub fn publish(
        &mut self,
        job: &BenchmarkJob,
        report: Report,
    ) -> Result<[u8; 32], PublishRefusal> {
        if self.requires_attestation() {
            return Err(PublishRefusal::Unattested { job_id: job.id });
        }
        self.insert(job, report, None)
    }

    /// Publish `report` under a run attestation that has to verify against the
    /// job, the report and this registry's trusted attesters.
    pub fn publish_attested(
        &mut self,
        job: &BenchmarkJob,
        report: Report,
        attestation: &RunAttestation,
    ) -> Result<[u8; 32], PublishRefusal> {
        let digest = report_digest(&report);
        attestation.verify(&self.trusted_attesters, job, digest)?;
        self.insert(job, report, Some(attestation.clone()))
    }

    /// The same as [`ReportRegistry::publish`] with no attestation supplied.
    /// Kept separate so the strict path cannot be reached by accident.
    fn insert(
        &mut self,
        job: &BenchmarkJob,
        report: Report,
        attestation: Option<RunAttestation>,
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
                attestation,
            },
        );
        Ok(digest)
    }

    /// Re-check the attestation a report was published under.
    pub fn verify_attestation(
        &self,
        job: &BenchmarkJob,
        report: &Report,
    ) -> Result<(), PublishRefusal> {
        let published = self
            .published
            .get(&job.id)
            .ok_or(PublishRefusal::NotPublished { job_id: job.id })?;
        let attestation = published
            .attestation
            .as_ref()
            .ok_or(PublishRefusal::Unattested { job_id: job.id })?;
        attestation.verify(&self.trusted_attesters, job, report_digest(report))
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

    /// A fresh copy of the same one-sample report, for tests that need the same
    /// content twice (the API takes the report by value, so a second use needs a
    /// second value).
    fn measured_clone() -> Report {
        report(vec![sample("one", 10, 100)])
    }

    fn attester() -> ed25519_dalek::SigningKey {
        ed25519_dalek::SigningKey::from_bytes(&[7u8; 32])
    }

    fn strict_registry() -> ReportRegistry {
        ReportRegistry::trusting(vec![attester().verifying_key().to_bytes()])
    }

    fn attestation_for(job: &BenchmarkJob, rep: &Report) -> RunAttestation {
        RunAttestation::signed(
            &attester(),
            job.id(),
            job.request().git_revision.clone(),
            "x3star1",
            [0x11; 32],
            report_digest(rep),
        )
    }

    /// The gap this closes: publication used to be bound to the job's identity and
    /// nothing else, so a submitter could name any revision it liked. A registry
    /// that knows who may attest refuses a report with no attestation at all.
    #[test]
    fn a_strict_registry_refuses_an_unattested_report() {
        let job = BenchmarkJob::submit(request("abc123", &["one"])).unwrap();
        let mut registry = strict_registry();
        assert!(registry.requires_attestation());
        assert_eq!(registry.trusted_attesters().len(), 1);

        let refusal = registry
            .publish(&job, report(vec![sample("one", 10, 100)]))
            .expect_err("a strict registry must not accept an unattested report");
        assert!(
            matches!(refusal, PublishRefusal::Unattested { .. }),
            "expected Unattested, got {refusal:?}"
        );
        assert_eq!(registry.len(), 0, "the refused report was not stored");
    }

    #[test]
    fn a_signed_attestation_publishes_and_reverifies() {
        let job = BenchmarkJob::submit(request("abc123", &["one", "two"])).unwrap();
        let rep = report(vec![sample("one", 10, 100), sample("two", 20, 200)]);
        let attestation = attestation_for(&job, &rep);
        let expected = report_digest(&rep);

        let mut registry = strict_registry();
        let digest = registry
            .publish_attested(&job, rep, &attestation)
            .expect("a verified attestation publishes");
        assert_eq!(digest, expected);
        let stored = registry.get(&job.id()).unwrap();
        assert!(stored.attestation.is_some());
        assert_eq!(stored.attestation.as_ref().unwrap().host_id, "x3star1");
        let stored_report = &registry.get(&job.id()).unwrap().report;
        registry
            .verify_attestation(&job, stored_report)
            .expect("the stored attestation re-verifies");
    }

    #[test]
    fn a_report_that_is_not_what_the_attester_measured_is_refused() {
        let job = BenchmarkJob::submit(request("abc123", &["one"])).unwrap();
        let measured = report(vec![sample("one", 10, 100)]);
        let attestation = attestation_for(&job, &measured);
        // Same sample set, different numbers: the digest is the only thing that
        // ties the attestation to *this* measurement.
        let swapped = report(vec![sample("one", 9_999, 100)]);

        let mut registry = strict_registry();
        let refusal = registry
            .publish_attested(&job, swapped, &attestation)
            .expect_err("a report the attester did not measure must not publish");
        assert!(
            matches!(refusal, PublishRefusal::SamplesDigestMismatch { .. }),
            "expected SamplesDigestMismatch, got {refusal:?}"
        );
    }

    #[test]
    fn an_attestation_for_another_revision_or_job_is_refused() {
        let job = BenchmarkJob::submit(request("abc123", &["one"])).unwrap();
        let rep = report(vec![sample("one", 10, 100)]);

        let mut wrong_revision = attestation_for(&job, &rep);
        wrong_revision.git_revision = "def456".to_string();
        let mut registry = strict_registry();
        let refusal = registry
            .publish_attested(&job, measured_clone(), &wrong_revision)
            .expect_err("a claim about another revision must be refused");
        assert!(
            matches!(refusal, PublishRefusal::RevisionMismatch { .. }),
            "expected RevisionMismatch, got {refusal:?}"
        );

        // A different job (different sample set, so a different identity) must not
        // accept an attestation that was signed for the first one.
        let other = BenchmarkJob::submit(request("abc123", &["two"])).unwrap();
        let reused = attestation_for(&job, &measured_clone());
        let refusal = registry
            .publish_attested(&other, measured_clone(), &reused)
            .expect_err("a re-used attestation from another job must be refused");
        assert!(
            matches!(refusal, PublishRefusal::AttestedJobMismatch { .. }),
            "expected AttestedJobMismatch, got {refusal:?}"
        );
    }

    #[test]
    fn a_signature_from_an_untrusted_key_is_refused() {
        let job = BenchmarkJob::submit(request("abc123", &["one"])).unwrap();
        let rep = report(vec![sample("one", 10, 100)]);
        let stranger = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
        let attestation = RunAttestation::signed(
            &stranger,
            job.id(),
            "abc123",
            "not-a-trusted-host",
            [0x11; 32],
            report_digest(&rep),
        );

        let mut registry = strict_registry();
        let refusal = registry
            .publish_attested(&job, rep, &attestation)
            .expect_err("an unknown attester must be refused");
        assert!(
            matches!(refusal, PublishRefusal::UntrustedSigner { .. }),
            "expected UntrustedSigner, got {refusal:?}"
        );
        assert!(refusal.to_string().contains("does not trust"));
    }

    #[test]
    fn a_tampered_attestation_does_not_verify() {
        let job = BenchmarkJob::submit(request("abc123", &["one"])).unwrap();
        let rep = report(vec![sample("one", 10, 100)]);
        let mut attestation = attestation_for(&job, &rep);
        // Re-attribute the run to another host without re-signing.
        attestation.host_id = "somebody-elses-box".to_string();

        let mut registry = strict_registry();
        let refusal = registry
            .publish_attested(&job, rep, &attestation)
            .expect_err("a tampered claim must not verify");
        assert!(
            matches!(refusal, PublishRefusal::BadSignature),
            "{refusal:?}"
        );
    }

    /// The local path stays usable and *says* it carries no proof about the run,
    /// so a reader is never left to assume one.
    #[test]
    fn the_local_registry_publishes_and_reports_that_it_is_unattested() {
        let job = BenchmarkJob::submit(request("abc123", &["one"])).unwrap();
        let rep = report(vec![sample("one", 10, 100)]);
        let mut registry = ReportRegistry::new();
        assert!(!registry.requires_attestation());
        registry.publish(&job, rep).expect("local publish works");
        assert!(
            registry.get(&job.id()).unwrap().attestation.is_none(),
            "an unattested publication must not look attested"
        );
        let stored_report = &registry.get(&job.id()).unwrap().report;
        assert!(matches!(
            registry.verify_attestation(&job, stored_report),
            Err(PublishRefusal::Unattested { .. })
        ));
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

    fn placed_attestation(
        job: &BenchmarkJob,
        rep: &Report,
        on: Option<Placement>,
    ) -> RunAttestation {
        RunAttestation::signed_with_placement(
            &attester(),
            job.id(),
            job.request().git_revision.clone(),
            "x3star1",
            [0x11; 32],
            report_digest(rep),
            on,
        )
    }

    /// The Reactor's two halves meet here: the scheduler placed the job on
    /// `cpu-0`, so only a run attested on `cpu-0` publishes, and the published
    /// report says so.
    #[test]
    fn a_placed_job_publishes_only_under_an_attestation_from_its_backend() {
        let placed = Placement::new("cpu-0", "cpu");
        let job = BenchmarkJob::submit_placed(request("abc123", &["one"]), placed.clone()).unwrap();
        assert_eq!(job.placement(), Some(&placed));
        assert_eq!(
            job.id(),
            BenchmarkJob::submit(request("abc123", &["one"]))
                .unwrap()
                .id(),
            "placement does not change what the job measures"
        );
        let mut registry = strict_registry();

        let elsewhere = placed_attestation(
            &job,
            &measured_clone(),
            Some(Placement::new("gpu-0", "gpu")),
        );
        assert_eq!(
            registry.publish_attested(&job, measured_clone(), &elsewhere),
            Err(PublishRefusal::PlacementMismatch {
                expected: placed.clone(),
                found: Some(Placement::new("gpu-0", "gpu")),
            }),
            "a run on another backend is not the run the scheduler placed"
        );
        let nowhere = placed_attestation(&job, &measured_clone(), None);
        assert_eq!(
            registry.publish_attested(&job, measured_clone(), &nowhere),
            Err(PublishRefusal::PlacementMismatch {
                expected: placed.clone(),
                found: None,
            }),
            "an attestation that names no backend cannot vouch for a placed job"
        );
        assert!(registry.is_empty(), "neither refusal stored anything");

        let here = placed_attestation(&job, &measured_clone(), Some(placed.clone()));
        registry
            .publish_attested(&job, measured_clone(), &here)
            .expect("the placed backend's own attestation publishes");
        let stored = registry.get(&job.id()).unwrap();
        assert_eq!(
            stored
                .attestation
                .as_ref()
                .and_then(|a| a.placement.as_ref()),
            Some(&placed),
            "the published report says which backend ran the job"
        );
        registry
            .verify_attestation(&job, &measured_clone())
            .expect("and it re-verifies");
    }

    /// The backend is a signed claim: rewriting it after signing breaks the
    /// signature, so a report cannot be re-labelled as having run elsewhere.
    #[test]
    fn the_attested_backend_is_covered_by_the_signature() {
        let job = BenchmarkJob::submit(request("abc123", &["one"])).unwrap();
        let mut attestation = placed_attestation(
            &job,
            &measured_clone(),
            Some(Placement::new("cpu-0", "cpu")),
        );
        attestation.placement = Some(Placement::new("gpu-0", "gpu"));
        assert_eq!(
            strict_registry().publish_attested(&job, measured_clone(), &attestation),
            Err(PublishRefusal::BadSignature)
        );
        attestation.placement = None;
        assert_eq!(
            strict_registry().publish_attested(&job, measured_clone(), &attestation),
            Err(PublishRefusal::BadSignature),
            "dropping the backend claim breaks the signature too"
        );
    }

    #[test]
    fn a_placement_with_no_backend_is_refused() {
        for placement in [Placement::new(" ", "cpu"), Placement::new("cpu-0", "")] {
            assert_eq!(
                BenchmarkJob::submit_placed(request("abc123", &["one"]), placement),
                Err(PublishRefusal::EmptyPlacement)
            );
        }
    }
}
