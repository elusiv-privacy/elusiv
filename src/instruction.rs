use crate::macros::*;
use crate::bytes::BorshSerDeSized;
use super::processor;
use super::processor::{BaseCommitmentHashRequest};
use crate::processor::{SingleInstancePDAAccountKind, ProofRequest};
use crate::state::queue::{CommitmentQueueAccount, BaseCommitmentQueueAccount};
use crate::state::{
    program_account::{
        PDAAccount,
        MultiAccountAccount,
        MultiAccountAccountFields,
        ProgramAccount,
        MultiAccountProgramAccount,
    },
    pool::PoolAccount,
    StorageAccount,
    NullifierAccount,
};
use crate::fee::{FeeAccount};
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
    #[pda(fee, Fee, pda_offset = Some(fee_version))]
    #[pda(pool, Pool, { ignore })]
    #[prg(sol_pool, key = pool.get_sol_pool(), { writable, account_info })]
    #[prg(fee_collector, key = pool.get_fee_collector(), { writable, account_info })]
    #[sys(system_program, key = system_program::ID)]
    #[pda(base_commitment_queue, BaseCommitmentQueue, { writable })]
    StoreBaseCommitment {
        fee_version: u64,
        base_commitment_request: BaseCommitmentHashRequest,
    },

    #[acc(fee_payer, { writable, signer })]
    #[pda(base_commitment_queue, BaseCommitmentQueue, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    #[pda(hashing_account, BaseCommitmentHashing, pda_offset = Some(hash_account_index), { writable, account_info, find_pda })]
    InitBaseCommitmentHash {
        hash_account_index: u64,
    },

    #[acc(fee_payer, { writable, signer })]
    #[pda(fee, Fee, pda_offset = Some(fee_version))]
    #[pda(pool, Pool, { ignore })]
    #[prg(sol_pool, key = pool.get_sol_pool(), { writable, account_info })]
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

    // Commitment (MT-root) hashing
    #[pda(commitment_hash_queue, CommitmentQueue, { writable })]
    #[pda(commitment_hashing_account, CommitmentHashing, { writable })]
    #[pda(storage_account, Storage, { multi_accounts })]
    InitCommitmentHash,

    #[acc(fee_payer, { writable, signer })]
    #[pda(fee, Fee, pda_offset = Some(fee_version))]
    #[pda(pool, Pool, { ignore })]
    #[prg(sol_pool, key = pool.get_sol_pool(), { writable, account_info })]
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
    #[pda(fee, Fee, pda_offset = Some(fee_version))]
    #[pda(pool, Pool, { ignore })]
    #[prg(sol_pool, key = pool.get_sol_pool(), { writable, account_info })]
    #[prg(fee_collector, key = pool.get_fee_collector(), { writable, account_info })]
    #[pda(verification_account, Verification, pda_offset = Some(verification_account_index), { writable, account_info, find_pda })]
    #[pda(storage_account, Storage, { multi_accounts })]
    #[pda(nullifier_account0, Nullifier, pda_offset = Some(tree_indices[0]), { multi_accounts })]
    #[pda(nullifier_account1, Nullifier, pda_offset = Some(tree_indices[1]), { multi_accounts })]
    #[sys(system_program, key = system_program::ID)]
    InitProof {
        verification_account_index: u64,
        fee_version: u64,
        proof_request: ProofRequest,
        ignore_duplicate_verifications: bool,
        tree_indices: [u64; 2],
    },

    // Proof verification computation
    #[acc(fee_payer, { writable, signer })]
    #[pda(fee, Fee, pda_offset = Some(fee_version))]
    #[pda(pool, Pool, { ignore })]
    #[prg(sol_pool, key = pool.get_sol_pool(), { writable, account_info })]
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
    #[pda(pool, Pool, { ignore })]
    #[prg(sol_pool, key = pool.get_sol_pool(), { writable, account_info })]
    #[prg(fee_collector, key = pool.get_fee_collector(), { writable, account_info })]
    #[pda(verification_account, Verification, pda_offset = Some(verification_account_index), { writable, account_info })]
    #[pda(commitment_hash_queue, CommitmentQueue, { writable })]
    #[pda(nullifier_account0, Nullifier, pda_offset = Some(tree_indices[0]), { writable, multi_accounts })]
    #[pda(nullifier_account1, Nullifier, pda_offset = Some(tree_indices[1]), { writable, multi_accounts })]
    FinalizeProof {
        verification_account_index: u64,
        fee_version: u64,
        tree_indices: [u64; 2],
    },

    // Can be called once per `SingleInstancePDAAccountKind`
    #[acc(payer, { writable, signer })]
    #[acc(pda_account, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    OpenSingleInstanceAccount {
        kind: SingleInstancePDAAccountKind,
        nonce: u8,  // nonce used for not-having duplicate transactions rejected (only important for this ix for test cases)
    },

    // Can be called once, setups all sub-accounts for the storage account
    #[pda(storage_account, Storage, { multi_accounts, no_subaccount_check, writable })]
    SetupStorageAccount,

    #[acc(payer, { writable, signer })]
    #[pda(pool, Pool, { writable, account_info, find_pda })]
    #[acc(sol_pool, { owned })]
    #[acc(fee_collector, { owned })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    SetupPoolAccounts,

    #[acc(payer, { writable, signer })]
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
pub fn open_all_initial_accounts(payer: Pubkey, nonce: u8, lamports_per_tx: u64) -> Vec<solana_program::instruction::Instruction> {
    let mut ixs = Vec::new();

    // Genesis Fee
    ixs.push(init_genesis_fee_account(payer, lamports_per_tx));

    // Commitment hashing
    ixs.push(
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::CommitmentHashing,
            nonce,
            SignerAccount(payer),
            WritableUserAccount(CommitmentHashingAccount::find(None).0)
        )
    );

    // Commitment queue
    ixs.push(
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::CommitmentQueue,
            nonce,
            SignerAccount(payer),
            WritableUserAccount(CommitmentQueueAccount::find(None).0)
        )
    );

    // Base commitment queue
    ixs.push(
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::BaseCommitmentQueue,
            nonce,
            SignerAccount(payer),
            WritableUserAccount(BaseCommitmentQueueAccount::find(None).0)
        )
    );

    ixs
}

#[cfg(feature = "instruction-abi")]
pub fn init_genesis_fee_account(payer: Pubkey, lamports_per_tx: u64) -> solana_program::instruction::Instruction {
    use crate::fee::{MAX_BASE_COMMITMENT_NETWORK_FEE, MAX_PROOF_NETWORK_FEE, MAX_RELAYER_HASH_TX_FEE, MAX_RELAYER_PROOF_TX_FEE, MAX_RELAYER_PROOF_REWARD};

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
        //assert_eq!(1, get_variant_tag!(ElusivInstruction::ComputeBaseCommitmentHash { hash_account_index: 123, nonce: 0, fee_version: 0 }));
    }
}