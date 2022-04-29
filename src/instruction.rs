use solana_program::program_error::{
    ProgramError,
};
use super::types::*;
use super::bytes::*;

#[derive(crate::macros::ElusivInstruction)]
pub enum ElusivInstruction {
    Store {
        base_commitment: U256,  // h(nullifier, timestamp)
        amount: u64,
        commitment: U256,
    },

    Merge {
        proof_data: ProofDataBinary,
    },

    Send {
        proof_data: ProofDataBinary,
        amount: u64,
        recipient: U256,
    },

    FinalizeSend,

    InitProof,
    ComputeProof,
    FinalizeProof,

    InitCommitment,
    ComputeCommitment,
    FinalizeCommitment,

    InitStorage {
        bump_seed: u8,
    },
    ArchiveStorage,
}