//! An agent must not be able to authorize itself into a production action.
//!
//! These tests drive the real verification path — the same
//! `ApprovalRequirement::is_satisfied` the crate ships — with real Ed25519 keys
//! and real signatures, so they would catch a change that made the gate pass on
//! weaker evidence. Keys come from fixed seeds rather than `OsRng`, so the suite
//! is deterministic.

use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use x3_swarm_core::policy::{ApprovalContext, GovernanceChecker, ReviewerRegistry};
use x3_swarm_core::{
    ApprovalGate, SensitiveAction, SensitiveRequest, SensitiveRefusal,
};

/// Deterministic signing key from a single-byte seed.
fn key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn verifying(seed: u8) -> VerifyingKey {
    key(seed).verifying_key()
}

/// One `[32-byte pubkey || 64-byte signature]` entry, the format the quorum
/// verifier expects.
fn entry(signer: &SigningKey, commitment: &[u8; 32]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(96);
    buf.extend_from_slice(signer.verifying_key().as_bytes());
    buf.extend_from_slice(&signer.sign(&commitment[..]).to_bytes());
    buf
}

fn quorum(signers: &[&SigningKey], commitment: &[u8; 32]) -> Vec<u8> {
    let mut buf = Vec::new();
    for signer in signers {
        buf.extend_from_slice(&entry(signer, commitment));
    }
    buf
}

struct Council {
    council_keys: Vec<VerifyingKey>,
    human_keys: Vec<VerifyingKey>,
}

impl Council {
    fn of(seeds: &[u8]) -> Self {
        Self {
            council_keys: seeds.iter().copied().map(verifying).collect(),
            human_keys: Vec::new(),
        }
    }
}

impl ReviewerRegistry for Council {
    fn human_reviewer_keys(&self) -> &[VerifyingKey] {
        &self.human_keys
    }
    fn security_council_keys(&self) -> &[VerifyingKey] {
        &self.council_keys
    }
}

/// Authorizes exactly the `(proposal_id, action_hash)` pairs it was built with.
struct Ledger {
    authorized: Vec<([u8; 32], [u8; 32])>,
}

impl GovernanceChecker for Ledger {
    fn is_proposal_authorized(&self, proposal_id: &[u8; 32], action_hash: &[u8; 32]) -> bool {
        self.authorized
            .iter()
            .any(|(pid, ah)| pid == proposal_id && ah == action_hash)
    }
}

fn runtime_upgrade_request() -> SensitiveRequest {
    SensitiveRequest::new(SensitiveAction::RuntimeUpgrade, "runtime-wasm-v2").unwrap()
}

fn supply_change_request() -> SensitiveRequest {
    SensitiveRequest::new(SensitiveAction::TokenSupplyChange, "emission-schedule").unwrap()
}

/// The gate refuses everything it is not given evidence for, using only
/// `is_satisfied`, so the baseline is a refusal rather than a pass.
#[test]
fn no_evidence_is_refused() {
    let gate = ApprovalGate::for_action(SensitiveAction::RuntimeUpgrade);
    let err = gate
        .authorize(
            &runtime_upgrade_request(),
            &ApprovalContext::default(),
            Some(&Council::of(&[1, 2, 3])),
            None,
        )
        .unwrap_err();
    assert!(
        matches!(err, SensitiveRefusal::Unmet { .. }),
        "expected Unmet, got {err:?}"
    );
}

#[test]
fn two_of_three_council_members_authorize_a_runtime_upgrade() {
    let council = Council::of(&[11, 12, 13]);
    let request = runtime_upgrade_request();
    let commitment = request.commitment();

    let evidence = ApprovalContext {
        security_quorum_sig: Some(quorum(&[&key(11), &key(12)], &commitment)),
        ..Default::default()
    };

    ApprovalGate::for_action(SensitiveAction::RuntimeUpgrade)
        .authorize(&request, &evidence, Some(&council), None)
        .expect("2-of-3 over the request's own commitment must authorize");
}

#[test]
fn one_of_three_is_not_a_quorum() {
    let council = Council::of(&[11, 12, 13]);
    let request = runtime_upgrade_request();
    let commitment = request.commitment();

    let evidence = ApprovalContext {
        security_quorum_sig: Some(quorum(&[&key(11)], &commitment)),
        ..Default::default()
    };

    assert_eq!(
        ApprovalGate::for_action(SensitiveAction::RuntimeUpgrade)
            .authorize(&request, &evidence, Some(&council), None)
            .unwrap_err(),
        SensitiveRefusal::Unmet {
            action: SensitiveAction::RuntimeUpgrade,
            requirement: x3_swarm_core::policy::ApprovalRequirement::SecurityReview,
        }
    );
}

#[test]
fn the_same_council_member_twice_is_not_a_quorum() {
    let council = Council::of(&[11, 12, 13]);
    let request = runtime_upgrade_request();
    let commitment = request.commitment();

    // The verifier de-duplicates signers; two copies of one member's entry must
    // not be counted as two approvals.
    let evidence = ApprovalContext {
        security_quorum_sig: Some(quorum(&[&key(11), &key(11)], &commitment)),
        ..Default::default()
    };

    assert!(ApprovalGate::for_action(SensitiveAction::RuntimeUpgrade)
        .authorize(&request, &evidence, Some(&council), None)
        .is_err());
}

