use crate::warden::FixedLenString;
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_derive::BorshSerDeSized;
use elusiv_proc_macros::elusiv_account;
use elusiv_types::{accounts::PDAAccountData, tokens::TokenID, ElusivOption};
use solana_program::pubkey::Pubkey;

#[repr(u8)]
#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Debug)]
#[cfg_attr(feature = "elusiv-client", derive(Clone, PartialEq))]
pub enum ApaLevel {
    Flag1,
    Flag2,
    Outcast,
}

#[repr(u8)]
#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Debug)]
#[cfg_attr(feature = "elusiv-client", derive(Clone, PartialEq))]
pub enum ApaProponentRole {
    Default,
    TokenFreezingAuthority,
}

#[cfg(feature = "elusiv-client")]
impl Default for ApaProponentRole {
    fn default() -> Self {
        ApaProponentRole::Default
    }
}

pub type ApaReason = FixedLenString<512>;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
#[cfg_attr(feature = "elusiv-client", derive(Clone, PartialEq, Debug))]
pub struct ApaProposal {
    pub proponent: Pubkey,
    pub proponent_role: ApaProponentRole,
    pub timestamp: u64,
    pub target: Pubkey,
    pub level: ApaLevel,
    pub token_constraint: ElusivOption<TokenID>,
    pub reason: ApaReason,
}

#[elusiv_account(eager_type: true)]
pub struct ApaProposalAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,
    pub proposal: ApaProposal,
}

/// Maps an APA-target's pubkey to proposal-ids
///
/// # Notes
///
/// Maps the PDA with [`None`] [`elusiv_types::PDAOffset`] to the proposal-id of the [`ApaProposal`] with the highest [`ApaLevel`].
/// If there are multiple proposals, the successfull one is used.
#[elusiv_account(eager_type: true)]
pub struct ApaTargetMapAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,
    pub proposal_id: ElusivOption<u32>,
}

#[elusiv_account(eager_type: true)]
pub struct ApaProposalsAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,
    pub number_of_proposals: u32,
}
