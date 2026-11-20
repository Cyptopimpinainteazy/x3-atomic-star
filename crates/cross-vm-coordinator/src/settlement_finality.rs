//! Settlement finality policy for cross-domain release.
//!
//! ## The MEV surface this closes
//!
//! `CoordinatorConfig::confirmations` has always described how deep an external
//! leg must be before it is safe to act on it, and nothing has ever read it.
//! `CrossDomainProofBundle::verify` checks that a bundle's finality evidence is
//! bound to the bundle (same chain, vm, txid, block hash) and that the bundle
//! says `finalized: true` — but `finalized` is the *caller's own* field, and no
//! confirmation depth was compared against the operator's policy anywhere. So a
//! relayer could hand the coordinator a claim proof for an external leg that is
//! one block deep, on a chain the operator believes needs twelve, and the
//! coordinator would build the release envelope for it.
//!
//! Three consequences, in the order an attacker would use them:
//!
//! 1. **Revert the leg after the local side is released.** The coordinator sees
//!    inclusion; the external chain reorgs it away; the intent settles on one
//!    domain only.
//! 2. **Race the reveal.** A claim envelope reveals the preimage. Releasing it
//!    against a leg whose own finality evidence says
//!    `safe_to_reveal_secret: false` publishes the secret to anyone watching the
//!    other domain, who can then claim it before this coordinator does.
//! 3. **Choose your own depth.** A domain the operator never sized inherits the
//!    depth the submitter's proof asserts, which is to say no policy at all.
//!
//! The fix is small on purpose: every release envelope now has to be built with
//! a policy, the policy must cover every chain the release touches, and the
//! observed depth has to reach it. A domain that is not in the policy is refused
//! rather than defaulted.
//!
//! This is a *release* gate, not a replacement for the atomicity guarantees:
//! it does not detect a relayer who simply withholds a settlement, and it does
//! not order competing settlements. See `docs/CROSS_DOMAIN_MEV_THREAT_MODEL.md`
//! for the surfaces that remain open.

use crate::config::ConfirmationConfig;
use crate::CoordinatorError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Execution-domain family a confirmation requirement applies to.
///
/// Deliberately not `x3_atomic_swap::VmType`: the policy has to be usable and
/// testable without the `canonical-proofs` feature, and a domain family is the
/// granularity operators actually configure.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum FinalityDomain {
    /// Ethereum-family chains.
    Evm,
    /// Solana-family chains.
    Svm,
    /// Substrate/Polkadot-family chains.
    Substrate,
    /// Bitcoin-family (script) chains.
    BitcoinScript,
    /// X3's own execution domain.
    X3Vm,
    /// Any domain this policy does not know about.
    Other,
}

impl FinalityDomain {
    /// Short label used in refusals, so an operator can see which leg failed.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Evm => "evm",
            Self::Svm => "svm",
            Self::Substrate => "substrate",
            Self::BitcoinScript => "bitcoin-script",
            Self::X3Vm => "x3vm",
            Self::Other => "unclassified",
        }
    }
}

/// Confirmations a Bitcoin-family leg needs before it is treated as reorg-safe.
///
/// This is the long-standing Bitcoin Core convention (a reorg deeper than six
/// blocks is treated as an attack, not as normal operation) and is named here
/// instead of appearing as a bare literal. `ConfirmationConfig` has no Bitcoin
/// entry, so without this the policy would silently leave Bitcoin uncovered.
pub const BITCOIN_REORG_SAFE_CONFIRMATIONS: u64 = 6;

/// Per-domain release requirements for a cross-domain settlement.
///
/// Constructed from `CoordinatorConfig::confirmations` so the depth an operator
/// configured is the depth that is enforced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementFinalityPolicy {
    required: BTreeMap<FinalityDomain, u64>,
    /// Require `safe_to_reveal_secret` on claim legs (the secret-leak guard).
    require_secret_reveal_safety: bool,
}

impl Default for SettlementFinalityPolicy {
    fn default() -> Self {
        Self::from_confirmations(&ConfirmationConfig::default())
    }
}

impl SettlementFinalityPolicy {
    /// An empty policy: every domain it is asked about is refused.
    pub fn none() -> Self {
        Self {
            required: BTreeMap::new(),
            require_secret_reveal_safety: true,
        }
    }

