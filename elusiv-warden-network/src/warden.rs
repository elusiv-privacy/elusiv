use std::net::Ipv4Addr;
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_utils::guard;
use solana_program::{pubkey::Pubkey, program_error::ProgramError};
use elusiv_types::{accounts::PDAAccountData, TOKENS, ElusivOption};
use crate::{macros::{elusiv_account, BorshSerDeSized}, error::ElusivWardenNetworkError};

/// A unique ID publicly identifying a single Warden
pub type ElusivWardenID = u32;

/// The [`ElusivWardensAccount`] assigns each new Warden it's [`ElusivWardenID`]
#[elusiv_account(eager_type: true)]
pub struct WardensAccount {
    pda_data: PDAAccountData,

    pub next_warden_id: ElusivWardenID,
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
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "String is too long"))
        }

        let mut data = [0; MAX_LEN];
        data[..value.len()].copy_from_slice(value.as_bytes());

        Ok(
            Self {
                len: value.len() as u64,
                data,
            }
        )
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
    pub operator: ElusivOption<Pubkey>,

    pub addr: Ipv4Addr,
    pub rpc_port: u16,
    pub tls_mode: TlsMode,

    pub jurisdiction: u16,
    pub timezone: u16,
    pub location: u16,

    pub version: [u16; 3],
    pub platform: Identifier,

    pub features: ElusivBasicWardenFeatures,
    pub tokens: [bool; TOKENS.len()],
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Debug, Clone)]
pub struct ElusivBasicWarden {
    pub config: ElusivBasicWardenConfig,
    pub lut: Pubkey,

    pub is_operator_confirmed: bool,
    pub is_metadata_valid: ElusivOption<bool>,
    pub is_active: bool,

    pub join_timestamp: u64,

    /// Indicates the last time, `is_active` has been changed
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

        self.total.checked_add(1)
            .ok_or(ElusivWardenNetworkError::Overflow)?;

        self.activity[day as usize].checked_add(1)
            .ok_or(ElusivWardenNetworkError::Overflow)?;

        Ok(self)
    }
}

/// An account associated with a single [`ElusivBasicWarden`] storing activity statistics for a single year
#[elusiv_account(eager_type: true)]
pub struct BasicWardenStatsAccount {
    pda_data: PDAAccountData,

    pub year: u16,

    pub store: WardenStatistics,
    pub send: WardenStatistics,
    pub migrate: WardenStatistics,

    pub last_activity_timestamp: u64,
}

/// An account associated with the operator of one or more [`ElusivBasicWarden`]s
#[elusiv_account(eager_type: true)]
pub struct BasicWardenOperatorAccount {
    pda_data: PDAAccountData,

    pub key: Pubkey,
    pub ident: Identifier,
    pub url: Identifier,
    pub jurisdiction: ElusivOption<u16>,
}