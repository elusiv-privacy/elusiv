use super::proof::PROOF_BYTES_SIZE;
use borsh::{ BorshSerialize, BorshDeserialize };

pub type U256 = [u8; 32];
pub const U256_ZERO: U256 = [0; 32];

pub type RawProof = [u8; PROOF_BYTES_SIZE];

#[derive(BorshSerialize, BorshDeserialize)]
struct ProofDataBinary {
    pub proof: RawProof,
    pub nullifiers: [U256; 2],
    pub roots: [U256; 2],
    pub commitment: U256,
}