    /// The depths `ConfirmationConfig` records, plus Bitcoin's documented depth.
    pub fn from_confirmations(confirmations: &ConfirmationConfig) -> Self {
        let mut policy = Self::none();
        policy
            .required
            .insert(FinalityDomain::Evm, u64::from(confirmations.evm));
        policy
            .required
            .insert(FinalityDomain::Svm, u64::from(confirmations.svm));
        policy
            .required
            .insert(FinalityDomain::X3Vm, u64::from(confirmations.x3));
        policy.required.insert(
            FinalityDomain::BitcoinScript,
            BITCOIN_REORG_SAFE_CONFIRMATIONS,
        );
        policy
    }

    /// Cover one more domain explicitly. Domains not covered are refused.
    pub fn with_domain(mut self, domain: FinalityDomain, confirmations: u64) -> Self {
        self.required.insert(domain, confirmations);
        self
    }

    /// Stop requiring `safe_to_reveal_secret` on claim legs.
    ///
    /// Only for domains whose finality evidence cannot express it (none of the
    /// adapters in this workspace). Named rather than a field so the relaxation
    /// is visible at the call site.
    pub fn without_secret_reveal_safety(mut self) -> Self {
        self.require_secret_reveal_safety = false;
        self
    }

    /// Required depth for a domain, or `None` when the policy does not cover it.
    pub fn required_depth(&self, domain: FinalityDomain) -> Option<u64> {
        self.required.get(&domain).copied()
    }

    /// Whether every one of `domains` has a configured depth.
    ///
    /// Exposed so an operator can check a config at startup instead of
    /// discovering the gap on the release path.
    pub fn covers_all(&self, domains: &[FinalityDomain]) -> bool {
        domains.iter().all(|d| self.required.contains_key(d))
    }

    pub const fn requires_secret_reveal_safety(&self) -> bool {
        self.require_secret_reveal_safety
    }
}

/// Map an execution-domain type onto the family an operator configures.
#[cfg(feature = "canonical-proofs")]
pub const fn domain_of(vm: x3_atomic_swap::VmType) -> FinalityDomain {
    use x3_atomic_swap::VmType;
    match vm {
        VmType::Evm => FinalityDomain::Evm,
        VmType::Svm => FinalityDomain::Svm,
        VmType::Substrate | VmType::PolkadotPvm => FinalityDomain::Substrate,
        VmType::BitcoinScript => FinalityDomain::BitcoinScript,
        VmType::X3Vm => FinalityDomain::X3Vm,
        // Every other VM family is unclassified on purpose: an operator has to
        // add it explicitly rather than inherit a depth nobody chose.
        _ => FinalityDomain::Other,
    }
}

/// Enforce the policy across every domain a release touches.
///
/// `proof_set` must already have passed the purpose-specific verification
/// (`verify_claim_set` / `verify_refund_set`); this adds the depth and
/// secret-reveal checks those do not make.
#[cfg(feature = "canonical-proofs")]
pub fn enforce_settlement_finality(
    policy: &SettlementFinalityPolicy,
    purpose: crate::SettlementProofPurpose,
    proof_set: &x3_atomic_swap::CrossDomainProofSet,
    required_domains: &[(x3_atomic_swap::ChainId, x3_atomic_swap::VmType)],
) -> Result<(), CoordinatorError> {
    use crate::SettlementProofPurpose;
    use x3_atomic_swap::CrossDomainOperation;

    let operation = match purpose {
        SettlementProofPurpose::Claim => CrossDomainOperation::Claim,
        SettlementProofPurpose::Refund => CrossDomainOperation::Refund,
    };

    for (chain_id, vm_type) in required_domains {
        let domain = domain_of(*vm_type);
        let required = policy.required_depth(domain).ok_or_else(|| {
            CoordinatorError::FinalityPolicyMissingForDomain {
                chain_id: chain_id.clone(),
                domain: domain.label().to_string(),
            }
        })?;

        let bundle = proof_set
            .require_operation(chain_id, *vm_type, operation)
            .map_err(|error| {
                CoordinatorError::MissingFinalityEvidence {
                    chain_id: chain_id.clone(),
                    domain: domain.label().to_string(),
                    reason: error.to_string(),
                }
            })?;

        let observed = bundle.finality.confirmations;
        if observed < required {
            return Err(CoordinatorError::LegBelowFinalityDepth {
                chain_id: chain_id.clone(),
                domain: domain.label().to_string(),
                observed,
                required,
            });
        }

        if matches!(purpose, SettlementProofPurpose::Claim)
            && policy.requires_secret_reveal_safety()
            && !bundle.finality.safe_to_reveal_secret
        {
            return Err(CoordinatorError::SecretRevealNotSafe {
                chain_id: chain_id.clone(),
                domain: domain.label().to_string(),
            });
        }
    }

    Ok(())
}

