pub mod poseidon_hash;
mod poseidon_constants;

use poseidon_hash::*;
use crate::error::ElusivError;
use crate::macros::elusiv_account;
use crate::state::queue::BaseCommitmentHashRequest;
use crate::types::U256;
use crate::bytes::SerDe;
use crate::macros::guard;
use crate::state::program_account::SizedAccount;

pub const MAX_BASE_COMMITMENT_ACCOUNTS_COUNT: u64 = 1;

/// Account used for computing `commitment = h(base_commitment, amount)`
/// - https://github.com/elusiv-privacy/circuits/blob/16de8d067a9c71aa7d807cfd80a128de6df863dd/circuits/commitment.circom#L7
/// - multiple of these accounts can exist
#[elusiv_account(pda_seed = b"base_commitment")]
pub struct BaseCommitmentHashingAccount {
    // `PartialComputationAccount` trait
    is_active: bool,
    round: u64,
    total_rounds: u64,
    fee_payer: U256,

    request: BaseCommitmentHashRequest,
}

impl<'a> BaseCommitmentHashingAccount<'a> {
    pub fn reset(
        &mut self,
        request: BaseCommitmentHashRequest,
        fee_payer: U256,
    ) -> Result<(), ElusivError> {
        guard!(!self.get_is_active(), ElusivError::AccountCannotBeReset);

        self.set_is_active(true);
        self.set_round(0);
        self.set_total_rounds(TOTAL_POSEIDON_ROUNDS as u64);
        self.set_fee_payer(fee_payer);

        self.set_request(request);

        Ok(())
    }
}

/// Account used for computing the hashes of a MT
/// - only one of these accounts can exist per MT
#[elusiv_account(pda_seed = b"commitment")]
pub struct CommitmentHashingAccount {
    // `PartialComputationAccount` trait
    is_active: bool,
    round: u64,
    total_rounds: u64,
    fee_payer: U256,

    commitment: U256,
}

impl<'a> CommitmentHashingAccount<'a> {
    pub fn reset(
        &mut self,
        commitment: U256,
        fee_payer: U256,
    ) -> Result<(), ElusivError> {
        guard!(!self.get_is_active(), ElusivError::AccountCannotBeReset);

        self.set_is_active(true);
        self.set_round(0);
        self.set_total_rounds(TOTAL_POSEIDON_ROUNDS as u64 * crate::state::MT_HEIGHT as u64);
        self.set_fee_payer(fee_payer);

        self.set_commitment(commitment);

        Ok(())
    }
}