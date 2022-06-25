#![allow(clippy::too_many_arguments)]

use crate::macros::*;
use crate::bytes::BorshSerDeSized;
use crate::state::fee::ProgramFee;
use super::processor;
use super::processor::{BaseCommitmentHashRequest};
use crate::processor::{SingleInstancePDAAccountKind, ProofRequest, MultiInstancePDAAccountKind};
use crate::state::queue::{CommitmentQueueAccount, BaseCommitmentQueueAccount};
use crate::state::{
    program_account::{
        PDAAccount,
        MultiAccountAccount,
        MultiAccountAccountData,
        ProgramAccount,
        MultiAccountProgramAccount,
    },
    governor::{GovernorAccount, PoolAccount, FeeCollectorAccount},
    StorageAccount,
    NullifierAccount,
    fee::FeeAccount,
};
use crate::proof::VerificationAccount;
use crate::commitment::{BaseCommitmentHashingAccount, CommitmentHashingAccount};
use solana_program::{
    system_program,
    account_info::{next_account_info, AccountInfo},
    pubkey::Pubkey,
    entrypoint::ProgramResult,
    program_error::ProgramError::{InvalidArgument, InvalidInstructionData, IllegalOwner},
};
use borsh::{BorshDeserialize, BorshSerialize};

#[cfg(feature = "instruction-abi")]
use solana_program::instruction::AccountMeta;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, ElusivInstruction)]
#[allow(clippy::large_enum_variant)]
pub enum ElusivInstruction {
    // Client sends base_commitment and amount to be stored in the Elusiv program
    #[acc(sender, { writable, signer })]
    #[pda(fee, Fee, pda_offset = Some(request.fee_version))]
    #[pda(governor, Governor)]
    #[pda(sol_pool, Pool, { writable, account_info })]
    #[pda(fee_collector, FeeCollector, { writable, account_info })]
    #[sys(system_program, key = system_program::ID)]
    #[pda(base_commitment_queue, BaseCommitmentQueue, pda_offset = Some(base_commitment_queue_index), { writable })]
    StoreBaseCommitment {
        base_commitment_queue_index: u64,
        request: BaseCommitmentHashRequest,
    },

    // Base commitment hashing (commitment = h(base_commitment, amount))
    #[acc(fee_payer, { writable, signer })]
    #[pda(base_commitment_queue, BaseCommitmentQueue, pda_offset = Some(base_commitment_queue_index), { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    #[pda(hashing_account, BaseCommitmentHashing, pda_offset = Some(hash_account_index), { writable, account_info, find_pda })]
    InitBaseCommitmentHash {
        base_commitment_queue_index: u64,
        hash_account_index: u64,
    },

    #[acc(fee_payer, { writable, signer })]
    #[pda(fee, Fee, pda_offset = Some(fee_version))]
    #[pda(sol_pool, Pool, { writable, account_info })]
    #[pda(hashing_account, BaseCommitmentHashing, pda_offset = Some(hash_account_index), { writable })]
    ComputeBaseCommitmentHash {
        hash_account_index: u64,
        fee_version: u64,
        nonce: u64,
    },

    #[acc(original_fee_payer, { writable })]
    #[pda(commitment_hash_queue, CommitmentQueue, { writable })]
    #[pda(hashing_account, BaseCommitmentHashing, pda_offset = Some(hash_account_index), { writable, account_info })]
    FinalizeBaseCommitmentHash {
        hash_account_index: u64,
    },

    // Hashes 1-N commitments in a new MT-root (Merkle-Tree-root)
    #[pda(commitment_hash_queue, CommitmentQueue, { writable })]
    #[pda(commitment_hashing_account, CommitmentHashing, { writable })]
    #[pda(storage_account, Storage, { multi_accounts })]
    InitCommitmentHash,

    #[acc(fee_payer, { writable, signer })]
    #[pda(fee, Fee, pda_offset = Some(fee_version))]
    #[pda(sol_pool, Pool, { writable, account_info })]
    #[pda(commitment_hashing_account, CommitmentHashing, { writable })]
    ComputeCommitmentHash {
        fee_version: u64,
        nonce: u64,
    },

    #[pda(commitment_hashing_account, CommitmentHashing, { writable })]
    #[pda(storage_account, Storage, { multi_accounts, writable })]
    FinalizeCommitmentHash,

    // Proof verification initialization for Send/Merge
    #[acc(fee_payer, { writable, signer })]
    #[pda(fee, Fee, pda_offset = Some(request.fee_version()))]
    #[pda(governor, Governor)]
    #[pda(sol_pool, Pool, { writable, account_info })]
    #[pda(fee_collector, FeeCollector, { writable, account_info })]
    #[pda(verification_account, Verification, pda_offset = Some(verification_account_index), { writable, account_info, find_pda })]
    #[pda(storage_account, Storage, { multi_accounts })]
    #[pda(nullifier_account0, Nullifier, pda_offset = Some(tree_indices[0]), { multi_accounts })]
    #[pda(nullifier_account1, Nullifier, pda_offset = Some(tree_indices[1]), { multi_accounts })]
    #[sys(system_program, key = system_program::ID)]
    InitProof {
        verification_account_index: u64,
        request: ProofRequest,
        ignore_duplicate_verifications: bool,
        tree_indices: [u64; 2],
    },

