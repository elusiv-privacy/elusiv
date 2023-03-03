use super::program_account::PDAAccountData;
use crate::bytes::{div_ceiling_u64, u64_as_usize_safe};
use crate::commitment::{
    commitment_hash_computation_instructions, commitments_per_batch, BaseCommitmentHashComputation,
    MAX_COMMITMENT_BATCHING_RATE,
};
use crate::macros::elusiv_account;
use crate::proof::verifier::{CombinedMillerLoop, FinalExponentiation};
use crate::token::{Lamports, Token, TokenError, TokenPrice};
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_computation::PartialComputation;
use elusiv_derive::BorshSerDeSized;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug))]
pub struct BasisPointFee(pub u64);

impl BasisPointFee {
    pub fn calc(&self, amount: u64) -> u64 {
        self.0 * amount / 10_000
    }
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug))]
pub struct ProgramFee {
    /// Consists of `lamports_per_signature` and possible additional compute units costs
    /// TODO: will be changed with our upcoming fee consensus fee-model
    pub lamports_per_tx: Lamports,

    /// Per storage-amount fee in basis points
    pub base_commitment_network_fee: BasisPointFee,

    /// Per join-split-amount fee in basis points
    pub proof_network_fee: BasisPointFee,

    /// Used only as privacy mining incentive to push rewards for wardens without increasing user costs
    pub base_commitment_subvention: Lamports,
    pub proof_subvention: Lamports,

    pub warden_hash_tx_reward: Lamports,
    pub warden_proof_reward: Lamports,

    /// Current tx count for init, combined miller loop, final exponentiation and finalization (dynamic tx for input preparation ignored)
    pub proof_base_tx_count: u64,
}

impl ProgramFee {
    /// Creates a new `ProgramFee` if the inputs are valid
    pub fn new(
        lamports_per_tx: u64,
        base_commitment_network_fee: u64,
        proof_network_fee: u64,
        base_commitment_subvention: u64,
        proof_subvention: u64,
        warden_hash_tx_reward: u64,
        warden_proof_reward: u64,
    ) -> Option<Self> {
        let s = Self {
            lamports_per_tx: Lamports(lamports_per_tx),
            base_commitment_network_fee: BasisPointFee(base_commitment_network_fee),
            proof_network_fee: BasisPointFee(proof_network_fee),
            base_commitment_subvention: Lamports(base_commitment_subvention),
            proof_subvention: Lamports(proof_subvention),
            warden_hash_tx_reward: Lamports(warden_hash_tx_reward),
            warden_proof_reward: Lamports(warden_proof_reward),
            proof_base_tx_count: Self::proof_base_tx_count(),
        };

        if s.is_valid() {
            Some(s)
        } else {
            None
        }
    }

    /// Verifies that possible subventions are not too high
    pub fn is_valid(&self) -> bool {
        for min_batching_rate in 0..MAX_COMMITMENT_BATCHING_RATE as u32 {
            let commitment_fee = self.commitment_hash_computation_fee(min_batching_rate).0;
            if self.base_commitment_subvention.0 > commitment_fee {
                return false;
            }

            // For proof verification we assume the cheapest scenario to be proof_base_tx_count (and network fee to be zero)
            let proof_fee = self.proof_base_tx_count * self.lamports_per_tx.0
                + self.commitment_hash_computation_fee(min_batching_rate).0;
            if self.proof_subvention.0 > proof_fee {
                return false;
            }

            if self.proof_base_tx_count != Self::proof_base_tx_count() {
                return false;
            }
        }
        true
    }

    pub fn proof_base_tx_count() -> u64 {
        (CombinedMillerLoop::TX_COUNT + FinalExponentiation::TX_COUNT + 2) as u64
    }
}

/// Specifies the program fees and compensation for wardens
#[elusiv_account]
pub struct FeeAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,

    pub program_fee: ProgramFee,
}

impl ProgramFee {
    pub fn hash_tx_compensation(&self) -> Lamports {
        Lamports(self.lamports_per_tx.0 + self.warden_hash_tx_reward.0)
    }

    pub fn base_commitment_hash_computation_fee(&self) -> Lamports {
        // extra `lamports_per_tx` for the second signature, paid for by the fee-payer

        Lamports(
            BaseCommitmentHashComputation::TX_COUNT as u64 * self.hash_tx_compensation().0
                + self.lamports_per_tx.0,
        )
    }

    pub fn commitment_hash_computation_fee(&self, min_batching_rate: u32) -> Lamports {
        let tx_count_total = commitment_hash_computation_instructions(min_batching_rate).len();
        let commitments_per_batch = commitments_per_batch(min_batching_rate);
        Lamports(div_ceiling_u64(
            tx_count_total as u64 * self.hash_tx_compensation().0,
            commitments_per_batch as u64,
        ))
    }

    pub fn proof_verification_computation_fee(
        &self,
        input_preparation_tx_count: usize,
    ) -> Lamports {
        let amount = (input_preparation_tx_count + u64_as_usize_safe(self.proof_base_tx_count))
            as u64
            * self.lamports_per_tx.0
            + self.warden_proof_reward.0;
        Lamports(amount)
    }

    pub fn proof_verification_fee(
        &self,
        input_preparation_tx_count: usize,
        min_batching_rate: u32,
        amount: u64,
        token_id: u16,
        price: &TokenPrice,
    ) -> Result<Token, TokenError> {
        let proof_verification_fee = self
            .proof_verification_computation_fee(input_preparation_tx_count)
            .into_token(price, token_id)?;
        let commitment_hash_fee = self
            .commitment_hash_computation_fee(min_batching_rate)
            .into_token(price, token_id)?;
        let network_fee = Token::new(token_id, self.proof_network_fee.calc(amount));
        let subvention = self.proof_subvention.into_token(price, token_id)?;

        ((proof_verification_fee + commitment_hash_fee)? + network_fee)? - subvention
    }
}
