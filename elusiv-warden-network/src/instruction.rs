use crate::apa::{APAECert, APAProposalKind, APAProposal};
use crate::proposal::Vote;
use crate::warden::ElusivWardenID;
use crate::macros::ElusivInstruction;
use borsh::{BorshDeserialize, BorshSerialize};
use crate::processor;

#[derive(BorshDeserialize, BorshSerialize, ElusivInstruction)]
pub enum ElusivWardenNetworkInstruction {
    // A Full Warden (with the corresponding APAE) applies for registration
    ApplyFullGenesisWardenSetup {
        warden_id: ElusivWardenID,
        apae_cert: APAECert,
    },

    // A registering Warden (+ each APAE) approves all other applicants
    ApproveFullGenesisWardenSetup {
        warden_id: ElusivWardenID,
    },

    // The registration leader generates the APA config
    CompleteFullGenesisWardenSetup {
        leader_id: ElusivWardenID,
    },

    InitApaProposal {
        kind: APAProposalKind,
        proposal: APAProposal,
    },

    VoteApaProposal {
        vote: Vote,
    },

    FinalizeApaProposal {
        leader_id: ElusivWardenID,
    }
}