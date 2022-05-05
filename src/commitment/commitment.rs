//! A commitment is defined as h(base_commitment, amount) with base_commitment = h(nullifier, timestamp)

use crate::{ types::U256, bytes::{serialize_u256, unpack_u256, unpack_u64} };
use borsh::{ BorshSerialize, BorshDeserialize };

/// Data for hashing a commitment with transparent amount on-chain
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BaseCommitmentHashRequest {
    base_commitment: U256,
    amount: u64,
    commitment: U256,   // commitment is stored in order to be able to quickly check for duplicates upfront
}

impl BaseCommitmentHashRequest {
    pub const SIZE: usize = 32 + 8 + 32;
}
