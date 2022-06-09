//! Every commitment/proof verification requires computation fees and additional fees

use crate::commitment::{BaseCommitmentHashComputation, CommitmentHashComputation};
use crate::macros::{elusiv_account};
use crate::bytes::BorshSerDeSized;
use crate::proof::{prepare_public_inputs_instructions, CombinedMillerLoop, FinalExponentiation};
use crate::proof::vkey::VerificationKey;
use ark_ff::BigInteger256;
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_computation::PartialComputation;
use solana_program::entrypoint::ProgramResult;
use crate::state::program_account::SizedAccount;
use crate::macros::guard;
use crate::error::ElusivError::InvalidFee;

/// Additional fee per store request
const MAX_BASE_COMMITMENT_FEE: u64 = 5000;

/// Additional fee per proof verification
const MAX_PROOF_FEE: u64 = 5000;

/// Additional fee per hash computation tx, rewarded to the relayer
const MAX_RELAYER_HASH_TX_FEE: u64 = 1;

/// Additional fee per proof computation tx, rewarded to the relayer
const MAX_RELAYER_PROOF_TX_FEE: u64 = 1;

/// Reward for the relayer, paying all fees upfront
const MAX_RELAYER_PROOF_REWARD: u64 = 100;

// Values taken from the genesis
const MIN_LAMPORTS_PER_SIGNATURE: u64 = 5_000;
const MAX_LAMPORTS_PER_SIGNATURE: u64 = 100_000;

#[elusiv_account(pda_seed = b"elusiv_fee")]
/// The current additional program fees
pub struct FeeAccount {
    bump_seed : u8,
    initialized: bool,

    is_setup: bool,

    base_commitment_fee: u64,
    proof_fee: u64,

    relayer_hash_tx_fee: u64,
    relayer_proof_tx_fee: u64,

    relayer_proof_reward: u64,
}

impl<'a> FeeAccount<'a> {
    pub fn setup(
        &mut self,
        base_commitment_fee: u64,
        proof_fee: u64,
        relayer_hash_tx_fee: u64,
        relayer_proof_tx_fee: u64,
        relayer_proof_reward: u64,
    ) -> ProgramResult {
        guard!(!self.get_is_setup(), InvalidFee);

        guard!(base_commitment_fee <= MAX_BASE_COMMITMENT_FEE, InvalidFee);
        self.set_base_commitment_fee(&base_commitment_fee);

        guard!(proof_fee <= MAX_PROOF_FEE, InvalidFee);
        self.set_proof_fee(&proof_fee);

        guard!(relayer_hash_tx_fee <= MAX_RELAYER_HASH_TX_FEE, InvalidFee);
        self.set_relayer_hash_tx_fee(&relayer_hash_tx_fee);

        guard!(relayer_proof_tx_fee <= MAX_RELAYER_PROOF_TX_FEE, InvalidFee);
        self.set_relayer_proof_tx_fee(&relayer_proof_tx_fee);

        guard!(relayer_proof_reward <= MAX_RELAYER_PROOF_REWARD, InvalidFee);
        self.set_relayer_proof_reward(&relayer_proof_reward);

        self.set_is_setup(&true);
        Ok(())
    }

    /// tx_count * (lamports_per_tx + relayer_hash_tx_fee) + commitment_hash_fee + base_commitment_fee
    pub fn base_commitment_hash_fee(&self, lamports_per_tx: u64) -> u64 {
        BaseCommitmentHashComputation::INSTRUCTIONS.len() as u64 * (lamports_per_tx + self.get_relayer_hash_tx_fee())
            + self.commitment_hash_fee(lamports_per_tx)
            + self.get_base_commitment_fee()
    }

    /// tx_count * (lamports_per_tx + relayer_hash_tx_fee)
    pub fn commitment_hash_fee(&self, lamports_per_tx: u64) -> u64 {
        CommitmentHashComputation::INSTRUCTIONS.len() as u64 * (lamports_per_tx + self.get_relayer_hash_tx_fee())
    }