#[cfg(all(test, feature = "canonical-proofs"))]
mod tests {
    use super::*;
    use crate::config::ConfirmationConfig;
    use crate::SettlementSubmissionEnvelope;
    use x3_atomic_swap::{
        adapter::FinalityProof,
        intent::{AtomicIntentBuilder, ChainKind, RefundPath},
        AtomicIntent, CrossDomainOperation, CrossDomainProofBundle, CrossDomainProofSet, VmType,
    };

    fn intent() -> AtomicIntent {
        AtomicIntentBuilder::new()
            .source_chain(ChainKind::X3)
            .destination_chain(ChainKind::Ethereum)
            .source_asset("X3")
            .destination_asset("ETH")
            .amount_in(1_000)
            .min_amount_out(900)
            .receiver("receiver")
            .hashlock([9u8; 32])
            .source_timeout(2_000)
            .destination_timeout(1_000)
            .refund_path(RefundPath {
                chain: ChainKind::X3,
                address: "refund".into(),
                asset: None,
            })
            .build(99)
            .expect("intent builds")
    }

    /// A verified bundle for `chain`/`vm`, with the finality evidence an adapter
    /// would produce: `confirmations` deep and `safe` as the caller declares.
    fn bundle(
        intent: &AtomicIntent,
        runtime: [u8; 32],
        chain: &str,
        vm: VmType,
        operation: CrossDomainOperation,
        confirmations: u64,
        safe: bool,
    ) -> CrossDomainProofBundle {
        let tx = format!("{chain}-{operation:?}");
        let block_hash = format!("{chain}-block");
        CrossDomainProofBundle::new(
            intent,
            runtime,
            chain.into(),
            vm,
            operation,
            tx.clone(),
            10,
            block_hash.clone(),
            vec![1, 2, 3],
            FinalityProof {
                chain_id: chain.into(),
                vm_type: vm,
                tx_id: tx,
                block_number: 10,
                block_hash,
                confirmations,
                finalized: true,
                finality_source: "test-adapter".into(),
                safe_to_reveal_secret: safe,
            },
        )
        .expect("bundle builds")
    }

    fn claim_set(
        intent: &AtomicIntent,
        runtime: [u8; 32],
        chain: &str,
        vm: VmType,
        confirmations: u64,
        safe: bool,
    ) -> CrossDomainProofSet {
        let mut set = CrossDomainProofSet::new(intent, runtime);
        set.push_verified(
            intent,
            bundle(
                intent,
                runtime,
                chain,
                vm,
                CrossDomainOperation::Claim,
                confirmations,
                safe,
            ),
        )
        .expect("bundle verifies");
        set
    }

    fn refund_set(
        intent: &AtomicIntent,
        runtime: [u8; 32],
        chain: &str,
        vm: VmType,
        confirmations: u64,
    ) -> CrossDomainProofSet {
        let mut set = CrossDomainProofSet::new(intent, runtime);
        set.push_verified(
            intent,
            bundle(
                intent,
                runtime,
                chain,
                vm,
                CrossDomainOperation::Refund,
                confirmations,
                true,
            ),
        )
        .expect("bundle verifies");
        set
    }

    fn policy() -> SettlementFinalityPolicy {
        SettlementFinalityPolicy::from_confirmations(&ConfirmationConfig {
            evm: 12,
            svm: 50,
            x3: 1,
        })
    }

    /// The exact threshold is accepted, so the check below cannot pass merely
    /// because the call refuses everything.
    #[test]
    fn a_claim_at_the_configured_depth_is_accepted() {
        let intent = intent();
        let runtime = [0x11u8; 32];
        let set = claim_set(&intent, runtime, "eth-mainnet", VmType::Evm, 12, true);

        SettlementSubmissionEnvelope::for_claim(
            &intent,
            runtime,
            set,
            &[("eth-mainnet".into(), VmType::Evm)],
            &policy(),
        )
        .expect("a leg at the configured depth is releasable");
    }

