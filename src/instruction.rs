use crate::macros::*;
use crate::state::queue::ProofRequest;
use crate::types::ProofKind;
use crate::bytes::BorshSerDeSized;
use super::processor::*;
use super::state::queue::{
    QueueManagementAccount,
    BaseCommitmentQueueAccount,
    CommitmentQueueAccount,
    BaseCommitmentHashRequest,
    FinalizeSendQueueAccount,
};
use super::state::{
    program_account::{PDAAccount,MultiAccountAccount},
    pool::PoolAccount,
    StorageAccount,
    NullifierAccount,
};
use crate::proof::VerificationAccount;
use crate::commitment::{BaseCommitmentHashingAccount, CommitmentHashingAccount};
use solana_program::{
    system_program,
    account_info::{next_account_info, next_account_infos, AccountInfo},
    pubkey::Pubkey,
    entrypoint::ProgramResult,
    program_error::ProgramError::{InvalidArgument, InvalidInstructionData},
};
use borsh::{BorshDeserialize, BorshSerialize};

#[cfg(feature = "instruction-abi")]
use solana_program::instruction::AccountMeta;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, ElusivInstruction)]
pub enum ElusivInstruction {
    // Client sends base commitment and amount to be stored in the Elusiv program
    /*#[usr(sender, { writable, signer })]
    #[pda(pool, Pool, { writable, account_info })]
    #[sys(system_program, key = system_program::ID)]
    //#[pda(q_manager, QueueManagement, { ignore })]
    #[prg(queue, BaseCommitmentQueue, key = q_manager.get_base_commitment_queue(), { writable })]
    Store {
        base_commitment_request: BaseCommitmentHashRequest,
    },*/

    // Proof request (Send, Merge, Migrate (since Migrate is unary, only first nullifier is used))
    /*#[usr(fee_payer, ( writable, signer ))]
    #[pda(pool, Pool, ( writable, account_info ))]
    #[sys(system_program, key = system_program::ID)]
    #[pda(storage_account, Storage, ( multi_accounts ))]
    #[pda(nullifier_account0, Nullifier, pda_offset = tree_indices[0], ( multi_accounts ))]
    #[pda(nullifier_account1, Nullifier, pda_offset = tree_indices[1], ( multi_accounts ))]
    #[prg(queue, ( writable, account_info ))] // Parsing of the queue happens in the processor
    RequestProofVerification {
        proof_request: ProofRequest,
        tree_indices: [u64; 2],
    },

    // Proof verification initialization
    #[prg(queue, ( writable, account_info ))]
    #[pda(verification_account, Verification, pda_offset = verification_account_index, ( writable ))]
    InitProof {
        kind: ProofKind,
        verification_account_index: u64
    },

    // Proof verification computation
    #[pda(verification_account, Verification, pda_offset = verification_account_index, ( writable ))]
    ComputeProof {
        verification_account_index: u64
    },

    // Finalizing successfully verified proofs
    #[usr(original_fee_payer, ( writable ))]
    #[pda(pool, Pool, ( writable, account_info ))]
    #[pda(verification_account, Verification, pda_offset = verification_account_index, ( writable ))]
    #[pda(commitment_hash_queue, CommitmentQueue, ( writable ))]
    #[pda(finalize_send_queue, FinalizeSendQueue, ( writable ))]
    #[pda(nullifier_account0, Nullifier, pda_offset = tree_indices[0], ( writable, multi_accounts ))]
    #[pda(nullifier_account1, Nullifier, pda_offset = tree_indices[1], ( writable, multi_accounts ))]
    FinalizeProof {
        verification_account_index: u64,
        tree_indices: [u64; 2],
    },

    // Base-commitment hashing
    #[usr(fee_payer, ( signer, writable ))]
    #[pda(queue, BaseCommitmentQueue, ( writable ))]
    #[pda(hashing_account, BaseCommitmentHashing, pda_offset = hash_account_index, ( writable ))]
    InitBaseCommitmentHash{
        hash_account_index: u64,
    },
    
    #[pda(hashing_account, BaseCommitmentHashing, pda_offset = hash_account_index, ( writable ))]
    ComputeBaseCommitmentHash {
        hash_account_index: u64,
    },

    #[pda(hashing_account, BaseCommitmentHashing, pda_offset = hash_account_index, ( writable ))]
    #[pda(commitment_queue, CommitmentQueue, ( writable ))]
    FinalizeBaseCommitmentHash {
        hash_account_index: u64,
    },

    // Commitment (MT-root) hashing
    #[usr(fee_payer, ( signer, writable ))]
    #[pda(queue, CommitmentQueue, ( writable ))]
    #[pda(hashing_account, CommitmentHashing, ( writable ))]
    InitCommitmentHash,
    
    #[pda(hashing_account, CommitmentHashing, ( writable ))]
    #[pda(storage_account, Storage, ( multi_accounts ))]
    ComputeCommitmentHash,

    #[pda(hashing_account, CommitmentHashing, ( writable ))]
    #[pda(storage_account, Storage, ( multi_accounts, writable ))]
    FinalizeCommitmentHash,

    // Funds are transferred to the recipient
    #[usr(recipient, ( writable ))]
    #[pda(pool, Pool, ( writable, account_info ))]
    #[pda(queue, FinalizeSendQueue, ( writable ))]
    FinalizeSend,*/

