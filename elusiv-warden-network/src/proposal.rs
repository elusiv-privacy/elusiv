use std::ops::Mul;

use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_types::{BorshSerDeSized, BorshSerDeSizedEnum, PDAAccount, SizedAccount};
use crate::error::ElusivWardenNetworkError;
use crate::macros::BorshSerDeSized;
use crate::{network::WardenNetwork, warden::ElusivWardenID};

macro_rules! proposal_account {
    ($ty: ident, $ty_account: ident, $ty_seed: expr, $n: ident) => {
        #[elusiv_account(pda_seed = $ty_seed)]
        pub struct $ty_account {
            pda_data: PDAAccountData,

            proposal: $ty,
            timestamp: u64,

            // The voting body (the network member composition at the time of proposal)
            body: [u32; $n::SIZE.members_count()],

            // Votes for each member of the body
            votes: [crate::proposal::Vote; $n::SIZE.members_count()],
            votes_count: crate::proposal::VotesCount,
        }

        impl ProposalAccount for $ty_account<'_> {
            type P = $ty;

            fn init(&mut self, proposal: Self::P, timestamp: u64, network_members_slice: &[u8]) {
                self.set_proposal(&proposal);
                self.set_timestamp(&timestamp);
                self.body.copy_from_slice(&network_members_slice[..self.body.len()]);
            }

            fn try_vote(
                &mut self,
                vote: Vote,
                warden_id: ElusivWardenID,
                current_timestamp: u64,
            ) -> Result<(), ElusivWardenNetworkError> {
                guard!(self.is_voting_window(current_timestamp), ElusivWardenNetworkError::VotingError);
        
                let voter_index = self.voter_index(warden_id)
                    .ok_or(ElusivWardenNetworkError::VotingError)?;
        
                match self.get_votes(voter_index) {
                    Vote::None => {
                        self.set_votes(voter_index, &vote);
                        Ok(())
                    }
                    _ => {
                        Err(ElusivWardenNetworkError::VotingError)
                    }
                }
            }

            fn proposal(&self) -> Self::P {
                self.get_proposal()
            }

            fn timestamp(&self) -> u64 {
                self.get_timestamp()
            }
        }

        impl $ty_account<'_> {
            pub fn voter_index(&self, warden_id: ElusivWardenID) -> Option<usize> {
                for i in 0..$n::SIZE.members_count() {
                    if self.get_body(i) == warden_id {
                        return Some(i)
                    }
                }
                None
            }
        }
    };
}

pub(crate) use proposal_account;

/// A proposal, all members of the associated network can vote on
pub trait Proposal {
    type Network: WardenNetwork;

    const VOTING_WINDOW: u64 = DEFAULT_VOTING_TIME;

    fn is_proponent_valid(&self, warden_id: Option<ElusivWardenID>) -> bool;
}

pub trait ProposalAccount: PDAAccount + SizedAccount {
    type P: Proposal;

    fn init(&mut self, proposal: Self::P, timestamp: u64, network_members_slice: &[u8]);
    fn try_vote(&mut self, vote: Vote, warden_id: ElusivWardenID, current_timestamp: u64) -> Result<(), ElusivWardenNetworkError>;

    fn proposal(&self) -> Self::P;
    fn timestamp(&self) -> u64;
}

pub trait ProposalVotingAccount: ProposalAccount {
    fn is_consensus_reached(&self) -> bool;
    fn is_voting_window(&self, current_timestamp: u64) -> bool {
        current_timestamp <= self.timestamp() + Self::P::VOTING_WINDOW
    }
}

pub const DEFAULT_VOTING_TIME: u64 = 0;

#[derive(Clone, Copy)]
pub struct Fraction(u32, u32);

impl Mul for Fraction {
    type Output = Option<Self>;

    fn mul(self, rhs: Self) -> Self::Output {
        Some(
            Fraction(
                self.0.checked_mul(rhs.0)?,
                self.1.checked_mul(rhs.1)?,
            )
        )
    }
}

impl PartialEq for Fraction {
    fn eq(&self, other: &Self) -> bool {
        let (a, b) = match self.enforce_equal_denominator_pair(other) {
            Some((a, b)) => (a, b),
            None => return false
        };

        a.0.eq(&b.0)
    }
}

impl PartialOrd for Fraction {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.enforce_equal_denominator(other)?.0
            .partial_cmp(&other.enforce_equal_denominator(self)?.0)
    }
}

impl Fraction {
    fn scale(&self, p: u32) -> Option<Self> {
        self.mul(Fraction(p, p))
    }

    fn enforce_equal_denominator(&self, other: &Self) -> Option<Self> {
        if self.1 == other.1 {
            Some(*self)
        } else {
            self.scale(other.1)
        }
    }

    fn enforce_equal_denominator_pair(&self, other: &Self) -> Option<(Self, Self)> {
        Some(
            (
                self.enforce_equal_denominator(other)?,
                other.enforce_equal_denominator(self)?,
            )
        )
    }
}

pub struct ProposalVoteConsensus {
    /// Required majority for approval (against the number of participants)
    pub qualified_majority: Fraction,

    /// Required percentage of members that need to participate
    pub required_participation: Fraction,

    pub majority_against_all_members: bool,
}

impl ProposalVoteConsensus {
    pub fn consensus(&self, accept: u32, total_votes: u32, members: u32) -> bool {
        self.required_participation <= Fraction(total_votes, members)
        &&
        self.qualified_majority <= Fraction(
            accept,
            if self.majority_against_all_members { members } else { total_votes },
        )
    }
}

/// Consensus requires a 2/3 supermajority of all network members
pub const SUPERMAJORITY_CONSENSUS_OF_ALL_MEMBERS: ProposalVoteConsensus = ProposalVoteConsensus {
    qualified_majority: Fraction(2, 3),
    required_participation: Fraction(0, 1),
    majority_against_all_members: true,
};

pub const MAJORITY_CONSENSUS_OF_ALL_MEMBERS: ProposalVoteConsensus = ProposalVoteConsensus {
    qualified_majority: Fraction(1, 2),
    required_participation: Fraction(0, 1),
    majority_against_all_members: true,
};

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Clone)]
pub enum Vote {
    None,
    Accept,
    Reject,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct VotesCount {
    pub accept: u32,
    pub reject: u32,
}