    #[test]
    fn a_claim_one_confirmation_shallow_is_refused() {
        let intent = intent();
        let runtime = [0x12u8; 32];
        let set = claim_set(&intent, runtime, "eth-mainnet", VmType::Evm, 11, true);

        let error = SettlementSubmissionEnvelope::for_claim(
            &intent,
            runtime,
            set,
            &[("eth-mainnet".into(), VmType::Evm)],
            &policy(),
        )
        .expect_err("a leg below the configured depth must not be released");

        match error {
            CoordinatorError::LegBelowFinalityDepth {
                chain_id,
                observed,
                required,
                ..
            } => {
                assert_eq!(chain_id, "eth-mainnet");
                assert_eq!(observed, 11);
                assert_eq!(required, 12);
            }
            other => panic!("expected LegBelowFinalityDepth, got {other:?}"),
        }
    }

    #[test]
    fn a_claim_that_is_not_safe_to_reveal_the_secret_is_refused() {
        let intent = intent();
        let runtime = [0x13u8; 32];
        let set = claim_set(&intent, runtime, "eth-mainnet", VmType::Evm, 12, false);

        let error = SettlementSubmissionEnvelope::for_claim(
            &intent,
            runtime,
            set,
            &[("eth-mainnet".into(), VmType::Evm)],
            &policy(),
        )
        .expect_err("revealing while the leg says it is unsafe must be refused");

        assert!(
            matches!(error, CoordinatorError::SecretRevealNotSafe { .. }),
            "expected SecretRevealNotSafe, got {error:?}"
        );
    }

    #[test]
    fn a_refund_below_the_configured_depth_is_refused() {
        let intent = intent();
        let runtime = [0x14u8; 32];
        let set = refund_set(&intent, runtime, "eth-mainnet", VmType::Evm, 11);

        assert!(
            matches!(
                SettlementSubmissionEnvelope::for_refund(
                    &intent,
                    runtime,
                    set,
                    &[("eth-mainnet".into(), VmType::Evm)],
                    &policy(),
                ),
                Err(CoordinatorError::LegBelowFinalityDepth { .. })
            ),
            "the refund path must be gated the same way the claim path is"
        );
    }

    #[test]
    fn a_domain_the_policy_never_sized_is_refused_instead_of_defaulted() {
        let intent = intent();
        let runtime = [0x15u8; 32];
        let only_evm = SettlementFinalityPolicy {
            required: [(FinalityDomain::Evm, 12u64)].into_iter().collect(),
            require_secret_reveal_safety: true,
        };

        let set = claim_set(&intent, runtime, "x3", VmType::X3Vm, 100, true);
        let error = SettlementSubmissionEnvelope::for_claim(
            &intent,
            runtime,
            claim_set(&intent, runtime, "x3", VmType::X3Vm, 100, true),
            &[("x3".into(), VmType::X3Vm)],
            &only_evm,
        )
        .expect_err("an uncovered domain must be refused");
        assert!(
            matches!(
                error,
                CoordinatorError::FinalityPolicyMissingForDomain { .. }
            ),
            "expected FinalityPolicyMissingForDomain, got {error:?}"
        );

        // …and the same release succeeds once the operator sizes the domain, so
        // the refusal is about coverage and not about the leg.
        SettlementSubmissionEnvelope::for_claim(
            &intent,
            runtime,
            set,
            &[("x3".into(), VmType::X3Vm)],
            &only_evm.clone().with_domain(FinalityDomain::X3Vm, 1),
        )
        .expect("a sized domain at depth is releasable");
    }

    #[test]
    fn the_policy_uses_the_depths_the_operator_configured() {
        let configured = ConfirmationConfig {
            evm: 7,
            svm: 9,
            x3: 2,
        };
        let policy = SettlementFinalityPolicy::from_confirmations(&configured);

        assert_eq!(policy.required_depth(FinalityDomain::Evm), Some(7));
        assert_eq!(policy.required_depth(FinalityDomain::Svm), Some(9));
        assert_eq!(policy.required_depth(FinalityDomain::X3Vm), Some(2));
        assert_eq!(
            policy.required_depth(FinalityDomain::BitcoinScript),
            Some(BITCOIN_REORG_SAFE_CONFIRMATIONS)
        );
        assert_eq!(policy.required_depth(FinalityDomain::Substrate), None);
        assert!(!policy.covers_all(&[
            FinalityDomain::Evm,
            FinalityDomain::Substrate
        ]));
    }

    #[test]
    fn the_secret_reveal_guard_can_only_be_relaxed_explicitly() {
        let default_policy = policy();
        assert!(default_policy.requires_secret_reveal_safety());
        assert!(!default_policy.without_secret_reveal_safety().requires_secret_reveal_safety());
        assert!(
            SettlementFinalityPolicy::none().required_depth(FinalityDomain::Evm).is_none(),
            "an empty policy must not hand out depths"
        );
    }
}
