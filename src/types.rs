use crate::proof::PROOF_BYTES_SIZE;
use borsh::{ BorshSerialize, BorshDeserialize };

pub type U256 = [u8; 32];
pub const U256_ZERO: U256 = [0; 32];

pub type RawProof = [u8; PROOF_BYTES_SIZE];

/// Minimum data and public inputs required for a n-ary join-split based proof
#[derive(BorshSerialize, BorshDeserialize)]
pub struct JoinSplitProofData<const N: usize> {
    pub proof: RawProof,
    pub nullifierHashes: [U256; N],
    pub rootHashes: [U256; N],
    pub commitment: U256,
}

impl<const N: usize> JoinSplitProofData<N> {
    pub const SIZE: usize = PROOF_BYTES_SIZE + 3 * (32 * N) + 32;
}