    /// tx_count * (lamports_per_tx + relayer_proof_tx_fee) + relayer_proof_reward + commitment_fee + proof_fee
    pub fn proof_verification_fee<VKey: VerificationKey>(
        &self,
        lamports_per_tx: u64,
        public_inputs: &[BigInteger256]
    ) -> u64 {
        let input_preparation_tx_count = prepare_public_inputs_instructions::<VKey>(public_inputs).len();
        let tx_count = input_preparation_tx_count
            + CombinedMillerLoop::INSTRUCTIONS.len()
            + FinalExponentiation::INSTRUCTIONS.len();

        tx_count as u64 * (lamports_per_tx + self.get_relayer_proof_tx_fee())
            + self.get_relayer_proof_reward()
            + self.commitment_hash_fee(lamports_per_tx)
            + self.get_proof_fee()
    }
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;
    use crate::{state::program_account::ProgramAccount, proof::vkey::SendBinaryVKey};
    use super::*;

    macro_rules! account {
        ($id: ident) => {
            let mut data = vec![0; FeeAccount::SIZE];
            let mut $id = FeeAccount::new(&mut data).unwrap();
        };
    }

    fn max_value_setup(fee_account: &mut FeeAccount) {
        fee_account.setup(
            MAX_BASE_COMMITMENT_FEE,
            MAX_PROOF_FEE,
            MAX_RELAYER_HASH_TX_FEE,
            MAX_RELAYER_PROOF_TX_FEE,
            MAX_RELAYER_PROOF_REWARD
        ).unwrap();
    }

    #[test]
    fn test_fee_account_setup() {
        account!(fee_account);

        assert_matches!(fee_account.setup(MAX_BASE_COMMITMENT_FEE + 1, 0, 0, 0, 0), Err(_));
        assert_matches!(fee_account.setup(0, MAX_PROOF_FEE + 1, 0, 0, 0), Err(_));         
        assert_matches!(fee_account.setup(0, 0, MAX_RELAYER_HASH_TX_FEE + 1, 0, 0), Err(_));
        assert_matches!(fee_account.setup(0, 0, 0, MAX_RELAYER_PROOF_TX_FEE + 1, 0), Err(_));
        assert_matches!(fee_account.setup(0, 0, 0, 0, MAX_RELAYER_PROOF_REWARD + 1), Err(_));

        max_value_setup(&mut fee_account);

        assert_eq!(fee_account.get_base_commitment_fee(), MAX_BASE_COMMITMENT_FEE);
        assert_eq!(fee_account.get_proof_fee(), MAX_PROOF_FEE);
        assert_eq!(fee_account.get_relayer_hash_tx_fee(), MAX_RELAYER_HASH_TX_FEE);
        assert_eq!(fee_account.get_relayer_proof_tx_fee(), MAX_RELAYER_PROOF_TX_FEE);
        assert_eq!(fee_account.get_relayer_proof_reward(), MAX_RELAYER_PROOF_REWARD);
    }

    #[test]
    fn test_base_commitment_hash_fee() {
        account!(fee_account);
        max_value_setup(&mut fee_account);

        assert_eq!(
            fee_account.base_commitment_hash_fee(1),
            (MAX_RELAYER_HASH_TX_FEE + 1) * BaseCommitmentHashComputation::INSTRUCTIONS.len() as u64
            + fee_account.commitment_hash_fee(1)
            + MAX_BASE_COMMITMENT_FEE
        )
    }

    #[test]
    fn test_commitment_hash_fee() {
        account!(fee_account);
        max_value_setup(&mut fee_account);

        assert_eq!(
            fee_account.commitment_hash_fee(1),
            (MAX_RELAYER_HASH_TX_FEE + 1) * CommitmentHashComputation::INSTRUCTIONS.len() as u64
        )
    }

    #[test]
    fn test_proof_verification_fee() {
        account!(fee_account);
        max_value_setup(&mut fee_account);

        type VK = SendBinaryVKey;
        let public_inputs = vec![BigInteger256::new([0,0,0,0]); VK::PUBLIC_INPUTS_COUNT];

        assert_eq!(
            fee_account.proof_verification_fee::<VK>(1, &public_inputs),
            (MAX_RELAYER_PROOF_TX_FEE + 1) * (
                1
                + CombinedMillerLoop::INSTRUCTIONS.len() as u64
                + FinalExponentiation::INSTRUCTIONS.len() as u64
            )
            + MAX_PROOF_FEE
            + MAX_RELAYER_PROOF_REWARD
            + fee_account.commitment_hash_fee(1)
        )
    }
}