use crate::{
    error::ElusivWardenNetworkError,
    macros::{elusiv_account, BorshSerDeSized},
};
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_types::{accounts::PDAAccountData, TOKENS};
use elusiv_utils::guard;
use solana_program::{program_error::ProgramError, pubkey::Pubkey};
use std::net::Ipv4Addr;

/// A unique ID publicly identifying a single Warden
pub type ElusivWardenID = u32;

/// The [`ElusivWardensAccount`] assigns each new Warden it's [`ElusivWardenID`]
#[elusiv_account(eager_type: true)]
pub struct WardensAccount {
    pda_data: PDAAccountData,

    pub next_warden_id: ElusivWardenID,
    pub full_network_configured: bool,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Debug, Clone, PartialEq)]
pub struct FixedLenString<const MAX_LEN: usize> {
    len: u64,
    data: [u8; MAX_LEN],
}

#[cfg(feature = "elusiv-client")]
impl<const MAX_LEN: usize> TryFrom<String> for FixedLenString<MAX_LEN> {
    type Error = std::io::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() > MAX_LEN {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "String is too long",
            ));
        }

        let mut data = [0; MAX_LEN];
        data[..value.len()].copy_from_slice(value.as_bytes());

        Ok(Self {
            len: value.len() as u64,
            data,
        })
    }
}

pub type Identifier = FixedLenString<256>;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Default, Debug, Clone, PartialEq)]
pub struct ElusivBasicWardenFeatures {
    pub apa: bool,
    pub rpc: bool,
    pub relay: bool,
    pub instant_relay: bool,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Debug, Clone, PartialEq)]
pub enum TlsMode {
    NoTls,
    Optional,
    Required,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Debug, Clone, PartialEq)]
pub struct ElusivBasicWardenConfig {
    pub ident: Identifier,
    pub key: Pubkey,
    pub owner: Pubkey,

    pub addr: Ipv4Addr,
    pub port: u16,
    pub tls_mode: TlsMode,

    pub jurisdiction: u16,
    pub timezone: u16,

    pub version: [u16; 3],
    pub platform: Identifier,

    pub features: ElusivBasicWardenFeatures,
    pub tokens: [bool; TOKENS.len()],
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Debug, Clone)]
pub struct ElusivBasicWarden {
    pub config: ElusivBasicWardenConfig,
    pub lut: Pubkey,
    pub is_active: bool,
    pub join_timestamp: u64,
    /// The timestamp of the last change of `is_active`
    pub activation_timestamp: u64,
}

/// An account associated with a single [`ElusivBasicWarden`]
#[elusiv_account(eager_type: true)]
pub struct BasicWardenAccount {
    pda_data: PDAAccountData,
    pub warden: ElusivBasicWarden,
}

/// An account associated with a single [`ElusivBasicWarden`]
#[elusiv_account(eager_type: true)]
pub struct BasicWardenMapAccount {
    pda_data: PDAAccountData,
    pub warden_id: ElusivWardenID,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Debug, Clone)]
pub struct WardenStatistics {
    pub activity: [u32; 366],
    pub total: u32,
}

impl WardenStatistics {
    pub fn inc(&self, day: u32) -> Result<&Self, ProgramError> {
        guard!(day < 366, ElusivWardenNetworkError::StatsError);

        self.total
            .checked_add(1)
            .ok_or(ElusivWardenNetworkError::Overflow)?;

        self.activity[day as usize]
            .checked_add(1)
            .ok_or(ElusivWardenNetworkError::Overflow)?;

        Ok(self)
    }
}

/// An account associated to a single [`ElusivBasicWarden`] storing activity statistics for a single year
#[elusiv_account(eager_type: true)]
pub struct BasicWardenStatsAccount {
    pda_data: PDAAccountData,

    pub year: u16,

    pub store: WardenStatistics,
    pub send: WardenStatistics,
    pub migrate: WardenStatistics,
}
