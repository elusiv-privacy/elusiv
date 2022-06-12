use crate::macros::*;
use crate::bytes::BorshSerDeSized;
use super::processor;
use super::processor::{BaseCommitmentHashRequest};
use crate::processor::{SingleInstancePDAAccountKind, ProofRequest, MultiInstancePDAAccountKind};
use crate::state::queue::{CommitmentQueueAccount, BaseCommitmentQueueAccount};
use crate::state::{
    program_account::{
        PDAAccount,
        MultiAccountAccount,
        MultiAccountAccountFields,
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
    program_error::ProgramError::{InvalidArgument, InvalidInstructionData},
};
use borsh::{BorshDeserialize, BorshSerialize};

#[cfg(feature = "instruction-abi")]
use solana_program::instruction::AccountMeta;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, ElusivInstruction)]
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

    #[pda(commitment_hash_queue, CommitmentQueue, { writable })]
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
    #[pda(active_nullifier_account, Nullifier, pda_offset = Some(active_mt_index), { writable, multi_accounts })]
    #[pda(next_nullifier_account, Nullifier, pda_offset = Some(active_mt_index + 1), { writable, multi_accounts })]
    ResetActiveMerkleTree {
        active_mt_index: u64,
    },

    // Creates a new `NullifierAccount`
    #[pda(nullifier_account, Nullifier, pda_offset = Some(mt_index), { multi_accounts, no_subaccount_check, writable })]
    OpenNewMerkleTree {
        mt_index: u64,
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

    // Can be called once, setups all sub-accounts for the storage account
    #[pda(storage_account, Storage, { multi_accounts, no_subaccount_check, writable })]
    SetupStorageAccount,

    #[acc(payer, { writable, signer })]
    #[acc(governor, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    SetupGovernorAccount,

    #[acc(payer, { writable, signer })]
    #[pda(governor, Governor)]
    #[pda(fee, Fee, pda_offset = Some(fee_version), { writable, account_info, find_pda })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    InitNewFeeVersion {
        fee_version: u64,
        lamports_per_tx: u64,
        base_commitment_fee: u64,
        proof_fee: u64,
        relayer_hash_tx_fee: u64,
        relayer_proof_tx_fee: u64,
        relayer_proof_reward: u64,
    },
}

#[cfg(feature = "instruction-abi")]
pub fn open_all_initial_accounts(payer: Pubkey, lamports_per_tx: u64) -> Vec<solana_program::instruction::Instruction> {
    let mut ixs = Vec::new();
    
    // Governor
    ixs.push(
        ElusivInstruction::setup_governor_account_instruction(
            SignerAccount(payer),
            WritableUserAccount(GovernorAccount::find(None).0)
        )
    );

    // Genesis Fee
    ixs.push(init_genesis_fee_account(payer, lamports_per_tx));

    // SOL pool
    ixs.push(
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::PoolAccount,
            SignerAccount(payer),
            WritableUserAccount(PoolAccount::find(None).0)
        )
    );

    // Fee collector
    ixs.push(
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::FeeCollectorAccount,
            SignerAccount(payer),
            WritableUserAccount(FeeCollectorAccount::find(None).0)
        )
    );

    // Commitment hashing
    ixs.push(
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::CommitmentHashingAccount,
            SignerAccount(payer),
            WritableUserAccount(CommitmentHashingAccount::find(None).0)
        )
    );

    // Commitment queue
    ixs.push(
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::CommitmentQueueAccount,
            SignerAccount(payer),
            WritableUserAccount(CommitmentQueueAccount::find(None).0)
        )
    );

    // Base commitment queue
    ixs.push(
        ElusivInstruction::open_multi_instance_account_instruction(
            MultiInstancePDAAccountKind::BaseCommitmentQueueAccount,
            0,
            SignerAccount(payer),
            WritableUserAccount(BaseCommitmentQueueAccount::find(Some(0)).0)
        )
    );

    ixs
}

#[cfg(feature = "instruction-abi")]
pub fn init_genesis_fee_account(payer: Pubkey, lamports_per_tx: u64) -> solana_program::instruction::Instruction {
    use crate::state::fee::{MAX_BASE_COMMITMENT_NETWORK_FEE, MAX_PROOF_NETWORK_FEE, MAX_RELAYER_HASH_TX_FEE, MAX_RELAYER_PROOF_TX_FEE, MAX_RELAYER_PROOF_REWARD};

    ElusivInstruction::init_new_fee_version_instruction(
        0,
        lamports_per_tx,
        MAX_BASE_COMMITMENT_NETWORK_FEE,
        MAX_PROOF_NETWORK_FEE,
        MAX_RELAYER_HASH_TX_FEE,
        MAX_RELAYER_PROOF_TX_FEE,
        MAX_RELAYER_PROOF_REWARD,
        SignerAccount(payer),
    )
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