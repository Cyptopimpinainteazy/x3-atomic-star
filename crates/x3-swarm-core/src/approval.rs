//! The gate that stands between an agent and a sensitive production action.
//!
//! History, because it explains the shape: this type used to hold a caller's
//! chosen [`ApprovalRequirement`] and expose `grant(&mut self)`, which set that
//! requirement to `None`. Any agent holding its own gate could therefore
//! authorize itself, and `request_approval` never consulted
//! `ApprovalRequirement::is_satisfied` at all — it logged and returned `false`
//! for reviews, so the gate was either self-granted or permanently shut. It had
//! no callers, which is the only reason the hole was latent rather than live.
//!
//! Measured 2026-09-27: `grep -rn "ApprovalGate\|\.grant("` outside this crate
//! finds no call site and no caller of `grant`.
//!
//! The gate now takes the *action*, derives the requirement itself
//! ([`crate::sensitive::required_requirement`]), derives the commitment the
//! evidence must cover ([`SensitiveRequest::commitment`]), and delegates the
//! decision to the existing M-of-N verifier. There is no constructor that
//! accepts a requirement and no method that lifts one.

use crate::policy::{ApprovalContext, ApprovalRequirement, GovernanceChecker, ReviewerRegistry};
use crate::sensitive::{required_requirement, SensitiveAction, SensitiveRefusal, SensitiveRequest};

/// Smallest security council that can meaningfully authorize a sensitive action.
///
/// `ReviewerRegistry::security_council_threshold` is `ceil(2/3)`, so a council
/// of one has a threshold of one. Two-of-three is the smallest council where a
/// majority is also a real quorum; below it the gate refuses outright rather
/// than reporting that the quorum was met.
pub const MIN_SECURITY_COUNCIL_SIZE: usize = 3;

/// The authorization gate for one sensitive production action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApprovalGate {
    action: SensitiveAction,
}

impl ApprovalGate {
    /// Build the gate for an action. The requirement comes from the action, not
    /// from the caller, so it cannot be chosen lower than the bar.
    pub const fn for_action(action: SensitiveAction) -> Self {
        Self { action }
    }

    pub const fn action(&self) -> SensitiveAction {
        self.action
    }

    /// The level this gate demands. Read-only; the gate does not carry a
    /// mutable requirement.
    pub const fn requirement(&self) -> ApprovalRequirement {
        required_requirement(self.action)
    }

    /// Authorize `request` with `evidence`.
    ///
    /// The approval hash is derived here. If the caller also supplies one it
    /// must match — a mismatch is reported rather than silently replaced, so an
    /// attempt to reuse another action's approval is visible instead of merely
    /// failing signature verification.
    ///
    /// Everything that can go wrong is a refusal: the wrong action, an empty
    /// subject, a foreign hash, or evidence that does not meet the required
    /// level (including `registry`/`governance` being absent, which
    /// `is_satisfied` already treats as unsatisfied rather than as a pass).
    pub fn authorize(
        &self,
        request: &SensitiveRequest,
        evidence: &ApprovalContext,
        registry: Option<&dyn ReviewerRegistry>,
        governance: Option<&dyn GovernanceChecker>,
    ) -> Result<(), SensitiveRefusal> {
        if request.action() != self.action {
            return Err(SensitiveRefusal::ActionMismatch {
                gate: self.action,
                request: request.action(),
            });
        }

        let derived = request.commitment();
        if let Some(presented) = evidence.action_hash {
            if presented != derived {
                return Err(SensitiveRefusal::HashMismatch { presented, derived });
            }
        }

        // Bind the evidence to this request's commitment, then ask the real
        // verifier. A caller-supplied hash never reaches it unchanged.
        let mut bound = evidence.clone();
        bound.action_hash = Some(derived);

        let requirement = self.requirement();

        // A council quorum is only a quorum if the council is big enough to have
        // one. Checked before `is_satisfied`, because a one-member council would
        // otherwise report `true` on a single signature.
        if requirement == ApprovalRequirement::SecurityReview {
            let size = registry.map_or(0, |r| r.security_council_keys().len());
            if size < MIN_SECURITY_COUNCIL_SIZE {
                return Err(SensitiveRefusal::CouncilTooSmall {
                    size,
                    minimum: MIN_SECURITY_COUNCIL_SIZE,
                });
            }
        }

        if requirement.is_satisfied(Some(&bound), registry, governance) {
            Ok(())
        } else {
            Err(SensitiveRefusal::Unmet {
                action: self.action,
                requirement,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_requirement_comes_from_the_action() {
        assert_eq!(
            ApprovalGate::for_action(SensitiveAction::RuntimeUpgrade).requirement(),
            ApprovalRequirement::SecurityReview
        );
        assert_eq!(
            ApprovalGate::for_action(SensitiveAction::TokenSupplyChange).requirement(),
            ApprovalRequirement::GovernanceReview
        );
    }

    #[test]
    fn a_gate_for_another_action_refuses() {
        let gate = ApprovalGate::for_action(SensitiveAction::RuntimeUpgrade);
        let request =
            SensitiveRequest::new(SensitiveAction::GenesisModification, "genesis-dev").unwrap();
        let err = gate
            .authorize(&request, &ApprovalContext::default(), None, None)
            .unwrap_err();
        assert_eq!(
            err,
            SensitiveRefusal::ActionMismatch {
                gate: SensitiveAction::RuntimeUpgrade,
                request: SensitiveAction::GenesisModification,
            }
        );
    }

    #[test]
    fn missing_evidence_is_a_refusal_not_a_pass() {
        let gate = ApprovalGate::for_action(SensitiveAction::TokenSupplyChange);
        let request =
            SensitiveRequest::new(SensitiveAction::TokenSupplyChange, "emission-schedule").unwrap();
        assert!(gate
            .authorize(&request, &ApprovalContext::default(), None, None)
            .is_err());
    }
}
