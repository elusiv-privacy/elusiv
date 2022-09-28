use elusiv_types::{PDAAccount, ProgramAccount};
use borsh::{BorshDeserialize, BorshSerialize};
use std::net::Ipv4Addr;
use solana_program::system_program;
use crate::apa::{APAECert, APAProposal, APAConfig, APAProposalAccount, APAAccount};
use crate::proposal::Vote;
use crate::warden::{ElusivBasicWardenAccount, ElusivFullWardenAccount, ElusivWardensAccount, ElusivWardenID};
use crate::macros::ElusivInstruction;
use crate::network::{FullWardenRegistrationAccount, ElusivFullWardenNetworkAccount};
use crate::processor;

#[derive(BorshDeserialize, BorshSerialize, ElusivInstruction)]
pub enum ElusivWardenNetworkInstruction {
    #[acc(payer, { signer, writable })]
    #[pda(warden_registration, FullWardenRegistrationAccount, { writable, find_pda, account_info })]
    #[pda(wardens, ElusivWardensAccount, { writable, find_pda, account_info })]
    #[pda(apa, APAAccount, { writable, find_pda, account_info })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    Init,

    // -------- Full Warden Genesis Network Setup --------

    #[acc(warden, { signer })]
    #[pda(warden_account, ElusivFullWardenAccount, pda_offset = Some(warden_id), { writable, find_pda, account_info })]
    #[pda(warden_registration, FullWardenRegistrationAccount, { writable })]
    #[pda(wardens, ElusivWardensAccount, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    ApplyFullGenesisWarden {
        warden_id: ElusivWardenID,
        apae_cert: APAECert,
        addr: Ipv4Addr,
    },

    #[acc(warden, { signer })]
    #[pda(warden_account, ElusivFullWardenAccount, pda_offset = Some(warden_id))]
    #[pda(warden_registration, FullWardenRegistrationAccount, { writable })]
    ConfirmFullGenesisWarden {
        warden_id: ElusivWardenID,
    },

    #[acc(warden, { signer, writable })]
    #[pda(warden_account, ElusivFullWardenAccount, pda_offset = Some(leader_id))]
    #[pda(warden_registration, FullWardenRegistrationAccount)]
    #[pda(wardens, ElusivWardensAccount, { writable })]
    #[pda(warden_network, ElusivFullWardenNetworkAccount, { writable, find_pda, account_info })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    CompleteFullGenesisWarden {
        leader_id: ElusivWardenID,
        apa_config: APAConfig,
    },

    // -------- Basic Warden Genesis Network Setup --------

    #[acc(warden, { signer, writable })]
    #[pda(warden_account, ElusivBasicWardenAccount, pda_offset = Some(warden_id), { writable, find_pda, account_info })]
    #[pda(wardens, ElusivWardensAccount, { writable })]
    ApplyBasicGenesisWarden {
        warden_id: ElusivWardenID,
        addr: Ipv4Addr,
    },

    #[acc(basic_warden_authority, { signer })]
    #[pda(warden_account, ElusivBasicWardenAccount, pda_offset = Some(warden_id), { writable })]
    ConfirmBasicGenesisWarden {
        warden_id: ElusivWardenID,
    },

    // -------- APA Proposals --------

    #[acc(proponent, { signer, writable })]
    #[acc(proposal_account, { writable, account_info })]
    #[pda(warden_network, ElusivFullWardenNetworkAccount)]
    InitApaProposal {
        proposal_id: u32,
        proposal: APAProposal,
    },

    #[acc(warden, { signer })]
    #[pda(warden_account, ElusivFullWardenAccount, pda_offset = Some(warden_id))]
    #[pda(proposal_account, APAProposalAccount, pda_offset = Some(proposal_id), { writable })]
    VoteApaProposal {
        warden_id: ElusivWardenID,
        proposal_id: u32,
        vote: Vote,
    },

    #[acc(warden, { signer })]
    #[pda(warden_account, ElusivFullWardenAccount, pda_offset = Some(warden_id))]
    #[pda(proposal_account, APAProposalAccount, pda_offset = Some(proposal_id), { writable })]
    #[pda(apa_account, APAAccount, { writable })]
    FinalizeApaProposal {
        warden_id: ElusivWardenID,
        proposal_id: u32,
        next_root: [u8; 32],
    }
}