#[test]
fn a_key_outside_the_council_is_refused() {
    let council = Council::of(&[11, 12, 13]);
    let request = runtime_upgrade_request();
    let commitment = request.commitment();

    // Two valid signatures from keys the council does not recognise.
    let evidence = ApprovalContext {
        security_quorum_sig: Some(quorum(&[&key(90), &key(91)], &commitment)),
        ..Default::default()
    };

    assert!(ApprovalGate::for_action(SensitiveAction::RuntimeUpgrade)
        .authorize(&request, &evidence, Some(&council), None)
        .is_err());
}

#[test]
fn a_one_member_council_cannot_authorize_a_runtime_upgrade() {
    // `security_council_threshold` is ceil(2/3), which is one for a council of
    // one — so the signature verifies and the quorum is "met". The gate refuses
    // before that, because a quorum of one is not a quorum.
    let council = Council::of(&[11]);
    let request = runtime_upgrade_request();
    let evidence = ApprovalContext {
        security_quorum_sig: Some(quorum(&[&key(11)], &request.commitment())),
        ..Default::default()
    };

    assert_eq!(
        ApprovalGate::for_action(SensitiveAction::RuntimeUpgrade)
            .authorize(&request, &evidence, Some(&council), None)
            .unwrap_err(),
        SensitiveRefusal::CouncilTooSmall {
            size: 1,
            minimum: x3_swarm_core::approval::MIN_SECURITY_COUNCIL_SIZE,
        }
    );
}

#[test]
fn a_signature_over_a_different_action_does_not_authorize_this_one() {
    // The council knowingly approves a *supply change*. The same signatures are
    // then presented for a runtime upgrade. They cover a different commitment,
    // so they verify against nothing.
    let council = Council::of(&[21, 22, 23]);
    let approved = supply_change_request();
    let presented_for = runtime_upgrade_request();

    let stolen = quorum(&[&key(21), &key(22)], &approved.commitment());
    let evidence = ApprovalContext {
        security_quorum_sig: Some(stolen),
        ..Default::default()
    };

    assert!(
        ApprovalGate::for_action(SensitiveAction::RuntimeUpgrade)
            .authorize(&presented_for, &evidence, Some(&council), None)
            .is_err(),
        "an approval for one action must not authorize another"
    );
}

#[test]
fn a_foreign_approval_hash_is_refused_loudly() {
    // The same attempt, but the caller also edits the context's `action_hash` to
    // the value it holds signatures for. The gate derives its own commitment and
    // names the mismatch instead of quietly substituting it.
    let council = Council::of(&[21, 22, 23]);
    let approved = supply_change_request();
    let presented_for = runtime_upgrade_request();

    let evidence = ApprovalContext {
        action_hash: Some(approved.commitment()),
        security_quorum_sig: Some(quorum(&[&key(21), &key(22)], &approved.commitment())),
        ..Default::default()
    };

    assert_eq!(
        ApprovalGate::for_action(SensitiveAction::RuntimeUpgrade)
            .authorize(&presented_for, &evidence, Some(&council), None)
            .unwrap_err(),
        SensitiveRefusal::HashMismatch {
            presented: approved.commitment(),
            derived: presented_for.commitment(),
        }
    );
}

#[test]
fn a_governance_proposal_for_this_commitment_authorizes_a_supply_change() {
    let request = supply_change_request();
    let proposal_id = [7u8; 32];
    let ledger = Ledger {
        authorized: vec![(proposal_id, request.commitment())],
    };
    let evidence = ApprovalContext {
        governance_proposal_id: Some(proposal_id),
        ..Default::default()
    };

    ApprovalGate::for_action(SensitiveAction::TokenSupplyChange)
        .authorize(&request, &evidence, None, Some(&ledger))
        .expect("an executed proposal naming this commitment must authorize the change");
}

#[test]
fn a_governance_proposal_for_another_action_does_not_authorize_a_supply_change() {
    let request = supply_change_request();
    let proposal_id = [7u8; 32];
    // The proposal authorized a *validator key replacement*, not the supply
    // change now being asked for.
    let other = SensitiveRequest::new(
        SensitiveAction::ValidatorKeyReplacement,
        "validator-01-session-key",
    )
    .unwrap();
    let ledger = Ledger {
        authorized: vec![(proposal_id, other.commitment())],
    };
    let evidence = ApprovalContext {
        governance_proposal_id: Some(proposal_id),
        ..Default::default()
    };

    assert!(ApprovalGate::for_action(SensitiveAction::TokenSupplyChange)
        .authorize(&request, &evidence, None, Some(&ledger))
        .is_err());
}

#[test]
fn an_empty_subject_cannot_be_authorized() {
    let err = SensitiveRequest::new(SensitiveAction::RuntimeUpgrade, "  ").unwrap_err();
    assert!(matches!(err, SensitiveRefusal::EmptySubject { .. }));
}
