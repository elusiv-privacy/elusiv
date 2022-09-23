use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;
use elusiv_types::{accounts::PDAAccountData, BorshSerDeSized, SizedAccount};
use crate::macros::{elusiv_account, BorshSerDeSized};

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct ElusivBasicWarden {
    pub key: Pubkey,
}

#[elusiv_account(pda_seed = b"basic_warden")]
pub struct ElusivBasicWardenAccount {
    pda_data: PDAAccountData,
    warden: ElusivBasicWarden,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct ElusivFullWarden {
    pub key: Pubkey,
    pub apae_key: Pubkey,
}

pub type ElusivWardenID = u32;