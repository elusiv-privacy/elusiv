use std::net::Ipv4Addr;
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_utils::{open_pda_account_with_offset, guard, pda_account};
use solana_program::{
    pubkey::Pubkey,
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
};
use elusiv_types::{accounts::PDAAccountData, BorshSerDeSized, ProgramAccount};
use crate::{macros::{elusiv_account, BorshSerDeSized}, error::ElusivWardenNetworkError};

pub type ElusivWardenID = u32;

#[elusiv_account(single_instance: true)]
pub struct ElusivWardensAccount {
    pda_data: PDAAccountData,

    next_warden_id: ElusivWardenID,
    pub full_network_configured: bool,
}

impl<'a> ElusivWardensAccount<'a> {
    #[allow(clippy::or_fun_call)]
    fn bump_next_warden_id(&mut self) -> ProgramResult {
        let next_id = self.get_next_warden_id();
        self.set_next_warden_id(
            &(
                next_id
                    .checked_add(1)
                    .ok_or(ProgramError::from(ElusivWardenNetworkError::WardenRegistrationError))?
            )
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
        self.bump_next_warden_id()?;

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

    pub fn add_full_warden<'b>(
        &mut self,
        payer: &AccountInfo<'b>,
        warden: ElusivFullWarden,
        warden_account: &AccountInfo<'b>,
    ) -> ProgramResult {
        let warden_id = self.get_next_warden_id();
        self.bump_next_warden_id()?;

        open_pda_account_with_offset::<ElusivFullWardenAccount>(
            &crate::id(),
            payer,
            warden_account,
            warden_id,
        )?;

        pda_account!(mut warden_account, ElusivFullWardenAccount, warden_account);
        warden_account.set_warden(&warden);

        Ok(())
    }
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct ElusivBasicWarden {
    pub key: Pubkey,
    pub addr: Ipv4Addr,
    pub active: bool,
}

#[elusiv_account]
pub struct ElusivBasicWardenAccount {
    pda_data: PDAAccountData,
    pub warden: ElusivBasicWarden,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct ElusivFullWarden {
    pub warden: ElusivBasicWarden,
    pub apae_key: Pubkey,
}

#[elusiv_account]
pub struct ElusivFullWardenAccount {
    pda_data: PDAAccountData,
    warden: ElusivFullWarden,
}

impl<'a> ElusivFullWardenAccount<'a> {
    pub fn verify(&self, signer: &AccountInfo) -> ProgramResult {
        guard!(signer.is_signer, ElusivWardenNetworkError::InvalidSignature);
        guard!(*signer.key == self.get_warden().warden.key, ElusivWardenNetworkError::InvalidSignature);

        Ok(())
    }
}