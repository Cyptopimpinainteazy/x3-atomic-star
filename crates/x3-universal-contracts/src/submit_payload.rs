//! Helpers for building pallet-x3-intent submit arguments from decoded X3IR plans.

use pallet_x3_intent::types::{DecodedCompensationPolicy, DecodedInstruction, DecodedPlanPayload};
use parity_scale_codec::Encode;
use x3_ir::{CompensationPolicy as IrCompensationPolicy, X3ExecutionPlan, X3Instruction};

use crate::error::UcError;

/// Canonical submit arguments expected by pallet-x3-intent::submit_intent.
#[derive(Debug)]
pub struct IntentSubmitArgs {
    pub plan_hash: [u8; 32],
    pub decoded_plan: DecodedPlanPayload,
}

/// Build submit arguments from a decoded X3IR execution plan.
///
/// The resulting `plan_hash` is computed from SCALE-encoded `decoded_plan` so
/// the pallet can enforce payload/hash binding at submit time.
pub fn build_intent_submit_args(plan: &X3ExecutionPlan) -> Result<IntentSubmitArgs, UcError> {
    const MAX_DECODED_INSTRUCTIONS: usize = 256;

    let mut found_compensate = false;
    let mut decoded_instructions = Vec::with_capacity(plan.instructions.len());

    for instruction in &plan.instructions {
        match instruction {
            X3Instruction::Compensate { action } => {
                found_compensate = true;
                let policy = match action.policy {
                    IrCompensationPolicy::TrueRollback => DecodedCompensationPolicy::TrueRollback,
                    IrCompensationPolicy::Refund => DecodedCompensationPolicy::Refund,
                    IrCompensationPolicy::EscrowRelease => DecodedCompensationPolicy::EscrowRelease,
                    IrCompensationPolicy::InsuranceSlash => {
                        DecodedCompensationPolicy::InsuranceSlash
                    }
                };
                decoded_instructions.push(DecodedInstruction::Compensate { policy });
            }
            _ => decoded_instructions.push(DecodedInstruction::Other),
        }
    }

    if !found_compensate {
        return Err(UcError::MissingCompensateInstruction);
    }

    let decoded_plan = DecodedPlanPayload {
        instructions: decoded_instructions
            .try_into()
            .map_err(|_| UcError::DecodedPlanTooLarge(MAX_DECODED_INSTRUCTIONS))?,
    };

    let plan_hash = sp_core::blake2_256(&decoded_plan.encode());

    Ok(IntentSubmitArgs {
        plan_hash,
        decoded_plan,
    })
}
