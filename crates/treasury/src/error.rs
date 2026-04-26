use sp_runtime::RuntimeDebug;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode, TypeInfo)]
pub enum TreasuryError {
    InsufficientFunds,
    InvalidProposal,
    Unauthorized,
}
