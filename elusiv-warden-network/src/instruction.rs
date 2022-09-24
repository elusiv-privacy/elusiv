use elusiv_types::{PDAAccount, ProgramAccount};
use borsh::{BorshDeserialize, BorshSerialize};
use std::net::Ipv4Addr;
use solana_program::system_program;
use crate::apa::{APAECert, APAProposalKind, APAProposal, APAConfig};
use crate::proposal::Vote;
use crate::warden::{ElusivFullWardenAccount, ElusivWardensAccount, ElusivWardenID};
use crate::macros::ElusivInstruction;
use crate::network::{FullWardenRegistrationAccount, ElusivFullWardenNetworkAccount};
use crate::processor;

#[derive(BorshDeserialize, BorshSerialize, ElusivInstruction)]
pub enum ElusivWardenNetworkInstruction {
    // -------- Full Warden Genesis Network Setup --------
    #[acc(payer, { signer, writable })]
    #[pda(warden_registration, FullWardenRegistrationAccount, { writable, find_pda, account_info })]
    #[pda(wardens, ElusivWardensAccount, { writable, find_pda, account_info })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    Init,

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

    #[acc(warden, { signer })]
    #[pda(warden_account, ElusivFullWardenAccount, pda_offset = Some(leader_id))]
    #[pda(warden_registration, FullWardenRegistrationAccount)]
    #[pda(warden_network, ElusivFullWardenNetworkAccount, { writable, find_pda, account_info })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    CompleteFullGenesisWarden {
        leader_id: ElusivWardenID,
        apa_config: APAConfig,
    },

    // -------- APA Proposals --------
    InitApaProposal {
        kind: APAProposalKind,
        proposal: APAProposal,
    },

    #[acc(warden, { signer })]
    #[pda(warden_account, ElusivFullWardenAccount, pda_offset = Some(warden_id))]
    VoteApaProposal {
        warden_id: ElusivWardenID,
        vote: Vote,
    },

    #[acc(warden, { signer })]
    #[pda(warden_account, ElusivFullWardenAccount, pda_offset = Some(leader_id))]
    FinalizeApaProposal {
        leader_id: ElusivWardenID,
    }
}