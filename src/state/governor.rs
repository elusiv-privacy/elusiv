use crate::macros::elusiv_account;
use crate::bytes::BorshSerDeSized;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::native_token::LAMPORTS_PER_SOL;
use crate::state::program_account::SizedAccount;
use super::{program_account::PDAAccountData, fee::ProgramFee};

#[elusiv_account(pda_seed = b"governor")]
pub struct GovernorAccount {
    pda_data: PDAAccountData,

    /// The current fee-version (new requests are forced to use this version)
    fee_version: u64,

    /// The `ProgramFee` for the `FeeAccount` with the offset `fee_version`
    program_fee: ProgramFee,

    /// The number of commitments in a MT-root hashing batch
    commitment_batching_rate: u32,

    program_version: u64,
}

#[elusiv_account(pda_seed = b"sol_pool")]
pub struct PoolAccount {
    pda_data: PDAAccountData,
}

pub const FEE_COLLECTOR_MINIMUM_BALANCE: u64 = LAMPORTS_PER_SOL;

#[elusiv_account(pda_seed = b"fee_collector")]
/// Collects the network fees
pub struct FeeCollectorAccount {
    pda_data: PDAAccountData,
}