    // Proof verification computation
    #[acc(fee_payer, { writable, signer })]
    #[pda(fee, Fee, pda_offset = Some(fee_version))]
    #[pda(sol_pool, Pool, { writable, account_info })]
    #[pda(verification_account, Verification, pda_offset = Some(verification_account_index), { writable })]
    ComputeProof {
        verification_account_index: u64,
        fee_version: u64,
        nonce: u64,
    },

    // Finalizing successfully verified proofs
    #[acc(original_fee_payer, { writable })]
    #[acc(recipient, { writable })]
    #[pda(fee, Fee, pda_offset = Some(fee_version))]
    #[pda(sol_pool, Pool, { writable, account_info })]
    #[pda(fee_collector, FeeCollector, { writable, account_info })]
    #[pda(verification_account, Verification, pda_offset = Some(verification_account_index), { writable, account_info })]
    #[pda(commitment_hash_queue, CommitmentQueue, { writable })]
    #[pda(nullifier_account0, Nullifier, pda_offset = Some(tree_indices[0]), { writable, multi_accounts })]
    #[pda(nullifier_account1, Nullifier, pda_offset = Some(tree_indices[1]), { writable, multi_accounts })]
    FinalizeProof {
        verification_account_index: u64,
        fee_version: u64,
        tree_indices: [u64; 2],
    },

    // Set the next MT as the active MT
    #[pda(storage_account, Storage, { writable, multi_accounts })]
    #[pda(commitment_hash_queue, CommitmentQueue, { writable })]
    #[pda(active_nullifier_account, Nullifier, pda_offset = Some(active_mt_index), { writable, multi_accounts })]
    ResetActiveMerkleTree {
        active_mt_index: u64,
    },

    // Archives a `NullifierAccount` into a N-SMT (Nullifier-Sparse-Merkle-Tree)
    #[acc(payer, { writable, signer })]
    #[pda(storage_account, Storage, { writable, multi_accounts })]
    #[pda(nullifier_account, Nullifier, pda_offset = Some(closed_mt_index), { writable, multi_accounts })]
    #[acc(archived_tree_account, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    ArchiveClosedMerkleTree {
        closed_mt_index: u64,
    },

    // Opens one `PDAAccount` with offset = None
    #[acc(payer, { writable, signer })]
    #[acc(pda_account, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    OpenSingleInstanceAccount {
        kind: SingleInstancePDAAccountKind,
    },

    // Opens one `MultiInstancePDAAccount` with some offset
    #[acc(payer, { writable, signer })]
    #[acc(pda_account, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    OpenMultiInstanceAccount {
        kind: MultiInstancePDAAccountKind,
        pda_offset: u64,
    },

    #[pda(storage_account, Storage, { account_info, writable })]
    #[acc(sub_account, { owned })]
    EnableStorageSubAccount {
        sub_account_index: u32,
    },

    #[pda(nullifier_account, Nullifier, pda_offset = Some(mt_index), { account_info, writable })]
    #[acc(sub_account, { owned })]
    EnableNullifierSubAccount {
        mt_index: u64,
        sub_account_index: u32,
    },

    #[acc(payer, { writable, signer })]
    #[acc(governor, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    SetupGovernorAccount,

    #[acc(authority, { signer })]
    #[pda(governor, Governor, { writable })]
    #[pda(commitment_hash_queue, CommitmentQueue, { writable })]
    UpgradeGovernorState {
        fee_version: u64,
        batching_rate: u32,
    },

    #[acc(payer, { writable, signer })]
    #[pda(governor, Governor)]
    #[pda(fee, Fee, pda_offset = Some(fee_version), { writable, account_info, find_pda })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    InitNewFeeVersion {
        fee_version: u64,
        program_fee: ProgramFee,
    },
}

#[cfg(feature = "instruction-abi")]
pub fn open_all_initial_accounts(payer: Pubkey) -> Vec<solana_program::instruction::Instruction> {
    vec![
        // Governor
        ElusivInstruction::setup_governor_account_instruction(
            SignerAccount(payer),
            WritableUserAccount(GovernorAccount::find(None).0)
        ),

        // SOL pool
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::PoolAccount,
            SignerAccount(payer),
            WritableUserAccount(PoolAccount::find(None).0)
        ),

        // Fee collector
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::FeeCollectorAccount,
            SignerAccount(payer),
            WritableUserAccount(FeeCollectorAccount::find(None).0)
        ),

        // Commitment hashing
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::CommitmentHashingAccount,
            SignerAccount(payer),
            WritableUserAccount(CommitmentHashingAccount::find(None).0)
        ),

        // Commitment queue
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::CommitmentQueueAccount,
            SignerAccount(payer),
            WritableUserAccount(CommitmentQueueAccount::find(None).0)
        ),

        // Base commitment queue
        ElusivInstruction::open_multi_instance_account_instruction(
            MultiInstancePDAAccountKind::BaseCommitmentQueueAccount,
            0,
            SignerAccount(payer),
            WritableUserAccount(BaseCommitmentQueueAccount::find(Some(0)).0)
        ),
    ]
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
        assert_eq!(2, get_variant_tag!(ElusivInstruction::ComputeBaseCommitmentHash { hash_account_index: 123, nonce: 0, fee_version: 0 }));
    }
}