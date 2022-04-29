//! We differentiate between unverified and verified commitments
//! - unverified: client asserts that hash(core, amount) = commitment and we verify this by performing the hash
//! - verified: commitment submitted by client has either been hashed by the program or is verified using a zk proof verification

use crate::{types::U256, bytes::{serialize_u256, unpack_u256, unpack_u64}};

/// Unverified commitment with its two preimages, supplied by the client
#[derive(Copy, Clone)]
pub struct UnverifiedCommitment {
    commitment_core: U256,
    amount: u64,
    commitment: U256,
}

impl UnverifiedCommitment {
    pub const SIZE: usize = 32 + 8 + 32;
}

pub fn serialize_unverified_commitment(value: UnverifiedCommitment) -> Vec<u8> {
    let mut buffer = Vec::new();

    buffer.extend(serialize_u256(value.commitment_core));
    buffer.extend(value.amount.to_le_bytes());
    buffer.extend(serialize_u256(value.commitment));

    buffer
}

pub fn deserialize_unverified_commitment(data: &[u8]) -> UnverifiedCommitment {
    let (commitment_core, data) = unpack_u256(data).unpack();
    let (amount, data) = unpack_u64(data).unpack();
    let (commitment, _) = unpack_u256(data).unpack();

    UnverifiedCommitment { commitment_core, amount, commitment }
}