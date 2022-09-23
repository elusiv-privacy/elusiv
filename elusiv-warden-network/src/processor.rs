use crate::{apa::{APAProposalKind, APAProposal, APAECert}, proposal::Vote, warden::ElusivWardenID};
use solana_program::entrypoint::ProgramResult;

pub fn apply_full_genesis_warden_setup(
    warden_id: ElusivWardenID,
    apae_cert: APAECert,
) -> ProgramResult {
    todo!()
}

pub fn approve_full_genesis_warden_setup(
    warden_id: ElusivWardenID,
) -> ProgramResult {
    todo!()
}

pub fn complete_full_genesis_warden_setup(
    leader_id: ElusivWardenID,
) -> ProgramResult {
    todo!()
}

pub fn init_apa_proposal(
    kind: APAProposalKind,
    proposal: APAProposal,
) -> ProgramResult {
    todo!()
}

pub fn vote_apa_proposal(
    vote: Vote,
) -> ProgramResult {
    todo!()
}

pub fn finalize_apa_proposal(
    leader_id: ElusivWardenID,
) -> ProgramResult {
    todo!()
}