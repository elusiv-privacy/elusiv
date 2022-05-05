use crate::proof::PROOF_BYTES_SIZE;
use crate::bytes::SerDe;
use crate::macros::*;

pub type U256 = [u8; 32];
pub const U256_ZERO: U256 = [0; 32];

pub type RawProof = [u8; PROOF_BYTES_SIZE];

/// Minimum data and public inputs required for a n-ary join-split based proof
#[derive(SerDe)]
pub struct JoinSplitProofData<const N: usize> {
    pub proof: RawProof,
    pub nullifierHashes: [U256; N],
    pub rootHashes: [U256; N],
    pub treeIndices: [u64; N],
    pub commitment: U256,
}