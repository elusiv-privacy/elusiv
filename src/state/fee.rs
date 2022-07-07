use crate::commitment::{BaseCommitmentHashComputation, commitment_hash_computation_instructions, commitments_per_batch, MAX_COMMITMENT_BATCHING_RATE};
use crate::macros::{elusiv_account};
use crate::bytes::{BorshSerDeSized, div_ceiling};
use crate::proof::{CombinedMillerLoop, FinalExponentiation};
use crate::state::program_account::SizedAccount;
use super::program_account::PDAAccountData;
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_computation::PartialComputation;
use elusiv_derive::BorshSerDeSized;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Debug, PartialEq, Clone)]
pub struct ProgramFee {
    /// consists of `lamports_per_signature` and possible additional compute units costs
    /// hard cap until we find a better solution (also depends on the future changed made to the Solana fee model)
    pub lamports_per_tx: u64,

    pub base_commitment_network_fee: u64,
    pub proof_network_fee: u64,

    /// Used only as privacy mining incentive to push rewards for relayers without increasing user costs
    pub base_commitment_subvention: u64,
    pub proof_subvention: u64,

    pub relayer_hash_tx_fee: u64,
    pub relayer_proof_tx_fee: u64,  // this fee is ignored atm by the proof-processor and forced to be zero (only here for possible future use)
    pub relayer_proof_reward: u64,
}

impl ProgramFee {
    /// Verifies that possible subventions are not too high
    pub fn is_valid(&self) -> bool {
        for min_batching_rate in 0..MAX_COMMITMENT_BATCHING_RATE as u32 {
            if self.base_commitment_hash_fee(min_batching_rate) < self.base_commitment_network_fee + self.base_commitment_subvention {
                return false
            }

            // For proof verification we assume the cheapest scenario to be 20 TX
            if 20 * (self.lamports_per_tx + self.relayer_proof_tx_fee) + self.commitment_hash_fee(min_batching_rate) < self.proof_subvention {
                return false
            }

            // Make sure that `relayer_proof_tx_fee` is zero
            if self.relayer_proof_tx_fee != 0 { return false }
        }
        true
    }
}

#[elusiv_account(pda_seed = b"fee")]
/// Specifies the program fees and compensation for relayers
/// - multiple fee-accounts can exist
/// - each one has it's own version as its pda-offset
/// - the `GovernorAccount` defines the most-recent version
pub struct FeeAccount {
    pda_data: PDAAccountData,

    program_fee: ProgramFee,
}

impl ProgramFee {
    /// Compensation for a single hash tx
    pub fn hash_tx_compensation(&self) -> u64 {
        self.lamports_per_tx + self.relayer_hash_tx_fee
    }

    /// tx_count * (lamports_per_tx + relayer_hash_tx_fee) + commitment_hash_fee + base_commitment_network_fee
    pub fn base_commitment_hash_fee(
        &self,
        min_batching_rate: u32,
    ) -> u64 {
        BaseCommitmentHashComputation::INSTRUCTIONS.len() as u64 * self.hash_tx_compensation()
            + self.base_commitment_network_fee
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
    pub fn proof_verification_fee(&self, input_preparation_tx_count: usize, min_batching_rate: u32) -> u64 {
        let tx_count = input_preparation_tx_count + CombinedMillerLoop::INSTRUCTIONS.len() + FinalExponentiation::INSTRUCTIONS.len();
        tx_count as u64 * (self.lamports_per_tx + self.relayer_proof_tx_fee)
            + self.relayer_proof_reward
            + self.commitment_hash_fee(min_batching_rate)
            + self.proof_network_fee
    }

    pub fn proof_tx_compensation(&self) -> u64 {
        self.lamports_per_tx + self.relayer_proof_tx_fee
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proof::{vkey::{SendBinaryVKey, VerificationKey}, prepare_public_inputs_instructions};
    use ark_ff::BigInteger256;

    impl Default for ProgramFee {
        fn default() -> Self {
            ProgramFee {
                lamports_per_tx: 11,
                base_commitment_network_fee: 22,
                proof_network_fee: 33,
                base_commitment_subvention: 44,
                proof_subvention: 555,
                relayer_hash_tx_fee: 666,
                relayer_proof_tx_fee: 777,
                relayer_proof_reward: 888
            }
        }
    }

    #[test]
    fn test_base_commitment_hash_fee() {
        let fee = ProgramFee::default();

        assert_eq!(
            fee.base_commitment_hash_fee(0),
            (666 + 11) * BaseCommitmentHashComputation::INSTRUCTIONS.len() as u64
            + fee.commitment_hash_fee(0)
            + 22
        )
    }

    #[test]
    fn test_commitment_hash_fee() {
        let fee = ProgramFee::default();

        assert_eq!(
            fee.commitment_hash_fee(0),
            (666 + 11) * commitment_hash_computation_instructions(0).len() as u64
        )
    }

    #[test]
    fn test_proof_verification_fee() {
        let fee = ProgramFee::default();

        type VK = SendBinaryVKey;
        let public_inputs = vec![BigInteger256::new([0,0,0,0]); VK::PUBLIC_INPUTS_COUNT];
        let input_preparation_tx_count = prepare_public_inputs_instructions::<VK>(&public_inputs).len();

        assert_eq!(
            fee.proof_verification_fee(input_preparation_tx_count, 0),
            (777 + 11) * (
                1
                + CombinedMillerLoop::INSTRUCTIONS.len() as u64
                + FinalExponentiation::INSTRUCTIONS.len() as u64
            )
            + 33
            + 888
            + fee.commitment_hash_fee(0)
        );
    }

    #[test]
    fn test_fee_is_valid() {
        assert!(
            !ProgramFee {
                lamports_per_tx: 1000,

                base_commitment_network_fee: 0,
                proof_network_fee: 0,

                base_commitment_subvention: 100_000,
                proof_subvention: 0,

                relayer_hash_tx_fee: 0,
                relayer_proof_tx_fee: 0,
                relayer_proof_reward: 0,
            }.is_valid()
        );
    }
}