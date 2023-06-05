use crate::{
    error::ElusivWardenNetworkError,
    macros::{elusiv_account, BorshSerDeSized},
};
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_types::{accounts::PDAAccountData, ElusivOption, TOKENS};
use elusiv_utils::guard;
use solana_program::{program_error::ProgramError, pubkey::Pubkey};
use std::net::Ipv4Addr;

/// An unique ID publicly identifying a single Warden
pub type ElusivWardenID = u32;

/// The [`ElusivWardensAccount`] assigns each new Warden it's [`ElusivWardenID`]
#[elusiv_account(eager_type: true)]
pub struct WardensAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,

    pub next_warden_id: ElusivWardenID,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Clone, PartialEq, Eq)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug))]
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

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Default, Clone, PartialEq, Eq)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug))]
pub struct WardenFeatures {
    pub apa: bool,
    pub attestation: bool,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Default, Clone, PartialEq, Eq)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug))]
pub struct BasicWardenFeatures {
    pub rpc: bool,
    pub relay: bool,
    pub instant_relay: bool,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Clone, PartialEq, Eq)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug))]
pub enum TlsMode {
    NoTls,
    Optional,
    Required,
}

/// An IANA timezone
#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Clone, PartialEq)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug))]
pub struct Timezone {
    /// The tz area index in alphabetical order in `[0; 11)`
    pub area: u8,
    pub location: FixedLenString<14>,
}

/// The geographic region of a Warden
///
/// # Notes
///
/// - Based on the IANA tz database (https://data.iana.org/time-zones/tz-link.html), ommiting the oceans.
/// - We simplify by mapping the oceans as follows:
///     - Arctic -> Europe,
///     - Atlantic -> America,
///     - Indian -> Asia,
///     - Pacific -> Asia
#[repr(u8)]
#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug))]
pub enum WardenRegion {
    Africa,
    America,
    Antarctica,
    Asia,
    Australia,
    Europe,
    Other, // Other is used to represent the tz Etc area or orbital locations
}

impl WardenRegion {
    #[cfg(feature = "elusiv-client")]
    pub fn from_tz_timezone_area(area: &str) -> Option<Self> {
        match area {
            "Africa" => Some(WardenRegion::Africa),
            "America" => Some(WardenRegion::America),
            "Antarctica" => Some(WardenRegion::Antarctica),
            "Arctic" => Some(WardenRegion::Europe),
            "Asia" => Some(WardenRegion::Asia),
            "Atlantic" => Some(WardenRegion::America),
            "Australia" => Some(WardenRegion::Australia),
            "Europe" => Some(WardenRegion::Europe),
            "Etc" => Some(WardenRegion::Other),
            "Indian" => Some(WardenRegion::Asia),
            "Pacific" => Some(WardenRegion::Asia),
            _ => None,
        }
    }
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Clone, PartialEq)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug))]
pub struct ElusivBasicWardenConfig {
    pub ident: Identifier,
    pub key: Pubkey,
    pub operator: ElusivOption<Pubkey>,

    pub addr: Ipv4Addr,
    pub rpc_port: u16,
    pub tls_mode: TlsMode,
    pub uses_proxy: bool,

    pub jurisdiction: u16,
    pub timezone: Timezone,
    pub region: WardenRegion,

    pub version: [u16; 3],
    pub platform: Identifier,

    pub warden_features: WardenFeatures,
    pub basic_warden_features: BasicWardenFeatures,
    pub tokens: [bool; TOKENS.len()],
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Clone)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug))]
pub struct ElusivBasicWarden {
    pub config: ElusivBasicWardenConfig,
    pub lut: Pubkey,

    pub asn: ElusivOption<u32>,

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
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,

    pub warden: ElusivBasicWarden,
}

/// An account associated with a single [`ElusivBasicWarden`]
#[elusiv_account(eager_type: true)]
pub struct BasicWardenMapAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,

    pub warden_id: ElusivWardenID,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Clone)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug))]
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

/// An account associated with a single [`ElusivBasicWarden`] storing activity statistics for a single year
#[elusiv_account(eager_type: true)]
pub struct BasicWardenStatsAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,

    pub year: u16,
    pub last_activity_timestamp: u64,

    pub store: WardenStatistics,
    pub send: WardenStatistics,
    pub migrate: WardenStatistics,
}

/// An account associated with a single [`ElusivBasicWarden`]
#[elusiv_account]
pub struct BasicWardenAttesterMapAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,

    pub warden_id: ElusivWardenID,
}

const HALF_QUOTE_SIZE: usize = 558;
const FULL_QUOTE_SIZE: usize = 1116;

/// An SGX quote.
/// See [the remote attestation crate](https://github.com/elusiv-privacy/rust-sgx-remote-attestation)
#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Clone)]
pub struct Quote(pub [u8; FULL_QUOTE_SIZE]);

/// The first half of an SGX quote.
/// Because quotes almost are the maximum size of a transactions, they are split in two.
///
/// See also [`QuoteEnd`]
#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Clone)]
pub struct QuoteStart(pub [u8; HALF_QUOTE_SIZE]);

impl QuoteStart {
    pub fn user_data_bytes(&self) -> [u8; 32] {
        self.0[368 + 32..368 + 64].try_into().unwrap()
    }

    pub fn join(&self, end: &QuoteEnd) -> Quote {
        let mut full_quote = [0u8; FULL_QUOTE_SIZE];

        full_quote[..HALF_QUOTE_SIZE].copy_from_slice(&self.0);
        full_quote[HALF_QUOTE_SIZE..].copy_from_slice(&end.0);

        Quote(full_quote)
    }
}

/// The second half of an SGX quote.
/// Because quotes almost are the maximum size of a transactions, they are split in two.
///
/// See also [`QuoteStart`]
#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Clone)]
pub struct QuoteEnd(pub [u8; HALF_QUOTE_SIZE]);

#[elusiv_account]
pub struct ApaWardenAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,

    pub warden_id: ElusivWardenID,
    pub network_member_index: u32,
    // pub latest_quote: Quote,
}
