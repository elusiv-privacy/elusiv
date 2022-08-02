use crate::commitment::{BaseCommitmentHashComputation, commitment_hash_computation_instructions, commitments_per_batch, MAX_COMMITMENT_BATCHING_RATE};
use crate::macros::{elusiv_account};
use crate::bytes::{BorshSerDeSized, div_ceiling, u64_as_usize_safe};
use crate::proof::{CombinedMillerLoop, FinalExponentiation};
use crate::state::program_account::SizedAccount;
use super::program_account::PDAAccountData;
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_computation::PartialComputation;
use elusiv_derive::BorshSerDeSized;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Debug, PartialEq, Clone)]
pub struct ProgramFee {
    /// Consists of `lamports_per_signature` and possible additional compute units costs
    /// Hard cap until we find a better solution (also depends on the future changes made to the Solana fee model)
    pub lamports_per_tx: u64,

    pub base_commitment_network_fee: u64,

    /// Per join-split-amount fee in 1/100 percent-points
    pub proof_network_fee: u64, 

    /// Used only as privacy mining incentive to push rewards for relayers without increasing user costs
    pub base_commitment_subvention: u64,
    pub proof_subvention: u64,

    pub relayer_hash_tx_fee: u64,
    pub relayer_proof_reward: u64,

    /// Current tx count for init, combined miller loop, final exponentiation and finalization (dynamic tx for input preparation ignored)
    pub proof_base_tx_count: u64,
}

impl ProgramFee {
    /// Verifies that possible subventions are not too high
    pub fn is_valid(&self) -> bool {
        for min_batching_rate in 0..MAX_COMMITMENT_BATCHING_RATE as u32 {
            if self.base_commitment_hash_fee(min_batching_rate) < self.base_commitment_network_fee + self.base_commitment_subvention {
                return false
            }

            // For proof verification we assume the cheapest scenario to be proof_base_tx_count (and network fee to be zero)
            if self.proof_base_tx_count * self.lamports_per_tx + self.commitment_hash_fee(min_batching_rate) < self.proof_subvention {
                return false
            }

            if u64_as_usize_safe(self.proof_base_tx_count) != CombinedMillerLoop::TX_COUNT + FinalExponentiation::TX_COUNT + 2 { return false }
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
        BaseCommitmentHashComputation::TX_COUNT as u64 * self.hash_tx_compensation()
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

    /// tx_count * lamports_per_tx + relayer_proof_reward + commitment_hash_fee + proof_network_fee
    pub fn proof_verification_fee(
        &self,
        input_preparation_tx_count: usize,
        min_batching_rate: u32,
        amount: u64,
    ) -> u64 {
        self.proof_verification_compensation(input_preparation_tx_count)
            + self.relayer_proof_reward
            + self.commitment_hash_fee(min_batching_rate)
            + self.proof_verification_network_fee(amount)
    }

    fn proof_verification_compensation(
        &self,
        input_preparation_tx_count: usize,
    ) -> u64 {
        (input_preparation_tx_count + u64_as_usize_safe(self.proof_base_tx_count)) as u64 * self.lamports_per_tx
    }

    pub fn proof_verification_network_fee(
        &self,
        amount: u64,
    ) -> u64 {
        self.proof_network_fee * amount / 10_000
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proof::{vkey::{TestVKey, VerificationKey}, prepare_public_inputs_instructions};
    use solana_program::native_token::LAMPORTS_PER_SOL;

    impl Default for ProgramFee {
        fn default() -> Self {
            ProgramFee {
                lamports_per_tx: 11,
                base_commitment_network_fee: 22,
                proof_network_fee: 10 * 100,   // equals ten percent
                base_commitment_subvention: 44,
                proof_subvention: 555,
                relayer_hash_tx_fee: 666,
                relayer_proof_reward: 777,
                proof_base_tx_count: 10,
            }
        }
    }

    #[test]
    fn test_base_commitment_hash_fee() {
        let fee = ProgramFee::default();

        assert_eq!(
            fee.base_commitment_hash_fee(0),
            (666 + 11) * BaseCommitmentHashComputation::TX_COUNT as u64
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

        let public_inputs = vec![[0; 32]; TestVKey::PUBLIC_INPUTS_COUNT];
        let input_preparation_tx_count = prepare_public_inputs_instructions::<TestVKey>(&public_inputs).len();

        assert_eq!(
            fee.proof_verification_fee(input_preparation_tx_count, 0, LAMPORTS_PER_SOL),
            11 * (fee.proof_base_tx_count + input_preparation_tx_count as u64)
            + 777
            + fee.commitment_hash_fee(0)
            + LAMPORTS_PER_SOL / 10
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
                relayer_proof_reward: 0,

                proof_base_tx_count: 10,
            }.is_valid()
        );
    }

    #[test]
    fn test_proof_verification_network_fee() {
        let mut fee = ProgramFee::default();

        let amount = LAMPORTS_PER_SOL;

        // 0.01%
        fee.proof_network_fee = 1;
        assert_eq!(fee.proof_verification_network_fee(amount), LAMPORTS_PER_SOL / 10000);

        // 0.1%
        fee.proof_network_fee = 10;
        assert_eq!(fee.proof_verification_network_fee(amount), LAMPORTS_PER_SOL / 1000);

        // 1%
        fee.proof_network_fee = 100;
        assert_eq!(fee.proof_verification_network_fee(amount), LAMPORTS_PER_SOL / 100);

        // 50%
        fee.proof_network_fee = 50 * 100;
        assert_eq!(fee.proof_verification_network_fee(amount), LAMPORTS_PER_SOL / 2);

        // 100%
        fee.proof_network_fee = 100 * 100;
        assert_eq!(fee.proof_verification_network_fee(amount), LAMPORTS_PER_SOL);
    }
}