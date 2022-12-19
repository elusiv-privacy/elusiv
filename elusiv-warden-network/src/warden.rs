use std::net::Ipv4Addr;
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_utils::{open_pda_account_with_offset, pda_account};
use solana_program::{
    pubkey::Pubkey,
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
};
use elusiv_types::{accounts::PDAAccountData, BorshSerDeSized, ProgramAccount};
use crate::{macros::{elusiv_account, BorshSerDeSized}, error::ElusivWardenNetworkError};

/// A unique ID publicly identifying a single Warden
pub type ElusivWardenID = u32;

/// The [`ElusivWardensAccount`] assigns each new Warden it's [`ElusivWardenID`]
#[elusiv_account(eager_type: true)]
pub struct ElusivWardensAccount {
    pda_data: PDAAccountData,

    pub next_warden_id: ElusivWardenID,
    pub full_network_configured: bool,
}

impl<'a> ElusivWardensAccount<'a> {
    fn inc_next_warden_id(&mut self) -> ProgramResult {
        let next_id = self.get_next_warden_id();

        #[allow(clippy::or_fun_call)]
        self.set_next_warden_id(
            &next_id
                .checked_add(1)
                .ok_or(ProgramError::from(ElusivWardenNetworkError::WardenRegistrationError))?
        );

        Ok(())
    }

    pub fn add_basic_warden<'b>(
        &mut self,
        payer: &AccountInfo<'b>,
        warden: ElusivBasicWarden,
        warden_account: &AccountInfo<'b>,
    ) -> ProgramResult {
        let warden_id = self.get_next_warden_id();
        self.inc_next_warden_id()?;

        open_pda_account_with_offset::<ElusivBasicWardenAccount>(
            &crate::id(),
            payer,
            warden_account,
            warden_id,
        )?;

        pda_account!(mut warden_account, ElusivBasicWardenAccount, warden_account);
        warden_account.set_warden(&warden);

        Ok(())
    }
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct FixedLenString<const MAX_LEN: usize> {
    len: u8,
    data: [u8; MAX_LEN],
}

pub type WardenIdentifier = FixedLenString<256>;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct ElusivBasicWardenConfig {
    pub ident: WardenIdentifier,
    pub key: Pubkey,
    pub owner: Pubkey,

    pub addr: Ipv4Addr,
    pub port: u16,

    pub country: u16,
    pub asn: u32,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct ElusivBasicWarden {
    pub warden_id: ElusivWardenID,
    pub config: ElusivBasicWardenConfig,
    pub lut: Pubkey,

    pub is_active: bool,

    pub join_timestamp: u64,
    pub activation_timestamp: u64,
}

/// An account associated to a single [`ElusivBasicWarden`]
#[elusiv_account(eager_type: true)]
pub struct ElusivBasicWardenAccount {
    pda_data: PDAAccountData,
    pub warden: ElusivBasicWarden,
}