use super::bytes::SerDe;
use crate::macros::*;
use super::processor::*;
use super::state::queue::{
    BaseCommitmentQueueAccount,
    BaseCommitmentHashRequest,
    SendProofQueueAccount, SendProofRequest,
    MergeProofQueueAccount, MergeProofRequest,
    MigrateProofQueueAccount, MigrateProofRequest,
    FinalizeSendQueueAccount,
};
use super::state::{
    program_account::{PDAAccount,MultiAccountAccount},
    pool::PoolAccount,
    StorageAccount,
    NullifierAccount,
};
use crate::proof::VerificationAccount;
use crate::error::ElusivError::InvalidAccount;

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
    #[sig_inf(fee_payer)]
    #[pda_inf(pool, Pool)]
    #[sys_inf(system_program, key = solana_program::system_program::id())]
    #[pda_arr(storage_account, Storage, pda_offset = 0)]
    #[pda_arr(nullifier_account0, Nullifier, pda_offset = proof_request.proof_data.tree_indices[0])]
    #[pda_arr(nullifier_account1, Nullifier, pda_offset = proof_request.proof_data.tree_indices[1])]
    #[pda_mut(queue, SendProofQueue)]
    Send {
        proof_request: SendProofRequest,
    },

    // Binary merge proof request
    #[sig_inf(fee_payer)]
    #[pda_inf(pool, Pool)]
    #[sys_inf(system_program, key = solana_program::system_program::id())]
    #[pda_arr(storage_account, Storage, pda_offset = 0)]
    #[pda_arr(nullifier_account0, Nullifier, pda_offset = proof_request.proof_data.tree_indices[0])]
    #[pda_arr(nullifier_account1, Nullifier, pda_offset = proof_request.proof_data.tree_indices[1])]
    #[pda_mut(queue, MergeProofQueue)]
    Merge {
        proof_request: MergeProofRequest,
    },

    // Unary migrate proof request
    #[sig_inf(fee_payer)]
    #[pda_inf(pool, Pool)]
    #[sys_inf(system_program, key = solana_program::system_program::id())]
    #[pda_arr(storage_account, Storage, pda_offset = 0)]
    #[pda_arr(nullifier_account0, Nullifier, pda_offset = proof_request.proof_data.tree_indices[0])]
    #[pda_mut(queue, MigrateProofQueue)]
    Migrate {
        proof_request: MigrateProofRequest,
    },

    // Binary merge proof request

    // Funds are transferred to the recipient
    #[usr_inf(fee_payer)]
    #[pda_inf(pool, Pool)]
    #[pda_mut(queue, FinalizeSendQueue)]
    FinalizeSend,

    // Proof initialization
    #[pda_mut(queue, SendProofQueue)]
    #[pda_mut(verification_account, Verification, pda_offset = verification_account_index)]
    InitSendProof { verification_account_index: u64 },

    #[pda_mut(queue, MergeProofQueue)]
    #[pda_mut(verification_account, Verification, pda_offset = verification_account_index)]
    InitMergeProof { verification_account_index: u64 },

    #[pda_mut(queue, MigrateProofQueue)]
    #[pda_mut(verification_account, Verification, pda_offset = verification_account_index)]
    InitMigrateProof { verification_account_index: u64 },

    //ComputeProof,
    //FinalizeProof,

    /*InitCommitment,
    ComputeCommitment,
    FinalizeCommitment,

    // Creates a new `NullifierAccount`
    CreateNewTree,

    // Resets the main MT
    ActivateTree,

    // Closes the oldest `NullifierAccount` and creates a `ArchivedTreeAccount`
    ArchiveTree,*/

    /*OpenUniqueAccounts,

    OpenProofVerificationAccount,    
    OpenBaseCommitmentHashAccount,*/
}