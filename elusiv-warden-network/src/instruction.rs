#![allow(clippy::large_enum_variant)]
#![allow(clippy::too_many_arguments)]

use crate::apa::{ApaProposal, ApaProposalsAccount, ApaTargetMapAccount};
use crate::macros::ElusivInstruction;
use crate::network::{ApaWardenNetworkAccount, BasicWardenNetworkAccount};
use crate::processor;
use crate::warden::{
    ApaWardenAccount, BasicWardenAccount, BasicWardenAttesterMapAccount, BasicWardenMapAccount,
    BasicWardenStatsAccount, ElusivBasicWardenConfig, ElusivWardenID, Identifier, QuoteEnd,
    QuoteStart, Timezone, WardenRegion, WardensAccount,
};
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_types::AccountRepr;
use solana_program::pubkey::Pubkey;
use solana_program::system_program;
use solana_program::sysvar::instructions;

#[cfg(feature = "elusiv-client")]
use crate::apa::ApaProposalAccount;
#[cfg(feature = "elusiv-client")]
use crate::operator::WardenOperatorAccount;
#[cfg(feature = "elusiv-client")]
pub use elusiv_types::accounts::{
    SignerAccount, UserAccount, WritableSignerAccount, WritableUserAccount,
};

#[repr(u8)]
#[derive(BorshDeserialize, BorshSerialize, ElusivInstruction)]
pub enum ElusivWardenNetworkInstruction {
    // -------- Program initialization --------
    #[acc(payer, { signer, writable })]
    #[pda(wardens, WardensAccount, { writable, skip_pda_verification, account_info })]
    #[pda(basic_network, BasicWardenNetworkAccount, { writable, skip_pda_verification, account_info })]
    #[pda(apa_network, ApaWardenNetworkAccount, { writable, skip_pda_verification, account_info })]
    #[pda(proposals_account, ApaProposalsAccount, { writable, skip_pda_verification, account_info })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    Init,

