mod poseidon_hash;
mod poseidon_constants;
pub mod state;

pub use poseidon_hash::*;
pub use poseidon_constants::ITERATIONS;
use crate::macros::elusiv_account;
use crate::state::program_account::PartialComputationAccount;
use crate::types::U256;

/// Account used for hashing commitments and forming new merkle roots
#[elusiv_account(pda_seed = b"commitment")]
pub struct CommitmentHashingAccount {
    // `PartialComputationAccount` trait fields
    is_active: bool,
    round: u64,
    total_rounds: u64,
    fee_payer: U256,

    // 
}