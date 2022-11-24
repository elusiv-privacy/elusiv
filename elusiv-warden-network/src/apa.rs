use elusiv_proc_macros::elusiv_account;
use solana_program::pubkey::Pubkey;
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_types::{BorshSerDeSized, BorshSerDeSizedEnum, PDAAccountData};
use crate::error::ElusivWardenNetworkError;
use crate::network::{ElusivFullWardenNetwork, WardenNetwork};
use crate::proposal::{
    Proposal,
    ProposalAccount,
    SUPERMAJORITY_CONSENSUS_OF_ALL_MEMBERS,
    MAJORITY_CONSENSUS_OF_ALL_MEMBERS,
    DEFAULT_VOTING_TIME,
    proposal_account, ProposalVotingAccount, VotesCount, Vote,
};
use crate::macros::{guard, BorshSerDeSized};
use crate::warden::ElusivWardenID;

pub const APA_KEY_VSSS_SHARES: usize = 1;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct APAConfig {
    pub apa_keys: [[u8; 32]; APA_KEY_VSSS_SHARES],
    pub apae_signature: [u8; 32],
}

#[elusiv_account]
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

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct APAReason {
    code: u32,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct APAProposal {
    pub key: Pubkey,
    pub reason: APAReason,
    pub kind: APAProposalKind,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub enum APAProposalKind {
    Flagging,
    Outcast,
}

proposal_account! {
    APAProposal,
    APAProposalAccount,
    b"apa_proposal",
    ElusivFullWardenNetwork
}

impl Proposal for APAProposal {
    type Network = ElusivFullWardenNetwork;

    const VOTING_WINDOW: u64 = DEFAULT_VOTING_TIME;

    fn is_proponent_valid(&self, _warden_id: Option<crate::warden::ElusivWardenID>) -> bool {
        true
    }
}

impl<'a> ProposalVotingAccount for APAProposalAccount<'a> {
    fn is_consensus_reached(&self) -> bool {
        let proposal = self.proposal();
        let VotesCount { accept, reject } = self.get_votes_count();
        let consensus = match proposal.kind {
            APAProposalKind::Flagging => MAJORITY_CONSENSUS_OF_ALL_MEMBERS,
            APAProposalKind::Outcast => SUPERMAJORITY_CONSENSUS_OF_ALL_MEMBERS,
        };
        consensus.consensus(accept, accept + reject, ElusivFullWardenNetwork::SIZE.members_count() as u32)
    }
}

#[elusiv_account]
pub struct APAAccount {
    pda_data: PDAAccountData,

    flagged: APATree,
    outcast: APATree,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct APATree {
    count: u32,
    root: [u8; 32],
    signature: [u8; 32],
}

#[elusiv_account]
pub struct APAFlaggedAccount {
    pda_data: PDAAccountData,
    key: Pubkey,
}

#[elusiv_account]
pub struct APAOutcastAccount {
    pda_data: PDAAccountData,
    key: Pubkey,
}