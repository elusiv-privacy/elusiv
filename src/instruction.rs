use solana_program::program_error::{
    ProgramError,
};
use super::types::*;
use super::bytes::*;

#[derive(crate::macros::ElusivInstruction)]
pub enum ElusivInstruction {
    Store {
        proof_data: ProofData,
        unbound_commitment: U256,
    },

    Bind {
        proof_data: ProofData,
        unbound_commitment: U256,
        bound_commitment: U256,
    },

    Send {
        proof_data: ProofData,
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