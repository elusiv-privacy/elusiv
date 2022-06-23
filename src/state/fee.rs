use crate::commitment::{BaseCommitmentHashComputation, commitment_hash_computation_instructions, commitments_per_batch};
use crate::macros::{elusiv_account};
use crate::bytes::{BorshSerDeSized, div_ceiling, u64_as_usize_safe};
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
pub const MAX_BASE_COMMITMENT_NETWORK_FEE: u64 = 5000;

/// Additional fee per proof verification
pub const MAX_PROOF_NETWORK_FEE: u64 = 5000;

/// Additional fee per hash computation tx, rewarded to the relayer
pub const MAX_RELAYER_HASH_TX_FEE: u64 = 10;

/// Additional fee per proof computation tx, rewarded to the relayer
pub const MAX_RELAYER_PROOF_TX_FEE: u64 = 10;

/// Reward for the relayer, paying all fees upfront
pub const MAX_RELAYER_PROOF_REWARD: u64 = 500;

// Values taken from the genesis
const MIN_LAMPORTS_PER_SIGNATURE: u64 = 5_000;
const MAX_LAMPORTS_PER_SIGNATURE: u64 = 100_000;

#[elusiv_account(pda_seed = b"fee")]
/// Specifies the program fees and compensation for relayers
/// - multiple fee-accounts can exist
/// - each one has it's own version as its pda-offset
/// - the `GovernorAccount` defines the most-recent version
pub struct FeeAccount {
    bump_seed : u8,
    version: u8,
    initialized: bool,

    /// consists of `lamports_per_signature` and possible additional compute units costs
    /// hard cap until we find a better solution (also depends on the future changed made to the Solana fee model)
    lamports_per_tx: u64,

    base_commitment_network_fee: u64,
    proof_network_fee: u64,

    relayer_hash_tx_fee: u64,
    relayer_proof_tx_fee: u64,

    relayer_proof_reward: u64,
}

impl<'a> FeeAccount<'a> {
    pub fn setup(
        &mut self,
        lamports_per_tx: u64,
        base_commitment_network_fee: u64,
        proof_network_fee: u64,
        relayer_hash_tx_fee: u64,
        relayer_proof_tx_fee: u64,
        relayer_proof_reward: u64,
    ) -> ProgramResult {
        guard!(lamports_per_tx >= MIN_LAMPORTS_PER_SIGNATURE, InvalidFee);
        guard!(lamports_per_tx <= MAX_LAMPORTS_PER_SIGNATURE, InvalidFee);
        self.set_lamports_per_tx(&lamports_per_tx);

        guard!(base_commitment_network_fee <= MAX_BASE_COMMITMENT_NETWORK_FEE, InvalidFee);
        self.set_base_commitment_network_fee(&base_commitment_network_fee);

        guard!(proof_network_fee <= MAX_PROOF_NETWORK_FEE, InvalidFee);
        self.set_proof_network_fee(&proof_network_fee);

        guard!(relayer_hash_tx_fee <= MAX_RELAYER_HASH_TX_FEE, InvalidFee);
        self.set_relayer_hash_tx_fee(&relayer_hash_tx_fee);

        guard!(relayer_proof_tx_fee <= MAX_RELAYER_PROOF_TX_FEE, InvalidFee);
        self.set_relayer_proof_tx_fee(&relayer_proof_tx_fee);

        guard!(relayer_proof_reward <= MAX_RELAYER_PROOF_REWARD, InvalidFee);
        self.set_relayer_proof_reward(&relayer_proof_reward);

        Ok(())
    }

    /// Compensation for a single hash tx
    pub fn hash_tx_compensation(&self) -> u64 {
        self.get_lamports_per_tx() + self.get_relayer_hash_tx_fee()
    }

    /// tx_count * (lamports_per_tx + relayer_hash_tx_fee) + commitment_hash_fee + base_commitment_network_fee
    pub fn base_commitment_hash_fee(
        &self,
        min_batching_rate: u32,
    ) -> u64 {
        BaseCommitmentHashComputation::INSTRUCTIONS.len() as u64 * self.hash_tx_compensation()
            + self.get_base_commitment_network_fee()
            + self.commitment_hash_fee(min_batching_rate)
    }

    /// tx_count * (lamports_per_tx + relayer_hash_tx_fee)
    pub fn commitment_hash_fee(
        &self,
        min_batching_rate: u32,
    ) -> u64 {
        let tx_count_total = commitment_hash_computation_instructions(min_batching_rate).len();
        let commitments_per_batch = commitments_per_batch(min_batching_rate);
        div_ceiling(
            tx_count_total as u64 * self.hash_tx_compensation(),
            commitments_per_batch as u64
        )
    }

