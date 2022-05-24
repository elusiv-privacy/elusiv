pub mod poseidon_hash;
mod poseidon_constants;

use poseidon_hash::*;
use crate::error::ElusivError;
use crate::macros::elusiv_account;
use crate::state::queue::BaseCommitmentHashRequest;
use crate::types::U256;
use crate::macros::{guard, multi_instance_account};
use crate::bytes::BorshSerDeSized;
use crate::state::program_account::SizedAccount;
use solana_program::program_error::ProgramError;
use crate::fields::fr_to_u256_le;
use ark_bn254::Fr;
use ark_ff::{Zero, BigInteger256};
use borsh::{BorshDeserialize, BorshSerialize};

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

    state: [U256; 3],
}

// We allow multiple instances, since base_commitments can be computed in parallel
multi_instance_account!(BaseCommitmentHashingAccount<'a>, 1);

impl<'a> BaseCommitmentHashingAccount<'a> {
    pub fn reset(
        &mut self,
        request: BaseCommitmentHashRequest,
        fee_payer: U256,
    ) -> Result<(), ProgramError> {
        guard!(!self.get_is_active(), ElusivError::AccountCannotBeReset);

        self.set_is_active(&true);
        self.set_round(&0);
        self.set_total_rounds(&(TOTAL_POSEIDON_ROUNDS as u64));
        self.set_fee_payer(&fee_payer);

        // Reset hashing state
        self.set_state(0, &fr_to_u256_le(&Fr::zero()));
        self.set_state(1, &request.base_commitment);
        self.set_state(2, &fr_to_u256_le(&Fr::from(BigInteger256::new([0, 0, 0, request.amount]))));

        self.set_request(&request);

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

    state: [U256; 3],
}

impl<'a> CommitmentHashingAccount<'a> {
    pub fn reset(
        &mut self,
        commitment: U256,
        fee_payer: U256,
    ) -> Result<(), ProgramError> {
        guard!(!self.get_is_active(), ElusivError::AccountCannotBeReset);

        self.set_is_active(&true);
        self.set_round(&0);
        self.set_total_rounds(&(TOTAL_POSEIDON_ROUNDS as u64 * crate::state::MT_HEIGHT as u64));
        self.set_fee_payer(&fee_payer);

        self.set_commitment(&commitment);

        Ok(())
    }
}