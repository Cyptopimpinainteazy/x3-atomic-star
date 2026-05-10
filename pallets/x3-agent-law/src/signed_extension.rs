use crate::{
    law_engine::{PolicyContext, PolicyEngine},
    types::PolicyResult,
    ActivePolicies, Blacklist, Error, ExtrinsicCountThisEpoch, LastEpoch, Pallet, TasksThisBlock,
    ViolationType, Config,
};
use codec::{Decode, Encode};
use frame_support::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{DispatchInfoProvider, SignedExtension},
    transaction_validity::{InvalidTransaction, TransactionValidity, ValidTransaction},
};
use sp_std::fmt;

/// SignedExtension for Agent Law enforcement
///
/// **SECURITY-CRITICAL**: This runs in the pre-dispatch phase BEFORE any state mutations.
/// Order in SignedExtra tuple is strict:
/// ```
/// pub type SignedExtra = (
///     frame_system::CheckNonZeroSender<Runtime>,
///     frame_system::CheckSpecVersion<Runtime>,
///     frame_system::CheckTxVersion<Runtime>,
///     frame_system::CheckGenesis<Runtime>,
///     frame_system::CheckEra<Runtime>,
///     frame_system::CheckNonce<Runtime>,
///     frame_system::CheckWeight<Runtime>,
///     pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
///
///     x3_invariants::InvariantCheck,        // 1. CRITICAL INVARIANTS FIRST
///     x3_agent_law::AgentLawCheck,          // 2. POLICY ENFORCEMENT
///     x3_swarm::CapabilityEnvelopeCheck,    // 3. LONG-RANGE VALIDATION
///     x3_kernel::AtomicSettlementCheck,     // 4. CROSS-VM ATOMICITY
///     x3_flash_finality::FlashFinalityExt,  // 5. FLASH FINALITY
/// );
/// ```
///
/// ⚠️ Reordering breaks the security model. Invariants MUST fail before policies are evaluated.
#[derive(Encode, Decode, Clone, Copy, Eq, PartialEq, TypeInfo)]
pub struct AgentLawCheck;

impl fmt::Debug for AgentLawCheck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AgentLawCheck")
    }
}

impl<T: Config + Send + Sync> SignedExtension for AgentLawCheck
where
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
{
    const IDENTIFIER: &'static str = "AgentLawCheck";

    type AccountId = T::AccountId;
    type Call = T::RuntimeCall;
    type AdditionalSigned = ();
    type Pre = ();

    fn additional_signed(&self) -> sp_std::result::Result<Self::AdditionalSigned, TransactionValidityError> {
        Ok(())
    }

    fn validate(
        &self,
        who: &Self::AccountId,
        _call: &Self::Call,
        _info: &DispatchInfoProvider,
        _len: usize,
    ) -> TransactionValidity {
        // 1. Check if agent is blacklisted
        let current_block = frame_system::Pallet::<T>::block_number();
        if let Some(expiry) = Blacklist::<T>::get(who) {
            if current_block < expiry {
                // Agent is currently blacklisted
                return Err(InvalidTransaction::Custom(100).into()); // Agent blacklisted
            }
        }

        // 2. Get active policies for this agent
        let policies = ActivePolicies::<T>::get(who);
        if policies.is_empty() {
            // No policies → unrestricted agent
            return Ok(ValidTransaction::default());
        }

        // 3. Build policy context
        let reputation_score = 100u64; // TODO: Query from x3-invariants registry
        let tasks_this_block = TasksThisBlock::<T>::get((current_block, who.clone()));
        let extrinsics_this_epoch = Self::get_extrinsic_count_this_epoch::<T>(who, current_block);
        let related_agents = vec![]; // TODO: Extract from call args

        let context = PolicyContext {
            reputation_score,
            tasks_this_block,
            extrinsics_this_epoch,
            requested_capability: Self::extract_requested_capability(_call),
            related_agents,
            current_block,
            last_activity_block: current_block,
        };

        // 4. Evaluate policies
        let policy_result = PolicyEngine::evaluate_policies::<T>(who, &policies, &context);

        match policy_result {
            PolicyResult::Pass => {
                // Update extrinsic count for rate limiting
                ExtrinsicCountThisEpoch::<T>::mutate(who, |count| *count = count.saturating_add(1));
                Ok(ValidTransaction::default())
            }
            PolicyResult::Fail(violation_type) => {
                // Log violation for later enforcement
                Pallet::<T>::deposit_event(crate::pallet::Event::PolicyViolation {
                    agent: who.clone(),
                    violation_type,
                    enforcement: crate::types::EnforcementAction::Slash(100),
                });
                Err(InvalidTransaction::Custom(101).into()) // Policy violation
            }
        }
    }

    fn pre_dispatch(
        &self,
        who: &Self::AccountId,
        _call: &Self::Call,
        _info: &DispatchInfoProvider,
        _len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        // Repeat blacklist check in pre_dispatch
        let current_block = frame_system::Pallet::<T>::block_number();
        if let Some(expiry) = Blacklist::<T>::get(who) {
            if current_block < expiry {
                return Err(InvalidTransaction::Custom(100).into());
            }
        }

        // Policies already checked in validate(), return Ok
        Ok(())
    }

    fn post_dispatch(
        &self,
        _result: Result<&mut PostDispatchInfo, &TransactionValidityError>,
        _info: &DispatchInfoProvider,
        _len: usize,
        _extrinsic_index: u32,
    ) -> Result<(), TransactionValidityError> {
        Ok(())
    }
}

impl AgentLawCheck {
    /// Get extrinsic count for current epoch, resetting if epoch changed
    fn get_extrinsic_count_this_epoch<T: Config>(
        agent: &T::AccountId,
        current_block: T::BlockNumber,
    ) -> u32 {
        let last_epoch = LastEpoch::<T>::get(agent);
        let epoch_length = T::RateLimitEpochLength::get();
        let current_epoch = current_block / epoch_length;
        let last_epoch_num = last_epoch / epoch_length;

        if current_epoch > last_epoch_num {
            // Epoch changed, reset counter
            ExtrinsicCountThisEpoch::<T>::insert(agent, 0);
            LastEpoch::<T>::insert(agent, current_block);
            0
        } else {
            // Same epoch, return current count
            ExtrinsicCountThisEpoch::<T>::get(agent)
        }
    }

    fn extract_requested_capability<T: Config>(call: &T::Call) -> Option<Vec<u8>> {
        // TODO: map runtime calls to agent capability labels.
        // This currently returns `None` for generic calls and should be extended
        // as the capability model is integrated with X3 call routing.
        let _ = call;
        None
    }
}

#[cfg(test)]
mod tests {
    // Full tests require mock Config trait implementation
    // See pallet/tests.rs for integration tests
}
