pub mod poseidon_hash;
mod poseidon_constants;

use crate::error::ElusivError;
use crate::macros::{elusiv_account, elusiv_hash_compute_units, guard, multi_instance_account};
use crate::state::queue::BaseCommitmentHashRequest;
use crate::types::U256;
use crate::bytes::BorshSerDeSized;
use crate::state::{program_account::SizedAccount, MT_HEIGHT};
use crate::fields::{fr_to_u256_le, u64_to_scalar};
use solana_program::program_error::ProgramError;
use ark_bn254::Fr;
use ark_ff::Zero;
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
        self.set_state(2, &fr_to_u256_le(&u64_to_scalar(request.amount)));

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
    ordering: u32,
    siblings: [U256; MT_HEIGHT as usize],
    finished_hashes: [U256; MT_HEIGHT as usize],
}

impl<'a> CommitmentHashingAccount<'a> {
    pub fn reset(
        &mut self,
        commitment: U256,
        ordering: u32,
        siblings: [Fr; MT_HEIGHT as usize],
        fee_payer: U256,
    ) -> Result<(), ProgramError> {
        guard!(!self.get_is_active(), ElusivError::AccountCannotBeReset);

        self.set_is_active(&true);
        self.set_instruction(&0);
        self.set_fee_payer(&fee_payer);

        // Reset hashing state
        self.set_state(0, &fr_to_u256_le(&Fr::zero()));
        let offset = ordering as usize % 2;
        self.set_state(1 + offset, &commitment);
        self.set_state(2 - offset, &fr_to_u256_le(&siblings[0]));

        // Assign new values
        self.set_commitment(&commitment);
        self.set_ordering(&ordering);
        for i in 0..MT_HEIGHT as usize {
            self.set_siblings(i, &fr_to_u256_le(&siblings[i]));
        }

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

    #[test]
    fn test_base_commitment_account_reset() {
        let mut data = vec![0; BaseCommitmentHashingAccount::SIZE];
        let mut account = BaseCommitmentHashingAccount::new(&mut data).unwrap();

        let mut base_commitment = [0; 32];
        for i in 0..32 { base_commitment[i] = i as u8; }

        let amount = 123456789;

        let mut commitment = [0; 32];
        for i in 0..32 { commitment[i] = i as u8 * 2; }

        let fee_payer = [9; 32];

        account.reset(BaseCommitmentHashRequest { base_commitment, amount, commitment }, fee_payer).unwrap();

        assert_eq!(account.get_state(0), [0; 32]);
        assert_eq!(account.get_state(1), base_commitment);
        assert_eq!(account.get_state(2), fr_to_u256_le(&u64_to_scalar(amount)));

        assert_eq!(fee_payer, account.get_fee_payer());
    }
    
    #[test]
    fn test_commitment_account_reset() {
        let mut data = vec![0; CommitmentHashingAccount::SIZE];
        let mut account = CommitmentHashingAccount::new(&mut data).unwrap();

        let mut commitment = [0; 32];
        for i in 0..32 { commitment[i] = i as u8; }

        let ordering = 123456789;

        let mut siblings = [Fr::zero(); MT_HEIGHT as usize];
        for i in 0..siblings.len() {
            siblings[i] = u64_to_scalar(i as u64);
        }

        let fee_payer = [9; 32];

        account.reset(commitment, ordering, siblings, fee_payer).unwrap();

        assert_eq!(account.get_state(0), [0; 32]);
        assert_eq!(account.get_state(1), fr_to_u256_le(&siblings[0]));
        assert_eq!(account.get_state(2), commitment);

        assert_eq!(commitment, account.get_commitment());
        assert_eq!(ordering, account.get_ordering());
        assert_eq!(fee_payer, account.get_fee_payer());
        for i in 0..MT_HEIGHT as usize {
            assert_eq!(fr_to_u256_le(&siblings[i]), account.get_siblings(i));
        }
    }
}