use crate::macros::*;
use crate::bytes::BorshSerDeSized;
use super::processor;
use super::processor::{MultiInstancePDAAccountKind, SingleInstancePDAAccountKind};
use super::state::queue::{
    QueueManagementAccount,
    BaseCommitmentQueueAccount,
    CommitmentQueueAccount,
    BaseCommitmentHashRequest,
};
use super::state::{
    program_account::{
        PDAAccount,
        MultiAccountAccount,
        MultiAccountAccountFields,
        ProgramAccount,
        MultiAccountProgramAccount,
    },
    pool::PoolAccount,
    StorageAccount,
};
use crate::proof::VerificationAccount;
use crate::commitment::{BaseCommitmentHashingAccount, CommitmentHashingAccount};
use solana_program::{
    system_program,
    account_info::{next_account_info, AccountInfo},
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
    #[usr(sender, { writable, signer })]
    #[pda(pool, Pool, { writable, account_info })]
    #[sys(system_program, key = system_program::ID)]
    #[pda(q_manager, QueueManagement)]
    #[prg(queue, BaseCommitmentQueue, key = q_manager.get_base_commitment_queue(), { writable })]
    Store {
        base_commitment_request: BaseCommitmentHashRequest,
    },

    // Base-commitment hashing
    #[usr(fee_payer, { signer, writable })]
    #[pda(q_manager, QueueManagement)]
    #[prg(queue, BaseCommitmentQueue, key = q_manager.get_base_commitment_queue(), { writable })]
    #[pda(hashing_account, BaseCommitmentHashing, pda_offset = Some(hash_account_index), { writable })]
    InitBaseCommitmentHash {
        hash_account_index: u64,
    },
    
    #[pda(hashing_account, BaseCommitmentHashing, pda_offset = Some(hash_account_index), { writable })]
    ComputeBaseCommitmentHash {
        hash_account_index: u64,
        nonce: u64,
    },

    #[pda(q_manager, QueueManagement)]
    #[prg(base_queue, BaseCommitmentQueue, key = q_manager.get_base_commitment_queue(), { writable })]
    #[prg(queue, CommitmentQueue, key = q_manager.get_commitment_queue(), { writable })]
    #[pda(hashing_account, BaseCommitmentHashing, pda_offset = Some(hash_account_index), { writable })]
    FinalizeBaseCommitmentHash {
        hash_account_index: u64,
    },

    // Commitment (MT-root) hashing
    #[usr(fee_payer, { signer, writable })]
    #[pda(q_manager, QueueManagement)]
    #[prg(queue, CommitmentQueue, key = q_manager.get_commitment_queue(), { writable })]
    #[pda(hashing_account, CommitmentHashing, { writable })]
    #[pda(storage_account, Storage, { multi_accounts })]
    InitCommitmentHash,
    
    #[pda(hashing_account, CommitmentHashing, { writable })]
    ComputeCommitmentHash {
        nonce: u64,
    },

    #[pda(q_manager, QueueManagement)]
    #[prg(queue, CommitmentQueue, key = q_manager.get_commitment_queue(), { writable })]
    #[pda(hashing_account, CommitmentHashing, { writable })]
    #[pda(storage_account, Storage, { multi_accounts, writable })]
    FinalizeCommitmentHash,

    #[pda(verification_account, Verification, pda_offset = Some(0), { writable })]
    VerifyProof,

    // Can be called once per `SingleInstancePDAAccountKind`
    #[usr(payer, { writable, signer })]
    #[usr(pda_account, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    OpenSingleInstanceAccount {
        kind: SingleInstancePDAAccountKind,
        nonce: u8,  // nonce used for not-having duplicate transactions rejected (only important for this ix for test cases)
    },

    // Can be called `MAX_ACCOUNTS_COUNT` times per `MultiInstancePDAAccountKind`
    #[usr(payer, { writable, signer })]
    #[usr(pda_account, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    OpenMultiInstanceAccount {
        pda_offset: u64,
        kind: MultiInstancePDAAccountKind,
        nonce: u8,
    },

    // Setup all queue accounts and store the pubkeys in the `QueueManagementAccount`
    #[usr(base_commitment_q, { owned })]
    #[usr(commitment_q, { owned })]
    #[usr(send_proof_q, { owned })]
    #[usr(merge_proof_q, { owned })]
    #[usr(migrate_proof_q, { owned })]
    #[usr(finalize_send_q, { owned })]
    #[pda(q_manager, QueueManagement, { writable })]
    SetupQueueAccounts,

    // Can be called once, setups all sub-accounts for the storage account
    // - `OpenMultiInstanceAccount` with `SingleInstancePDAAccountKind::Storage` has to be called before
    #[pda(storage_account, Storage, { multi_accounts, no_subaccount_check, writable })]
    SetupStorageAccount,
}

#[cfg(feature = "instruction-abi")]
pub fn open_all_initial_accounts(payer: Pubkey, nonce: u8) -> Vec<solana_program::instruction::Instruction> {
    let mut ixs = Vec::new();

    // Single instance PDAs
    // Pool
    ixs.push(ElusivInstruction::open_single_instance_account_instruction(
        SingleInstancePDAAccountKind::Pool,
        nonce,
        SignerAccount(payer),
        WritableUserAccount(PoolAccount::find(None).0)
    ));
    // QueueManager
    ixs.push(ElusivInstruction::open_single_instance_account_instruction(
        SingleInstancePDAAccountKind::QueueManagement,
        nonce,
        SignerAccount(payer),
        WritableUserAccount(QueueManagementAccount::find(None).0)
    ));
    // CommitmentHashing
    ixs.push(ElusivInstruction::open_single_instance_account_instruction(
        SingleInstancePDAAccountKind::CommitmentHashing,
        nonce,
        SignerAccount(payer),
        WritableUserAccount(CommitmentHashingAccount::find(None).0)
    ));

    // Multi instance PDAs
    // BaseCommitmentHashingAccount
    ixs.push(ElusivInstruction::open_multi_instance_account_instruction(
        0,
        MultiInstancePDAAccountKind::BaseCommitmentHashing,
        nonce,
        SignerAccount(payer),
        WritableUserAccount(BaseCommitmentHashingAccount::find(Some(0)).0)
    ));
    // VerificationAccount
    ixs.push(ElusivInstruction::open_multi_instance_account_instruction(
        0,
        MultiInstancePDAAccountKind::Verification,
        nonce,
        SignerAccount(payer),
        WritableUserAccount(VerificationAccount::find(Some(0)).0)
    ));

    ixs
}

#[cfg(feature = "instruction-abi")]
#[derive(Debug)]
pub struct UserAccount(pub solana_program::pubkey::Pubkey);

#[cfg(feature = "instruction-abi")]
#[derive(Debug)]
pub struct WritableUserAccount(pub solana_program::pubkey::Pubkey);

#[cfg(feature = "instruction-abi")]
#[derive(Debug)]
pub struct SignerAccount(pub solana_program::pubkey::Pubkey);

#[cfg(feature = "instruction-abi")]
#[derive(Debug)]
pub struct WritableSignerAccount(pub solana_program::pubkey::Pubkey);

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! get_variant_tag {
        ($v: expr) => {
            $v.try_to_vec().unwrap()[0]
        };
    }

    #[test]
    fn test_instruction_tag() {
        assert_eq!(get_variant_tag!(ElusivInstruction::InitBaseCommitmentHash { hash_account_index: 123 }), 1);
        assert_eq!(get_variant_tag!(ElusivInstruction::ComputeBaseCommitmentHash { hash_account_index: 123, nonce: 456 }), 2);
    }
}