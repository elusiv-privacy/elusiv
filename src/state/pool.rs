//! Currently the single SOL pool used to store funds

use crate::macros::{elusiv_account};
use crate::bytes::BorshSerDeSized;
use borsh::{BorshDeserialize, BorshSerialize};
use crate::state::program_account::SizedAccount;

#[elusiv_account(pda_seed = b"sol_pool")]
pub struct PoolAccount {
    bump_seed: u8,
    initialized: bool,
}