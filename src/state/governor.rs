use crate::macros::elusiv_account;
use crate::bytes::BorshSerDeSized;
use borsh::{BorshDeserialize, BorshSerialize};
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
}

#[elusiv_account(pda_seed = b"sol_pool")]
pub struct PoolAccount {
    pda_data: PDAAccountData,
}

#[elusiv_account(pda_seed = b"fee_collector")]
/// Collects the network fees
pub struct FeeCollectorAccount {
    pda_data: PDAAccountData,
}