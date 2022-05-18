use super::types::*;
use super::processor::*;
use super::state::queue::{ BaseCommitmentHashRequest, SendProofRequest, MergeProofRequest, MigrateProofRequest, }

#[derive(crate::macros::ElusivInstruction)]
pub enum ElusivInstruction {
    // Client sends base commitment and amount to be stored in the Elusiv program
    #[sig(sender)]
    #[prg(storage_account, Storage)]
    #[prg(queue, BaseCommitmentQueue)]
    #[pdi(pool, Pool)]
    #[arr(tree, ActiveTree, pda_offset = storage_account.get_active_tree())]
    #[sys(system_program, key = solana_program::system_program::id())]
    Store {
        base_commitment_request: BaseCommitmentHashRequest,
    },

    // Binary send proof
    Send {
        proof_request: SendProofRequest,
        timestamp: u64,
    },

    // Binary merge proof
    Merge {
        proof_request: MergeProofRequest,
    },
    
    // Unary migrate proof
    Migrate {
        proof_request: MigrateProofRequest,
    },

    // Funds are transferred to the recipient
    FinalizeSend,

    InitProof,
    ComputeProof,
    FinalizeProof,

    InitCommitment,
    ComputeCommitment,
    FinalizeCommitment,

    // Instructions used for accounts management
    // Resets the main MT and creates a new active TreeAccount PDA
    ActivateTree,
    ArchiveTree,
}