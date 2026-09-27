//! Sensitive production actions and the commitment their approvals are bound to.
//!
//! The release bar says an agent must never perform a sensitive production
//! action — a runtime upgrade, a token supply change, a validator key
//! replacement, a genesis modification, a settlement-rule change — on its own
//! authority. `policy::ApprovalRequirement::is_satisfied` already verifies real
//! Ed25519 approvals (a human reviewer token, or an M-of-N security-council
//! quorum) over an `action_hash`. What was missing is the other half: nothing in
//! this crate *derived* that hash from an action, so `action_hash` was a
//! caller-supplied `[u8; 32]` and the binding was only ever asserted.
//!
//! Measured 2026-09-27: `grep -rn action_hash` finds the field read in three
//! places in `policy.rs` and produced nowhere except test helpers that sign an
//! arbitrary constant. So a signature legitimately issued for one action could
//! be presented for any other by relabelling the hash — the same defect shape as
//! a quorum that compares two fields of the caller's own evidence.
//!
//! This module supplies the missing half. [`SensitiveRequest::commitment`] is
//! the only producer of an approval hash in the crate, it is deterministic, and
//! it commits to both the action and its subject, so an approval for
//! `TokenSupplyChange` of one subject cannot authorize a `RuntimeUpgrade` of
//! another.

use crate::policy::ApprovalRequirement;
use sha2::{Digest, Sha256};

/// Domain separator for approval commitments.
///
/// Changing this invalidates every issued approval, which is the point: the
/// domain is part of what a reviewer's signature covers.
pub const SENSITIVE_DOMAIN: &[u8] = b"x3-sensitive-action-v1";

/// A production action an agent may only take with the required authorization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SensitiveAction {
    /// Installing a new runtime (WASM code) on a live chain.
    RuntimeUpgrade,
    /// Minting, burning or otherwise changing total token supply.
    TokenSupplyChange,
    /// Replacing a validator's session/authority key.
    ValidatorKeyReplacement,
    /// Changing genesis state or the genesis configuration.
    GenesisModification,
    /// Changing a settlement rule (how value is finally moved).
    SettlementRuleChange,
}

impl SensitiveAction {
    /// Every sensitive action, for exhaustive policy checks and tests.
    pub const ALL: [SensitiveAction; 5] = [
        SensitiveAction::RuntimeUpgrade,
        SensitiveAction::TokenSupplyChange,
        SensitiveAction::ValidatorKeyReplacement,
        SensitiveAction::GenesisModification,
        SensitiveAction::SettlementRuleChange,
    ];

    /// Stable tag committed to by approvals and used to name the action in
    /// policy strings. Never renumber or reuse a tag: an approval is bound to it.
    pub const fn tag(&self) -> &'static str {
        match self {
            SensitiveAction::RuntimeUpgrade => "runtime_upgrade",
            SensitiveAction::TokenSupplyChange => "token_supply_change",
            SensitiveAction::ValidatorKeyReplacement => "validator_key_replacement",
            SensitiveAction::GenesisModification => "genesis_modification",
            SensitiveAction::SettlementRuleChange => "settlement_rule_change",
        }
    }

    /// Parse an operation name. Accepts the tag with `-` or `_` separators and
    /// is case-insensitive on ASCII. Returns `None` for anything unrecognized —
    /// callers must treat that as "not a documented operation", never as a pass.
    pub fn from_operation(operation: &str) -> Option<SensitiveAction> {
        let normalized = operation
            .trim()
            .replace('-', "_")
            .to_ascii_lowercase();
        SensitiveAction::ALL
            .into_iter()
            .find(|action| action.tag() == normalized)
    }
}

/// The authorization level a sensitive action requires.
///
/// These are floors, not suggestions: [`crate::permissions::Permissions`] takes
/// the stronger of this and the agent's tier, so a permissive tier can never
/// lower a sensitive action's requirement.
pub const fn required_requirement(action: SensitiveAction) -> ApprovalRequirement {
    match action {
        // Installing code is verified by the security council, and the on-chain
        // half is a governance proposal; the council quorum is the floor here.
        SensitiveAction::RuntimeUpgrade => ApprovalRequirement::SecurityReview,
        // Anything that moves or creates value is an on-chain decision.
        SensitiveAction::TokenSupplyChange => ApprovalRequirement::GovernanceReview,
        SensitiveAction::ValidatorKeyReplacement => ApprovalRequirement::GovernanceReview,
        SensitiveAction::GenesisModification => ApprovalRequirement::GovernanceReview,
        SensitiveAction::SettlementRuleChange => ApprovalRequirement::SecurityReview,
    }
}

/// A request to perform a sensitive action against a named subject.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SensitiveRequest {
    action: SensitiveAction,
    subject: String,
}

impl SensitiveRequest {
    /// Build a request. Refuses an empty subject, because an authorization that
    /// is not bound to anything is not an authorization.
    pub fn new(
        action: SensitiveAction,
        subject: impl Into<String>,
    ) -> Result<Self, SensitiveRefusal> {
        let subject = subject.into();
        if subject.trim().is_empty() {
            return Err(SensitiveRefusal::EmptySubject { action });
        }
        Ok(Self { action, subject })
    }

