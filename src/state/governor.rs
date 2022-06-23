use crate::macros::elusiv_account;
use crate::bytes::BorshSerDeSized;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey;
use crate::state::program_account::SizedAccount;

pub const GOVERNOR_UPGRADE_AUTHORITY: pubkey::Pubkey = pubkey!("My11111111111111111111111111111111111111111");

#[elusiv_account(pda_seed = b"governor")]
pub struct GovernorAccount {
    bump_seed: u8,
    version: u8,
    initialized: bool,

    /// the current fee-version (new requests are forced to use this version)
    fee_version: u64,

    /// the number of commitments in a MT-root hashing batch
    commitment_batching_rate: u32,
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