pub mod poseidon_hash;
mod poseidon_constants;

use crate::error::ElusivError;
use crate::macros::{elusiv_account, elusiv_hash_compute_units, guard, multi_instance_account};
use crate::state::queue::BaseCommitmentHashRequest;
use crate::types::U256;
use crate::bytes::BorshSerDeSized;
use crate::state::{program_account::SizedAccount, MT_HEIGHT};
use crate::fields::fr_to_u256_le;
use solana_program::program_error::ProgramError;
use ark_bn254::Fr;
use ark_ff::{Zero, BigInteger256};
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_computation::{PartialComputation};

// Base commitment hashing instructions
elusiv_hash_compute_units!(BaseCommitmentHashComputation, 1);
const_assert_eq!(BaseCommitmentHashComputation::INSTRUCTIONS.len(), 2);

/// Account used for computing `commitment = h(base_commitment, amount)`
/// - https://github.com/elusiv-privacy/circuits/blob/16de8d067a9c71aa7d807cfd80a128de6df863dd/circuits/commitment.circom#L7
/// - multiple of these accounts can exist
#[elusiv_account(pda_seed = b"base_commitment", partial_computation)]
pub struct BaseCommitmentHashingAccount {
    bump_seed: u8,
    initialized: bool,

    is_active: bool,
    instruction: u32,
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
        self.set_instruction(&0);
        self.set_fee_payer(&fee_payer);

        // Reset hashing state
        self.set_state(0, &fr_to_u256_le(&Fr::zero()));
        self.set_state(1, &request.base_commitment);
        self.set_state(2, &fr_to_u256_le(&Fr::from(BigInteger256::from(request.amount))));

        self.set_request(&request);

        Ok(())
    }
}

// Commitment hashing instructions
elusiv_hash_compute_units!(CommitmentHashComputation, 20);
const_assert_eq!(MT_HEIGHT, 20);
const_assert_eq!(CommitmentHashComputation::INSTRUCTIONS.len(), 26);

/// Account used for computing the hashes of a MT
/// - only one of these accounts can exist per MT
#[elusiv_account(pda_seed = b"commitment", partial_computation)]
pub struct CommitmentHashingAccount {
    bump_seed: u8,
    initialized: bool,

    is_active: bool,
    instruction: u32,
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
        self.set_instruction(&0);
        self.set_fee_payer(&fee_payer);

        self.set_commitment(&commitment);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_commitment_hashing_account_setup() {
        let mut data = vec![0; BaseCommitmentHashingAccount::SIZE];
        BaseCommitmentHashingAccount::new(&mut data).unwrap();
    }

    #[test]
    fn test_commitment_hashing_account_setup() {
        let mut data = vec![0; CommitmentHashingAccount::SIZE];
        CommitmentHashingAccount::new(&mut data).unwrap();
    }
}