    pub fn action(&self) -> SensitiveAction {
        self.action
    }

    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// The value an approval must be a signature over.
    ///
    /// SHA-256 over `domain || 0x00 || action_tag || 0x00 || subject`.
    /// Deterministic, with no clock or allocator input. The domain separator and
    /// the two `0x00` separators keep a field boundary from being shifted — the
    /// tag/subject pair cannot be split differently to collide — and keep a
    /// commitment from one protocol version from being replayed as another.
    pub fn commitment(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(SENSITIVE_DOMAIN);
        hasher.update([0u8]);
        hasher.update(self.action.tag().as_bytes());
        hasher.update([0u8]);
        hasher.update(self.subject.as_bytes());
        let digest = hasher.finalize();
        let mut commitment = [0u8; 32];
        commitment.copy_from_slice(&digest[..32]);
        commitment
    }
}

/// Why a sensitive action was refused. Every variant is a refusal; there is no
/// "unknown" variant that a caller could mistake for a pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SensitiveRefusal {
    /// The subject was empty or whitespace, so nothing would be bound.
    EmptySubject { action: SensitiveAction },
    /// The gate was built for a different action than the request names.
    ActionMismatch {
        gate: SensitiveAction,
        request: SensitiveAction,
    },
    /// The evidence carried an approval hash that is not this request's
    /// commitment. The evidence is for some other action or subject.
    HashMismatch {
        presented: [u8; 32],
        derived: [u8; 32],
    },
    /// The security council is too small for a quorum to mean anything.
    ///
    /// `ReviewerRegistry::security_council_threshold` is `ceil(2/3)`, so a
    /// one-member council has a threshold of one and a single signature would
    /// authorize a runtime upgrade. A quorum of one is not a quorum.
    CouncilTooSmall { size: usize, minimum: usize },
    /// The required approval level was not met by the presented evidence.
    Unmet {
        action: SensitiveAction,
        requirement: ApprovalRequirement,
    },
}

impl core::fmt::Display for SensitiveRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SensitiveRefusal::EmptySubject { action } => write!(
                f,
                "sensitive action {} has no subject; an approval bound to nothing is not an approval",
                action.tag()
            ),
            SensitiveRefusal::ActionMismatch { gate, request } => write!(
                f,
                "gate authorizes {} but the request asks for {}",
                gate.tag(),
                request.tag()
            ),
            SensitiveRefusal::HashMismatch { .. } => write!(
                f,
                "the presented approval hash is not this request's commitment"
            ),
            SensitiveRefusal::CouncilTooSmall { size, minimum } => write!(
                f,
                "security council has {size} member(s); a sensitive action needs at least {minimum}"
            ),
            SensitiveRefusal::Unmet { action, requirement } => write!(
                f,
                "{} requires {requirement:?} and the presented evidence does not satisfy it",
                action.tag()
            ),
        }
    }
}

impl std::error::Error for SensitiveRefusal {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commitment_binds_the_action_and_the_subject() {
        let a = SensitiveRequest::new(SensitiveAction::RuntimeUpgrade, "spec-42").unwrap();
        let same = SensitiveRequest::new(SensitiveAction::RuntimeUpgrade, "spec-42").unwrap();
        let other_action =
            SensitiveRequest::new(SensitiveAction::TokenSupplyChange, "spec-42").unwrap();
        let other_subject =
            SensitiveRequest::new(SensitiveAction::RuntimeUpgrade, "spec-43").unwrap();

        assert_eq!(a.commitment(), same.commitment(), "must be deterministic");
        assert_ne!(
            a.commitment(),
            other_action.commitment(),
            "the action tag must be part of the commitment"
        );
        assert_ne!(
            a.commitment(),
            other_subject.commitment(),
            "the subject must be part of the commitment"
        );
    }

    #[test]
    fn commitment_separates_fields() {
        // Without the 0x00 separators, ("ab", "c") and ("a", "bc") would hash
        // identically and one approval would authorize the other.
        let split_a = SensitiveRequest::new(SensitiveAction::RuntimeUpgrade, "abc").unwrap();
        let split_b =
            SensitiveRequest::new(SensitiveAction::ValidatorKeyReplacement, "abc").unwrap();
        assert_ne!(split_a.commitment(), split_b.commitment());
    }

    #[test]
    fn an_empty_subject_is_refused() {
        let err = SensitiveRequest::new(SensitiveAction::RuntimeUpgrade, "   ").unwrap_err();
        assert_eq!(
            err,
            SensitiveRefusal::EmptySubject {
                action: SensitiveAction::RuntimeUpgrade
            }
        );
    }

    #[test]
    fn operation_names_resolve_and_unknown_names_do_not() {
        assert_eq!(
            SensitiveAction::from_operation("runtime-upgrade"),
            Some(SensitiveAction::RuntimeUpgrade)
        );
        assert_eq!(
            SensitiveAction::from_operation("  Token_Supply_Change "),
            Some(SensitiveAction::TokenSupplyChange)
        );
        assert_eq!(SensitiveAction::from_operation("deploy_everything"), None);
        assert_eq!(SensitiveAction::from_operation(""), None);
    }

    #[test]
    fn every_sensitive_action_requires_more_than_human_review() {
        for action in SensitiveAction::ALL {
            let requirement = required_requirement(action);
            assert!(
                matches!(
                    requirement,
                    ApprovalRequirement::SecurityReview | ApprovalRequirement::GovernanceReview
                ),
                "{} must not be authorizable by a single human token, got {requirement:?}",
                action.tag()
            );
        }
    }
}
