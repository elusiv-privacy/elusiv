use crate::macros::{elusiv_account};
use crate::bytes::BorshSerDeSized;
use borsh::{BorshDeserialize, BorshSerialize};
use crate::state::program_account::SizedAccount;

#[elusiv_account(pda_seed = b"elusiv_fee")]
pub struct FeeAccount {
    bump_seed : u8,
    initialized: bool,

    network_fee: u64,
}