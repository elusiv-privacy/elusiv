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
    reserve::ReserveAccount,
    StorageAccount,
    NullifierAccount,
};
use crate::proof::VerificationAccount;
use crate::commitment::{BaseCommitmentHashingAccount, CommitmentHashingAccount};
use crate::error::ElusivError::InvalidAccount;
use solana_program::system_program;

#[cfg(feature = "instruction-abi")]
use solana_program::instruction::AccountMeta;

#[derive(SerDe, ElusivInstruction)]
pub enum ElusivInstruction {
    // Client sends base commitment and amount to be stored in the Elusiv program
    #[usr(sender, [ writable, signer ])]
    #[pda(pool, Pool, [ writable, account_info ])]
    #[sys(system_program, key = system_program::id())]
    #[pda(queue, BaseCommitmentQueue, [ writable ])]
    Store {
        base_commitment_request: BaseCommitmentHashRequest,
    },

    // Binary send proof request
    #[usr(fee_payer, [ writable, signer ])]
    #[pda(pool, Pool, [ writable, account_info ])]
    #[sys(system_program, key = system_program::id())]
    #[pda(storage_account, Storage, multi_accounts)]
    #[pda(nullifier_account0, Nullifier, pda_offset = proof_request.proof_data.tree_indices[0], [ multi_accounts ])]
    #[pda(nullifier_account1, Nullifier, pda_offset = proof_request.proof_data.tree_indices[1], [ multi_accounts ])]
    #[pda(queue, SendProofQueue, [ writable ])]
    Send {
        proof_request: SendProofRequest,
    },

    // Binary merge proof request
    #[usr(fee_payer, [ writable, signer ])]
    #[pda(pool, Pool, [ writable, account_info ])]
    #[sys(system_program, key = system_program::id())]
    #[pda(storage_account, Storage, multi_accounts)]
    #[pda(nullifier_account0, Nullifier, pda_offset = proof_request.proof_data.tree_indices[0], [ multi_accounts ])]
    #[pda(nullifier_account1, Nullifier, pda_offset = proof_request.proof_data.tree_indices[1], [ multi_accounts ])]
    #[pda(queue, MergeProofQueue, [ writable ])]
    Merge {
        proof_request: MergeProofRequest,
    },

    // Unary migrate proof request
    #[usr(fee_payer, [ writable, signer ])]
    #[pda(pool, Pool, [ writable, account_info ])]
    #[sys(system_program, key = system_program::id())]
    #[pda(storage_account, Storage, multi_accounts)]
    #[pda(nullifier_account0, Nullifier, pda_offset = proof_request.proof_data.tree_indices[0], [ multi_accounts ])]
    #[pda(queue, MigrateProofQueue, [ writable ])]
    Migrate {
        proof_request: MigrateProofRequest,
    },

    // Funds are transferred to the recipient
    #[usr(recipient, [ writable ])]
    #[pda(pool, Pool, [ writable, account_info ])]
    #[pda(queue, FinalizeSendQueue, [ writable ])]
    FinalizeSend,

    // Proof initialization
    #[pda(queue, SendProofQueue, [ writable ])]
    #[pda(verification_account, Verification, pda_offset = verification_account_index, [ writable ])]
    InitSendProof { verification_account_index: u64 },

    #[pda(queue, MergeProofQueue, [ writable ])]
    #[pda(verification_account, Verification, pda_offset = verification_account_index, [ writable ])]
    InitMergeProof { verification_account_index: u64 },

    #[pda(queue, MigrateProofQueue, [ writable ])]
    #[pda(verification_account, Verification, pda_offset = verification_account_index, [ writable ])]
    InitMigrateProof { verification_account_index: u64 },

    // Proof verification computation
    #[pda(verification_account, Verification, pda_offset = verification_account_index, [ writable ])]
    ComputeProof { verification_account_index: u64 },

    // Finalizing successfully verified proofs of arity 2
    #[usr(original_fee_payer, [ writable ])]
    #[pda(pool, Pool, [ writable, account_info ])]
    #[pda(verification_account, Verification, pda_offset = verification_account_index, [ writable ])]
    #[pda(commitment_hash_queue, CommitmentQueue, [ writable ])]
    #[pda(finalize_send_queue, FinalizeSendQueue, [ writable ])]
    #[pda(nullifier_account0, Nullifier, pda_offset = tree_indices[0], [ writable, multi_accounts ])]
    #[pda(nullifier_account1, Nullifier, pda_offset = tree_indices[1], [ writable, multi_accounts ])]
    FinalizeProofBinary {
        verification_account_index: u64,
        tree_indices: [u64; 2],
    },

    // Finalizing successfully verified proofs of arity 1
    #[usr(original_fee_payer, [ writable ])]
    #[pda(pool, Pool, [ writable, account_info ])]
    #[pda(verification_account, Verification, pda_offset = verification_account_index, [ writable ])]
    #[pda(commitment_hash_queue, CommitmentQueue, [ writable ])]
    #[pda(nullifier_account, Nullifier, pda_offset = tree_index, [ writable, multi_accounts ])]
    FinalizeProofUnary {
        verification_account_index: u64,
        tree_index: u64,
    },

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

    // Opens all accounts that only have single instances
    #[usr(payer, [ writable, signer ])]
    #[pda(pool, Pool, [ writable, account_info ])]
    #[pda(reserve, Reserve, [ writable, account_info ])]
    #[pda(commitment_queue, CommitmentQueue, [ writable, account_info ])]
    #[pda(base_commitment_queue, BaseCommitmentQueue, [ writable, account_info ])]
    #[pda(send_queue, SendProofQueue, [ writable, account_info ])]
    #[pda(merge_queue, MergeProofQueue, [ writable, account_info ])]
    #[pda(migrate_queue, MigrateProofQueue, [ writable, account_info ])]
    #[pda(storage_account, Storage, [ writable, multi_accounts, account_info ])]
    #[pda(commitment_hash_account, CommitmentHashing, [ writable, account_info ])]
    #[sys(system_program, key = system_program::id())]
    OpenUniqueAccounts,

    // Opens a new `BaseCommitmentHashAccount` if there not enough yet with the reserve as payer
    #[pda(reserve, Reserve, [ writable, account_info ])]
    #[pda(hash_account, BaseCommitmentHashing, pda_offset = base_commitment_hash_account_index, [ writable, account_info ])]
    #[sys(system_program, key = system_program::id())]
    OpenBaseCommitmentHashAccount {
        base_commitment_hash_account_index: u64,
    },

    // Opens a new `VerificationAccount` if there not enough yet with the reserve as payer
    #[pda(reserve, Reserve, [ writable, account_info ])]
    #[pda(verification_account, Verification, pda_offset = verification_account_index, [ writable, account_info ])]
    #[sys(system_program, key = system_program::id())]
    OpenProofVerificationAccount {
        verification_account_index: u64,
    },

    TestFail,
}