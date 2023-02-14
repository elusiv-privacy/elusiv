use crate::warden::Identifier;
use elusiv_proc_macros::elusiv_account;
use elusiv_types::{ElusivOption, PDAAccountData};
use solana_program::pubkey::Pubkey;

/// An account associated with the operator of one or more [`ElusivBasicWarden`]s
#[elusiv_account]
pub struct WardenOperatorAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,

    pub key: Pubkey,
    pub ident: Identifier,
    pub url: Identifier,
    pub jurisdiction: ElusivOption<u16>,
}
