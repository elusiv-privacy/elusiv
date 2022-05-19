use super::bytes::SerDe;
use crate::macros::*;
use super::processor::*;
use super::state::queue::{
    BaseCommitmentQueueAccount,
    BaseCommitmentHashRequest,
    SendProofQueueAccount, SendProofRequest,
    MergeProofQueueAccount, MergeProofRequest,
    MigrateProofQueueAccount, MigrateProofRequest,
};
use super::state::{
    program_account::{PDAAccount,MultiAccountAccount},
    pool::PoolAccount,
    StorageAccount,
    NullifierAccount,
};
use crate::error::ElusivError::{InvalidAccount};

#[derive(SerDe, ElusivInstruction)]
//#[derive(SerDe)]
pub enum ElusivInstruction {
    // Client sends base commitment and amount to be stored in the Elusiv program
    #[sig_inf(sender)]
    #[pda_inf(pool, Pool)]
    #[sys_inf(system_program, key = solana_program::system_program::id())]
    #[pda_mut(queue, BaseCommitmentQueue)]
    Store {
        base_commitment_request: BaseCommitmentHashRequest,
    },

    // Binary send proof request
    #[sig_inf(relayer)]
    #[pda_inf(pool, Pool)]
    #[sys_inf(system_program, key = solana_program::system_program::id())]
    #[pda_arr(storage_account, Storage, pda_offset = 0)]
    #[pda_arr(nullifier_account0, Nullifier, pda_offset = 0)]
    #[pda_arr(nullifier_account1, Nullifier, pda_offset = 0)]
    #[pda_mut(queue, SendProofQueue)]
    Send {
        proof_request: SendProofRequest,
    },

    // Binary merge proof request
    #[sig_inf(relayer)]
    #[pda_inf(pool, Pool)]
    #[sys_inf(system_program, key = solana_program::system_program::id())]
    #[pda_arr(storage_account, Storage, pda_offset = 0)]
    #[pda_arr(nullifier_account0, Nullifier, pda_offset = 0)]
    #[pda_arr(nullifier_account1, Nullifier, pda_offset = 0)]
    #[pda_mut(queue, MergeProofQueue)]
    Merge {
        proof_request: MergeProofRequest,
    },

    // Unary migrate proof request
    #[sig_inf(relayer)]
    #[pda_inf(pool, Pool)]
    #[sys_inf(system_program, key = solana_program::system_program::id())]
    #[pda_arr(storage_account, Storage, pda_offset = 0)]
    #[pda_arr(nullifier_account, Nullifier, pda_offset = 0)]
    #[pda_mut(queue, MigrateProofQueue)]
    Migrate {
        proof_request: MigrateProofRequest,
    },

    // Binary merge proof request
    /*Merge {
        proof_request: MergeProofRequest,
    },
    
    // Unary migrate proof request
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

    // Creates a new `NullifierAccount`
    CreateNewTree,

    // Resets the main MT
    ActivateTree,

    // Closes the oldest `NullifierAccount` and creates a `ArchivedTreeAccount`
    ArchiveTree,*/

}