use crate::macros::{elusiv_account};
use crate::bytes::BorshSerDeSized;
use crate::types::U256;
use borsh::{BorshDeserialize, BorshSerialize};
use crate::state::program_account::SizedAccount;

#[elusiv_account(pda_seed = b"pool")]
pub struct PoolAccount {
    bump_seed: u8,
    initialized: bool,

    sol_pool: U256,
    fee_collector: U256,
}