use crate::{
    network::{FULL_WARDEN_GENESIS_NETWORK_SIZE, ElusivFullWardenGenesisNetwork},
    proposal::{Proposal, ProponentConstraint, ProposalVoteConsensus, SUPERMAJORITY_CONSENSUS_OF_ALL_MEMBERS, MAJORITY_CONSENSUS_OF_ALL_MEMBERS, DEFAULT_VOTING_TIME},
};
use solana_program::pubkey::Pubkey;
use borsh::{BorshDeserialize, BorshSerialize};

pub struct APARegistrationAccount {
    pub applicants: [APAECert; FULL_WARDEN_GENESIS_NETWORK_SIZE],
    pub approved_others: [bool; FULL_WARDEN_GENESIS_NETWORK_SIZE],
}

pub struct APAGenesisConfigAccount {
    pub apa_key: Pubkey,

    /// All participating genesis APAEs
    pub apaes: [APAECert; FULL_WARDEN_GENESIS_NETWORK_SIZE],
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct RemoteAttestationCert { }

#[derive(BorshDeserialize, BorshSerialize)]
pub struct APAECert {
    pub warden: [u8; 32],
    pub ra_cert: RemoteAttestationCert,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub enum APAReason {
    Custom(u32),
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct APAProposal {
    pub key: Pubkey,
    pub reason: APAReason,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub enum APAProposalKind {
    Flagging,
    Outcast,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct APAFlaggingProposal {
    pub apa_proposal: APAProposal,
}

impl Proposal for APAFlaggingProposal {
    type Network = ElusivFullWardenGenesisNetwork;
    const PROPONENT_CONSTRAINT: ProponentConstraint = ProponentConstraint::Any;
    const CONSENSUS_REQUIREMENT: ProposalVoteConsensus = MAJORITY_CONSENSUS_OF_ALL_MEMBERS;
    const MAX_VOTING_TIME: u64 = DEFAULT_VOTING_TIME;
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct APAOutcastProposal {
    pub apa_proposal: APAProposal,
}

impl Proposal for APAOutcastProposal {
    type Network = ElusivFullWardenGenesisNetwork;
    const PROPONENT_CONSTRAINT: ProponentConstraint = ProponentConstraint::Any;
    const CONSENSUS_REQUIREMENT: ProposalVoteConsensus = SUPERMAJORITY_CONSENSUS_OF_ALL_MEMBERS;
    const MAX_VOTING_TIME: u64 = DEFAULT_VOTING_TIME;
}