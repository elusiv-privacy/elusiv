use std::net::Ipv4Addr;
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_utils::{open_pda_account_with_offset, pda_account, guard};
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
pub struct WardensAccount {
    pda_data: PDAAccountData,

    pub next_warden_id: ElusivWardenID,
    pub full_network_configured: bool,
}

impl<'a> WardensAccount<'a> {
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

        open_pda_account_with_offset::<BasicWardenAccount>(
            &crate::id(),
            payer,
            warden_account,
            warden_id,
        )?;

        pda_account!(mut warden_account, BasicWardenAccount, warden_account);
        warden_account.set_warden(&warden);

        Ok(())
    }
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Debug)]
pub struct FixedLenString<const MAX_LEN: usize> {
    len: u8,
    data: [u8; MAX_LEN],
}

pub type Identifier = FixedLenString<256>;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Debug)]
pub struct ElusivBasicWardenConfig {
    pub ident: Identifier,
    pub key: Pubkey,
    pub owner: Pubkey,

    pub addr: Ipv4Addr,
    pub port: u16,

    pub country: u16,
    pub asn: u32,

    pub version: [u16; 3],
    pub platform: Identifier,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Debug)]
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
pub struct BasicWardenAccount {
    pda_data: PDAAccountData,
    pub warden: ElusivBasicWarden,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Debug)]
pub struct WardenStatistics {
    pub activity: [u32; 366],
    pub total: u32,
}

const BASE_YEAR: u16 = 2022;
const YEARS_COUNT: usize = 100;
const WARDENS_COUNT: u32 = u32::MAX / YEARS_COUNT as u32;

impl WardenStatistics {
    pub fn inc(&self, day: u32) -> Result<&Self, ProgramError> {
        guard!(day < 366, ElusivWardenNetworkError::StatsError);

        self.total.checked_add(1)
            .ok_or(ElusivWardenNetworkError::Overflow)?;

        self.activity[day as usize].checked_add(1)
            .ok_or(ElusivWardenNetworkError::Overflow)?;

        Ok(self)
    }
}

/// An account associated to a single [`ElusivBasicWarden`] storing activity statistics for a single year
#[elusiv_account(eager_type: true)]
pub struct BasicWardenStatsAccount {
    pda_data: PDAAccountData,

    pub warden_id: ElusivWardenID,
    pub year: u16,

    pub store: WardenStatistics,
    pub send: WardenStatistics,
    pub migrate: WardenStatistics,
}

pub fn stats_account_pda_offset(warden_id: ElusivWardenID, year: u16) -> u32 {
    assert!(year >= BASE_YEAR);
    assert!(warden_id < WARDENS_COUNT);

    (year - BASE_YEAR) as u32 * WARDENS_COUNT + warden_id
}