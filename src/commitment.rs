mod poseidon_hash;
mod poseidon_constants;
pub mod state;

pub use poseidon_hash::*;
pub use poseidon_constants::ITERATIONS;
use crate::error::ElusivError;
use crate::macros::elusiv_account;
use crate::state::program_account::PartialComputationAccount;
use crate::state::queue::BaseCommitmentHashRequest;
use crate::types::U256;

/// Account used for computing `commitment = h(base_commitment, amount)`
/// - https://github.com/elusiv-privacy/circuits/blob/16de8d067a9c71aa7d807cfd80a128de6df863dd/circuits/commitment.circom#L7
/// - multiple of these accounts can exist
#[elusiv_account(pda_seed = b"base_commitment")]
pub struct BaseCommitmentHashingAccount {
    // `PartialComputationAccount` trait fields
    is_active: bool,
    round: u64,
    total_rounds: u64,
    fee_payer: U256,

    request: BaseCommitmentHashRequest,
}

impl<'a> PartialComputationAccount for BaseCommitmentHashingAccount<'a> { }
impl<'a> BaseCommitmentHashingAccount<'a> {
    pub fn reset(
        &mut self,
        request: BaseCommitmentHashRequest,
        fee_payer: U256,
    ) -> Result<(), ElusivError> {
        self.set_request(request);
        self.reset_values(0, fee_payer)
    }
}

/// Account used for computing the hashes of a MT
/// - only one of these accounts can exist per MT
#[elusiv_account(pda_seed = b"commitment")]
pub struct CommitmentHashingAccount {
    // `PartialComputationAccount` trait fields
    is_active: bool,
    round: u64,
    total_rounds: u64,
    fee_payer: U256,

    commitment: U256,
}

impl<'a> PartialComputationAccount for CommitmentHashingAccount<'a> { }
impl<'a> CommitmentHashingAccount<'a> {
    pub fn reset(
        &mut self,
        commitment: U256,
        fee_payer: U256,
    ) -> Result<(), ElusivError> {
        self.set_commitment(commitment);
        self.reset_values(0, fee_payer)
    }
}