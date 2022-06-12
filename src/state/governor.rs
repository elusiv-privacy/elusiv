use crate::macros::{elusiv_account};
use crate::bytes::BorshSerDeSized;
use borsh::{BorshDeserialize, BorshSerialize};
use crate::state::program_account::SizedAccount;

pub const DEFAULT_COMMITMENT_BATCHING_RATE: u64 = 1;

#[elusiv_account(pda_seed = b"governor")]
pub struct GovernorAccount {
    bump_seed: u8,
    version: u8,
    initialized: bool,

    /// the current fee-version (new requests are forced to use this version)
    fee_version: u64,

    /// the number of commitments in a MT-root hashing batch
    commitment_batching_rate: u64,
}

#[elusiv_account(pda_seed = b"sol_pool")]
pub struct PoolAccount {
    bump_seed: u8,
    version: u8,
    initialized: bool,
}

#[elusiv_account(pda_seed = b"fee_collector")]
/// Collects the network fees
pub struct FeeCollectorAccount {
    bump_seed: u8,
    version: u8,
    initialized: bool,
}