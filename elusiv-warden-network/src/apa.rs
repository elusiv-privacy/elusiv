use elusiv_proc_macros::elusiv_account;
use solana_program::pubkey::Pubkey;
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_types::{BorshSerDeSized, PDAAccountData, SizedAccount};
use crate::network::ElusivFullWardenNetwork;
use crate::proposal::{Proposal, ProponentConstraint, ProposalVoteConsensus, SUPERMAJORITY_CONSENSUS_OF_ALL_MEMBERS, MAJORITY_CONSENSUS_OF_ALL_MEMBERS, DEFAULT_VOTING_TIME};
use crate::macros::BorshSerDeSized;

pub const APA_KEY_VSSS_SHARES: usize = 1;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct APAConfig {
    pub apa_keys: [[u8; 32]; APA_KEY_VSSS_SHARES],
    pub apae_signature: [u8; 32],
}

#[elusiv_account(pda_seed = b"apa_config_account")]
pub struct APAGenesisConfigAccount {
    pda_data: PDAAccountData,
    apa_config: APAConfig,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct RemoteAttestationCert {
    data: [u8; 1],
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct APAECert {
    pub warden_key: [u8; 32],
    pub apae_key: [u8; 32],
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
    type Network = ElusivFullWardenNetwork;
    const PROPONENT_CONSTRAINT: ProponentConstraint = ProponentConstraint::Any;
    const CONSENSUS_REQUIREMENT: ProposalVoteConsensus = MAJORITY_CONSENSUS_OF_ALL_MEMBERS;
    const MAX_VOTING_TIME: u64 = DEFAULT_VOTING_TIME;
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct APAOutcastProposal {
    pub apa_proposal: APAProposal,
}

impl Proposal for APAOutcastProposal {
    type Network = ElusivFullWardenNetwork;
    const PROPONENT_CONSTRAINT: ProponentConstraint = ProponentConstraint::Any;
    const CONSENSUS_REQUIREMENT: ProposalVoteConsensus = SUPERMAJORITY_CONSENSUS_OF_ALL_MEMBERS;
    const MAX_VOTING_TIME: u64 = DEFAULT_VOTING_TIME;
}