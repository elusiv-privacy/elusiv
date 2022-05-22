use super::bytes::SerDe;
use crate::macros::*;
use crate::state::queue::ProofRequest;
use crate::types::ProofKind;
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
use solana_program::{
    system_program,
    account_info::AccountInfo,
    pubkey::Pubkey,
    entrypoint::ProgramResult,
    program_error::ProgramError::{InvalidArgument, InvalidInstructionData},
};
use crate::error::ElusivError;

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

    // Proof request (Send, Merge, Migrate (since Migrate is unary, only first nullifier is used))
    #[usr(fee_payer, [ writable, signer ])]
    #[pda(pool, Pool, [ writable, account_info ])]
    #[sys(system_program, key = system_program::id())]
    #[pda(storage_account, Storage, multi_accounts)]
    #[pda(nullifier_account0, Nullifier, pda_offset = tree_indices[0], [ multi_accounts ])]
    #[pda(nullifier_account1, Nullifier, pda_offset = tree_indices[1], [ multi_accounts ])]
    #[prg(queue, [ writable, account_info ])] // Parsing of the queue happens in the processor
    RequestProofVerification {
        proof_request: ProofRequest,
        tree_indices: [u64; 2],
    },

    // Proof verification initialization
    #[prg(queue, [ writable, account_info ])]
    #[pda(verification_account, Verification, pda_offset = verification_account_index, [ writable ])]
    InitProof {
        kind: ProofKind,
        verification_account_index: u64
    },

    // Proof verification computation
    #[pda(verification_account, Verification, pda_offset = verification_account_index, [ writable ])]
    ComputeProof {
        verification_account_index: u64
    },

    // Finalizing successfully verified proofs
    #[usr(original_fee_payer, [ writable ])]
    #[pda(pool, Pool, [ writable, account_info ])]
    #[pda(verification_account, Verification, pda_offset = verification_account_index, [ writable ])]
    #[pda(commitment_hash_queue, CommitmentQueue, [ writable ])]
    #[pda(finalize_send_queue, FinalizeSendQueue, [ writable ])]
    #[pda(nullifier_account0, Nullifier, pda_offset = tree_indices[0], [ writable, multi_accounts ])]
    #[pda(nullifier_account1, Nullifier, pda_offset = tree_indices[1], [ writable, multi_accounts ])]
    FinalizeProof {
        verification_account_index: u64,
        tree_indices: [u64; 2],
    },

    // Base-commitment hashing
    #[usr(fee_payer, [ signer, writable ])]
    #[pda(queue, BaseCommitmentQueue, [ writable ])]
    #[pda(hashing_account, BaseCommitmentHashing, pda_offset = base_commitment_hash_account_index, [ writable ])]
    InitBaseCommitmentHash{ base_commitment_hash_account_index: u64, },
    
    #[pda(hashing_account, BaseCommitmentHashing, pda_offset = base_commitment_hash_account_index, [ writable ])]
    ComputeBaseCommitmentHash { base_commitment_hash_account_index: u64, },

    #[pda(hashing_account, BaseCommitmentHashing, pda_offset = base_commitment_hash_account_index, [ writable ])]
    #[pda(commitment_queue, CommitmentQueue, [ writable ])]
    FinalizeBaseCommitmentHash { base_commitment_hash_account_index: u64, },
/*
    // Commitment (MT-root) hashing
    #[usr(fee_payer, [ signer, writable ])]
    #[pda(queue, CommitmentQueue, [ writable ])]
    #[pda(hashing_account, CommitmentHashing, [ writable ])]
    InitCommitmentHash,
    
    #[pda(hashing_account, CommitmentHashing, [ writable ])]
    #[pda(storage_account, Storage, [ multi_accounts ])]
    ComputeCommitmentHash,

    #[pda(hashing_account, CommitmentHashing, [ writable ])]
    #[pda(storage_account, Storage, [ multi_accounts, writable ])]
    FinalizeCommitmentHash,*/

    // Funds are transferred to the recipient
    #[usr(recipient, [ writable ])]
    #[pda(pool, Pool, [ writable, account_info ])]
    #[pda(queue, FinalizeSendQueue, [ writable ])]
    FinalizeSend,
/*
    CreateNewTree,
    ActivateTree,
    ArchiveTree,

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
    OpenUniqueAccount,
    */

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