    /// tx_count * (lamports_per_tx + relayer_proof_tx_fee) + relayer_proof_reward + commitment_hash_fee + proof_network_fee
    pub fn proof_verification_fee<VKey: VerificationKey>(
        &self,
        public_inputs: &[BigInteger256],
        min_batching_rate: u32,
    ) -> u64 {
        Self::proof_tx_count::<VKey>(public_inputs) as u64 * (self.get_lamports_per_tx() + self.get_relayer_proof_tx_fee())
            + self.get_relayer_proof_reward()
            + self.commitment_hash_fee(min_batching_rate)
            + self.get_proof_network_fee()
    }

    fn proof_tx_count<VKey: VerificationKey>(public_inputs: &[BigInteger256]) -> u64 {
        let input_preparation_tx_count = prepare_public_inputs_instructions::<VKey>(public_inputs).len();

        (input_preparation_tx_count
            + CombinedMillerLoop::INSTRUCTIONS.len()
            + FinalExponentiation::INSTRUCTIONS.len()) as u64
    }

    pub fn proof_tx_compensation(&self) -> u64 {
        self.get_lamports_per_tx() + self.get_relayer_proof_tx_fee()
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
            MAX_LAMPORTS_PER_SIGNATURE,
            MAX_BASE_COMMITMENT_NETWORK_FEE,
            MAX_PROOF_NETWORK_FEE,
            MAX_RELAYER_HASH_TX_FEE,
            MAX_RELAYER_PROOF_TX_FEE,
            MAX_RELAYER_PROOF_REWARD
        ).unwrap();
    }

    #[test]
    fn test_fee_account_setup() {
        account!(fee_account);

        assert_matches!(fee_account.setup(0, MAX_BASE_COMMITMENT_NETWORK_FEE + 1, 0, 0, 0, 0), Err(_));
        assert_matches!(fee_account.setup(0, 0, MAX_PROOF_NETWORK_FEE + 1, 0, 0, 0), Err(_));         
        assert_matches!(fee_account.setup(0, 0, 0, MAX_RELAYER_HASH_TX_FEE + 1, 0, 0), Err(_));
        assert_matches!(fee_account.setup(0, 0, 0, 0, MAX_RELAYER_PROOF_TX_FEE + 1, 0), Err(_));
        assert_matches!(fee_account.setup(0, 0, 0, 0, 0, MAX_RELAYER_PROOF_REWARD + 1), Err(_));

        max_value_setup(&mut fee_account);

        assert_eq!(fee_account.get_base_commitment_network_fee(), MAX_BASE_COMMITMENT_NETWORK_FEE);
        assert_eq!(fee_account.get_proof_network_fee(), MAX_PROOF_NETWORK_FEE);
        assert_eq!(fee_account.get_relayer_hash_tx_fee(), MAX_RELAYER_HASH_TX_FEE);
        assert_eq!(fee_account.get_relayer_proof_tx_fee(), MAX_RELAYER_PROOF_TX_FEE);
        assert_eq!(fee_account.get_relayer_proof_reward(), MAX_RELAYER_PROOF_REWARD);
    }

    #[test]
    fn test_base_commitment_hash_fee() {
        account!(fee_account);
        max_value_setup(&mut fee_account);

        assert_eq!(
            fee_account.base_commitment_hash_fee(0),
            (MAX_RELAYER_HASH_TX_FEE + MAX_LAMPORTS_PER_SIGNATURE) * BaseCommitmentHashComputation::INSTRUCTIONS.len() as u64
            + fee_account.commitment_hash_fee(0)
            + MAX_BASE_COMMITMENT_NETWORK_FEE
        )
    }

    #[test]
    fn test_commitment_hash_fee() {
        account!(fee_account);
        max_value_setup(&mut fee_account);

        assert_eq!(
            fee_account.commitment_hash_fee(0),
            (MAX_RELAYER_HASH_TX_FEE + MAX_LAMPORTS_PER_SIGNATURE) * commitment_hash_computation_instructions(0).len() as u64
        )
    }

    #[test]
    fn test_proof_verification_fee() {
        account!(fee_account);
        max_value_setup(&mut fee_account);

        type VK = SendBinaryVKey;
        let public_inputs = vec![BigInteger256::new([0,0,0,0]); VK::PUBLIC_INPUTS_COUNT];

        assert_eq!(
            fee_account.proof_verification_fee::<VK>(&public_inputs, 0),
            (MAX_RELAYER_PROOF_TX_FEE + MAX_LAMPORTS_PER_SIGNATURE) * (
                1
                + CombinedMillerLoop::INSTRUCTIONS.len() as u64
                + FinalExponentiation::INSTRUCTIONS.len() as u64
            )
            + MAX_PROOF_NETWORK_FEE
            + MAX_RELAYER_PROOF_REWARD
            + fee_account.commitment_hash_fee(0)
        );
    }
}