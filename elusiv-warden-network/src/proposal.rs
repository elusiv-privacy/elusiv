use crate::{network::WardenNetwork, warden::ElusivWardenID};
use std::collections::BTreeMap;
use borsh::{BorshDeserialize, BorshSerialize};

/// A proposal, all members of the associated network can vote on
pub trait Proposal {
    type Network: WardenNetwork;
    const PROPONENT_CONSTRAINT: ProponentConstraint;
    const CONSENSUS_REQUIREMENT: ProposalVoteConsensus;
    const MAX_VOTING_TIME: u64;
}

pub enum ProponentConstraint {
    Any,
    NetworkMember,
}

pub const DEFAULT_VOTING_TIME: u64 = 0;

pub struct ProposalVote {
    /// Maps each network member to a vote index
    pub network_members: BTreeMap<ElusivWardenID, u32>,
    pub votes: Vec<Vote>,
    pub timestamp: u64,
}

pub struct Percentage(u8);

pub struct ProposalVoteConsensus {
    /// Required majority for approval (against the number of participants)
    pub qualified_majority: Percentage,

    /// Required percentage of members that need to participate
    /// - Note: at least one vote is required in total
    pub required_participation: Percentage,
}

/// Consensus requires a 2/3 supermajority of all network members
pub const SUPERMAJORITY_CONSENSUS_OF_ALL_MEMBERS: ProposalVoteConsensus = ProposalVoteConsensus {
    qualified_majority: Percentage(66),
    required_participation: Percentage(100),
};

pub const MAJORITY_CONSENSUS_OF_ALL_MEMBERS: ProposalVoteConsensus = ProposalVoteConsensus {
    qualified_majority: Percentage(50),
    required_participation: Percentage(100),
};

#[derive(BorshDeserialize, BorshSerialize, Clone)]
pub enum Vote {
    None,
    Accept,
    Reject,
}