    // -------- Basic Warden --------
    #[acc(warden, { signer, writable })]
    #[pda(warden_account, BasicWardenAccount, pda_offset = Some(warden_id), { writable, skip_pda_verification, account_info })]
    #[pda(warden_map_account, BasicWardenMapAccount, pda_pubkey = config.key, { writable, skip_pda_verification, account_info })]
    #[pda(wardens, WardensAccount, { writable })]
    #[pda(basic_network, BasicWardenNetworkAccount, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    RegisterBasicWarden {
        warden_id: ElusivWardenID,
        config: ElusivBasicWardenConfig,
    },

    #[acc(warden, { signer })]
    #[pda(warden_account, BasicWardenAccount, pda_offset = Some(warden_id), { writable })]
    UpdateBasicWardenState {
        warden_id: ElusivWardenID,
        is_active: bool,
    },

    #[acc(warden, { signer })]
    #[pda(warden_account, BasicWardenAccount, pda_offset = Some(warden_id), { writable })]
    #[acc(lut_account)]
    UpdateBasicWardenLut {
        warden_id: ElusivWardenID,
    },

    // -------- APA Warden --------
    #[acc(warden, { signer, writable })]
    #[pda(warden_map_account, BasicWardenMapAccount, pda_pubkey = warden.pubkey())]
    #[pda(apa_warden_account, ApaWardenAccount, pda_offset = Some(warden_id), { writable, skip_pda_verification, account_info })]
    #[pda(apa_network, ApaWardenNetworkAccount, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    StartApaGenesisWardenApplication {
        warden_id: ElusivWardenID,
        quote_start: QuoteStart,
    },

    #[acc(warden, { signer })]
    #[pda(warden_map_account, BasicWardenMapAccount, pda_pubkey = warden.pubkey())]
    #[pda(apa_network, ApaWardenNetworkAccount, { writable })]
    CompleteApaGenesisWardenApplication {
        warden_id: ElusivWardenID,
        quote_end: QuoteEnd,
    },

    #[acc(exchange_key, { signer })]
    #[pda(apa_warden_account, ApaWardenAccount, pda_offset = Some(warden_id))]
    #[pda(apa_network, ApaWardenNetworkAccount, { writable })]
    ConfirmApaGenesisNetwork {
        warden_id: ElusivWardenID,
        confirmation_message: [u8; 32],
    },

    CompleteApaGenesisNetwork,

    // -------- Warden operator --------
    #[acc(operator, { signer, writable })]
    #[pda(operator_account, WardenOperatorAccount, pda_pubkey = operator.pubkey(), { writable, skip_pda_verification, account_info })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    RegisterWardenOperator {
        ident: Identifier,
        url: Identifier,
        jurisdiction: Option<u16>,
    },

    #[acc(operator, { signer })]
    #[pda(warden_account, BasicWardenAccount, pda_offset = Some(warden_id), { writable })]
    ConfirmBasicWardenOperation {
        warden_id: ElusivWardenID,
    },

    // -------- Basic Warden statistics --------
    #[acc(warden)]
    #[acc(payer, { signer, writable })]
    #[pda(stats_account, BasicWardenStatsAccount, pda_pubkey = warden.pubkey(), pda_offset = Some(year.into()), { writable, skip_pda_verification, account_info })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    OpenBasicWardenStatsAccount {
        year: u16,
    },

    #[acc(warden)]
    #[pda(stats_account, BasicWardenStatsAccount, pda_pubkey = warden.pubkey(), pda_offset = Some(year.into()), { writable })]
    #[sys(instructions, key = instructions::ID)]
    TrackBasicWardenStats {
        year: u16,
        can_fail: bool,
    },

    // -------- APA --------
    #[acc(proponent, { signer, writable })]
    #[pda(proposal_account, ApaProposalAccount, pda_offset = Some(proposal_id), { writable, skip_pda_verification, account_info })]
    #[pda(proposals_account, ApaProposalsAccount, { writable })]
    #[pda(map_account, ApaTargetMapAccount, pda_pubkey = proposal.target, { writable, find_pda, account_info })]
    #[acc(token_mint)]
    #[sys(system_program, key = system_program::ID, { ignore })]
    ProposeApaProposal {
        proposal_id: u32,
        proposal: ApaProposal,
    },

    // -------- Metadata attestation --------
    #[acc(signer, { signer, writable })]
    #[pda(attester_account, BasicWardenAttesterMapAccount, pda_pubkey = attester, { writable, skip_pda_verification, account_info })]
    #[pda(warden_account, BasicWardenAccount, pda_offset = Some(warden_id), { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    AddMetadataAttester {
        warden_id: ElusivWardenID,
        attester: Pubkey,
    },

    #[acc(signer, { signer, writable })]
    #[acc(attester)]
    #[pda(attester_account, BasicWardenAttesterMapAccount, pda_pubkey = attester.pubkey(), { writable, account_info })]
    #[pda(warden_account, BasicWardenAccount, pda_offset = Some(warden_id), { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    RevokeMetadataAttester {
        warden_id: ElusivWardenID,
    },

    #[acc(attester, { signer })]
    #[pda(attester_warden_account, BasicWardenAccount, pda_offset = Some(attester_warden_id))]
    #[pda(warden_account, BasicWardenAccount, pda_offset = Some(warden_id), { writable })]
    #[pda(basic_network, BasicWardenNetworkAccount, { writable })]
    AttestBasicWardenMetadata {
        attester_warden_id: ElusivWardenID,
        warden_id: ElusivWardenID,
        member_index: u32,
        asn: Option<u32>,
        timezone: Timezone,
        region: WardenRegion,
        uses_proxy: bool,
    },

    // -------- Program state management --------
    #[cfg(not(feature = "mainnet"))]
    #[acc(payer, { signer })]
    #[acc(recipient, { writable })]
    #[acc(program_account, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    CloseProgramAccount,
}
