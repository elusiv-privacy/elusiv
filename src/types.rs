use super::proof::PROOF_BYTES_SIZE;

pub type U256 = [u8; 32];
pub const U256_ZERO: U256 = [0; 32];

pub type RawProof = [u8; PROOF_BYTES_SIZE];

#[derive(Clone, Copy, PartialEq)]
pub struct ProofData {
    pub amount: u64,
    pub nullifier_hash: U256,
    pub root: U256,
    pub proof: RawProof,
}