use super::bytes::SerDe;
use crate::macros::*;
use super::processor::*;
use super::state::queue::{
    BaseCommitmentQueueAccount,
    CommitmentQueueAccount,
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
pub enum ElusivInstruction {
    // Client sends base commitment and amount to be stored in the Elusiv program
    #[usr(sender, [ writable, signer ])]
    #[pda(pool, Pool, [ writable, account_info ])]
    #[sys(system_program, key = solana_program::system_program::id())]
    #[pda(queue, BaseCommitmentQueue, [ writable ])]
    Store {
        base_commitment_request: BaseCommitmentHashRequest,
    },

    // Binary send proof request
    #[usr(fee_payer, [ writable, signer ])]
    #[pda(pool, Pool, [ writable, account_info ])]
    #[sys(system_program, key = solana_program::system_program::id())]
    #[pda(storage_account, Storage, multi_accounts)]
    #[pda(nullifier_account0, Nullifier, pda_offset = proof_request.proof_data.tree_indices[0], multi_accounts)]
    #[pda(nullifier_account1, Nullifier, pda_offset = proof_request.proof_data.tree_indices[1], multi_accounts)]
    #[pda(queue, SendProofQueue, [ writable ])]
    Send {
        proof_request: SendProofRequest,
    },

    // Binary merge proof request
    #[usr(fee_payer, [ writable, signer ])]
    #[pda(pool, Pool, [ writable, account_info ])]
    #[sys(system_program, key = solana_program::system_program::id())]
    #[pda(storage_account, Storage, multi_accounts)]
    #[pda(nullifier_account0, Nullifier, pda_offset = proof_request.proof_data.tree_indices[0], multi_accounts)]
    #[pda(nullifier_account1, Nullifier, pda_offset = proof_request.proof_data.tree_indices[1], multi_accounts)]
    #[pda(queue, MergeProofQueue, [ writable ])]
    Merge {
        proof_request: MergeProofRequest,
    },

    // Unary migrate proof request
    #[usr(fee_payer, [ writable, signer ])]
    #[pda(pool, Pool, [ writable, account_info ])]
    #[sys(system_program, key = solana_program::system_program::id())]
    #[pda(storage_account, Storage, multi_accounts)]
    #[pda(nullifier_account0, Nullifier, pda_offset = proof_request.proof_data.tree_indices[0], multi_accounts)]
    #[pda(queue, MigrateProofQueue, [ writable ])]
    Migrate {
        proof_request: MigrateProofRequest,
    },

    // Funds are transferred to the recipient
    /*#[usr_inf(recipient)]
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

    // Proof verification computation
    #[pda_mut(verification_account, Verification, pda_offset = verification_account_index)]
    ComputeProof { verification_account_index: u64 },

    // Finalizing successfully verified proofs of arity 2
    #[usr_inf(original_fee_payer)]
    #[pda_inf(pool, Pool)]
    #[pda_mut(verification_account, Verification, pda_offset = verification_account_index)]
    #[pda_mut(commitment_hash_queue, CommitmentQueue)]
    #[pda_mut(finalize_send_queue, FinalizeSendQueue)]
    #[pda_arr(nullifier_account0, Nullifier, pda_offset = tree_indices[0], mut = true)]
    #[pda_arr(nullifier_account1, Nullifier, pda_offset = tree_indices[1], mut = true)]
    FinalizeProofBinary {
        verification_account_index: u64,
        tree_indices: [u64; 2],
    },

    // Finalizing successfully verified proofs of arity 1
    #[usr_inf(original_fee_payer)]
    #[pda_inf(pool, Pool)]
    #[pda_mut(verification_account, Verification, pda_offset = verification_account_index)]
    #[pda_mut(commitment_hash_queue, CommitmentQueue)]
    #[pda_arr(nullifier_account, Nullifier, pda_offset = tree_index, mut = true)]
    FinalizeProofUnary {
        verification_account_index: u64,
        tree_index: u64,
    },*/

    // Commitment hash initialization
    //InitCommitment,
    /*ComputeCommitment,
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