    /*
    CreateNewTree,
    ActivateTree,
    ArchiveTree,
    */

    #[usr(payer, { writable, signer })]
    #[usr(pda_account, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    OpenSingleInstanceAccount {
        kind: SingleInstancePDAAccountKind,
        nonce: u8,  // nonce used for not-having duplicate transactions rejected (only important for this ix for test cases)
    },

    #[usr(payer, { writable, signer })]
    #[usr(pda_account, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    OpenMultiInstanceAccount {
        pda_offset: u64,
        kind: MultiInstancePDAAccountKind,
        nonce: u8,
    },

    #[usr(base_commitment_q, { owned })]
    #[usr(commitment_q, { owned })]
    #[usr(send_proof_q, { owned })]
    #[usr(merge_proof_q, { owned })]
    #[usr(migrate_proof_q, { owned })]
    #[usr(finalize_send_q, { owned })]
    #[pda(q_manager, QueueManagement, { writable })]
    SetupQueueAccounts,

    TestFail,
}

#[cfg(feature = "instruction-abi")]
pub fn open_all_initial_accounts(payer: Pubkey, nonce: u8) -> Vec<solana_program::instruction::Instruction> {
    use ElusivInstruction as EI;

    let mut ixs = Vec::new();

    // Single instance PDAs
    // Pool
    ixs.push(EI::open_single_instance_account(
        SingleInstancePDAAccountKind::Pool,
        nonce,
        SignerAccount(payer),
        WritableUserAccount(PoolAccount::find(None).0)
    ));
    // QueueManager
    ixs.push(EI::open_single_instance_account(
        SingleInstancePDAAccountKind::QueueManagementAccount,
        nonce,
        SignerAccount(payer),
        WritableUserAccount(QueueManagementAccount::find(None).0)
    ));
    // CommitmentHashing
    ixs.push(EI::open_single_instance_account(
        SingleInstancePDAAccountKind::CommitmentHashing,
        nonce,
        SignerAccount(payer),
        WritableUserAccount(CommitmentHashingAccount::find(None).0)
    ));

    // Multi instance PDAs
    // BaseCommitmentHashingAccount
    ixs.push(EI::open_multi_instance_account(
        0,
        MultiInstancePDAAccountKind::BaseCommitmentHashing,
        nonce,
        SignerAccount(payer),
        WritableUserAccount(BaseCommitmentHashingAccount::find(Some(0)).0)
    ));
    // VerificationAccount
    ixs.push(EI::open_multi_instance_account(
        0,
        MultiInstancePDAAccountKind::Verification,
        nonce,
        SignerAccount(payer),
        WritableUserAccount(VerificationAccount::find(Some(0)).0)
    ));

    ixs
}