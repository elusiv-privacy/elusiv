#![allow(clippy::too_many_arguments)]

use crate::macros::*;
use crate::bytes::{BorshSerDeSized, BorshSerDeSizedEnum};
use crate::state::fee::ProgramFee;
use crate::types::Proof;
use super::processor;
use super::processor::BaseCommitmentHashRequest;
use crate::processor::{
    SingleInstancePDAAccountKind,
    MultiInstancePDAAccountKind,
    TokenAuthorityAccountKind,
    ProofRequest, MAX_MT_COUNT, FinalizeSendData,
};
use crate::state::queue::CommitmentQueueAccount;
use crate::state::{
    program_account::{
        PDAAccount,
        MultiAccountAccount,
        ProgramAccount,
        MultiAccountProgramAccount,
    },
    governor::{GovernorAccount, PoolAccount, FeeCollectorAccount},
    StorageAccount,
    NullifierAccount,
    fee::FeeAccount,
};
use crate::commitment::{BaseCommitmentHashingAccount, CommitmentHashingAccount};
use crate::proof::{VerificationAccount, precompute::PrecomputesAccount};
use solana_program::entrypoint::ProgramResult;
use solana_program::{system_program, sysvar::instructions};
use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, ElusivInstruction)]
#[allow(clippy::large_enum_variant)]
pub enum ElusivInstruction {
    // -------- Base Commitment Hashing --------

    // Client sends `base_commitment` and `amount` to be stored in the Elusiv program
    #[acc(sender, { signer })]
    #[acc(sender_account, { writable })]
    #[acc(fee_payer, { writable, signer })]
    #[acc(fee_payer_account, { writable })]
    #[pda(pool, PoolAccount, { writable, account_info })]
    #[acc(pool_account, { writable })]
    #[pda(fee_collector, FeeCollectorAccount, { writable, account_info })]
    #[acc(fee_collector_account, { writable })]
    #[acc(sol_price_account)]
    #[acc(token_price_account)]
    #[pda(governor, GovernorAccount)]
    #[pda(hashing_account, BaseCommitmentHashingAccount, pda_offset = Some(hash_account_index), { writable, account_info, find_pda })]
    #[acc(token_program)]   // if `token_id = 0` { `system_program` } else { `token_program` }
    #[sys(system_program, key = system_program::ID)]
    StoreBaseCommitment {
        hash_account_index: u32,
        request: BaseCommitmentHashRequest,
    },

    #[pda(hashing_account, BaseCommitmentHashingAccount, pda_offset = Some(hash_account_index), { writable })]
    ComputeBaseCommitmentHash {
        hash_account_index: u32,
        nonce: u32,
    },

    #[acc(original_fee_payer, { writable })]
    #[pda(pool, PoolAccount, { writable, account_info })]
    #[pda(fee, FeeAccount, pda_offset = Some(fee_version))]
    #[pda(hashing_account, BaseCommitmentHashingAccount, pda_offset = Some(hash_account_index), { writable, account_info })]
    #[pda(commitment_hash_queue, CommitmentQueueAccount, { writable })]
    FinalizeBaseCommitmentHash {
        hash_account_index: u32,
        fee_version: u32,
    },

    // -------- Commitment Hashing --------

    // Hashes commitments in a new MT-root
    #[pda(commitment_hashing_account, CommitmentHashingAccount, { writable })]
    #[pda(storage_account, StorageAccount, { multi_accounts })]
    InitCommitmentHashSetup,

    #[pda(commitment_hash_queue, CommitmentQueueAccount, { writable })]
    #[pda(commitment_hashing_account, CommitmentHashingAccount, { writable })]
    InitCommitmentHash,

    #[acc(fee_payer, { writable, signer })]
    #[pda(fee, FeeAccount, pda_offset = Some(fee_version))]
    #[pda(pool, PoolAccount, { writable, account_info })]
    #[pda(commitment_hashing_account, CommitmentHashingAccount, { writable })]
    ComputeCommitmentHash {
        fee_version: u32,
        nonce: u32,
    },

    #[pda(commitment_hashing_account, CommitmentHashingAccount, { writable })]
    #[pda(storage_account, StorageAccount, { multi_accounts, writable })]
    FinalizeCommitmentHash,

    // -------- Proof Verification --------

    // Proof verification initialization
    #[acc(fee_payer, { writable, signer })]
    #[pda(verification_account, VerificationAccount, pda_offset = Some(verification_account_index), { writable, account_info, find_pda })]
    #[acc(nullifier_duplicate_account, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    #[acc(recipient)]
    #[pda(storage_account, StorageAccount, { multi_accounts, ignore_sub_accounts })]
    #[pda(nullifier_account0, NullifierAccount, pda_offset = Some(tree_indices[0]), { multi_accounts })]
    #[pda(nullifier_account1, NullifierAccount, pda_offset = Some(tree_indices[1]), { multi_accounts })]
    InitVerification {
        verification_account_index: u32,
        tree_indices: [u32; MAX_MT_COUNT],
        request: ProofRequest,
        skip_nullifier_pda: bool,
    },

    #[acc(fee_payer, { writable, signer })]
    #[acc(fee_payer_account, { writable })]
    #[pda(pool, PoolAccount, { writable, account_info })]
    #[acc(pool_account, { writable })]
    #[pda(fee_collector, FeeCollectorAccount, { writable, account_info })]
    #[acc(fee_collector_account, { writable })]
    #[acc(sol_price_account)]
    #[acc(token_price_account)]
    #[pda(governor, GovernorAccount)]
    #[pda(verification_account, VerificationAccount, pda_offset = Some(verification_account_index), { writable })]
    #[acc(token_program)]   // if `token_id = 0` { `system_program` } else { `token_program` }
    #[sys(system_program, key = system_program::ID)]
    InitVerificationTransferFee {
        verification_account_index: u32,
    },

    #[acc(fee_payer, { signer })]
    #[pda(verification_account, VerificationAccount, pda_offset = Some(verification_account_index), { writable })]
    InitVerificationProof {
        verification_account_index: u32,
        proof: Proof,
    },

    // Proof verification computation
    #[pda(verification_account, VerificationAccount, pda_offset = Some(verification_account_index), { writable })]
    #[pda(precomputes_account, PrecomputesAccount, { multi_accounts })]
    #[sys(instructions_account, key = instructions::ID)]
    ComputeVerification {
        verification_account_index: u32,
    },

    // Finalizing proofs that finished 
    #[acc(identifier_account)]
    #[acc(salt_account)]
    #[pda(commitment_hash_queue, CommitmentQueueAccount, { writable })]
    #[pda(verification_account, VerificationAccount, pda_offset = Some(verification_account_index), { writable })]
    #[pda(storage_account, StorageAccount, { multi_accounts, ignore_sub_accounts })]
    FinalizeVerificationSend {
        data: FinalizeSendData,
        verification_account_index: u32,
    },

    #[pda(verification_account, VerificationAccount, pda_offset = Some(verification_account_index), { writable })]
    #[pda(nullifier_account0, NullifierAccount, pda_offset = Some(verification_account.get_tree_indices(0)), { writable, multi_accounts, skip_abi })]
    #[pda(nullifier_account1, NullifierAccount, pda_offset = Some(verification_account.get_tree_indices(1)), { writable, multi_accounts, skip_abi  })]
    FinalizeVerificationSendNullifiers {
        verification_account_index: u32,
    },

    #[acc(recipient, { writable })]
    #[acc(original_fee_payer, { writable })]
    #[pda(pool, PoolAccount, { account_info, writable })]
    #[pda(fee_collector, FeeCollectorAccount, { account_info, writable })]
    #[pda(commitment_hash_queue, CommitmentQueueAccount, { writable })]
    #[pda(verification_account, VerificationAccount, pda_offset = Some(verification_account_index), { writable, account_info })]
    #[acc(nullifier_duplicate_account, { writable, owned })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    FinalizeVerificationTransferLamports {
        verification_account_index: u32,
    },

    #[acc(signer, { writable, signer })]
    #[acc(recipient, { writable })]
    #[acc(recipient_wallet)]
    #[acc(original_fee_payer, { writable })]
    #[acc(original_fee_payer_account, { writable })]
    #[pda(pool, PoolAccount, { account_info, writable })]
    #[acc(pool_account, { writable })]
    #[pda(fee_collector, FeeCollectorAccount, { account_info, writable })]
    #[acc(fee_collector_account, { writable })]
    #[pda(commitment_hash_queue, CommitmentQueueAccount, { writable })]
    #[pda(verification_account, VerificationAccount, pda_offset = Some(verification_account_index), { writable, account_info })]
    #[acc(nullifier_duplicate_account, { writable, owned })]
    #[sys(a_token_program, key = spl_associated_token_account::ID, { ignore })]
    #[sys(token_program, key = spl_token::ID)]
    #[sys(system_program, key = system_program::ID, { ignore })]
    #[acc(mint_account)]
    FinalizeVerificationTransferToken {
        verification_account_index: u32,
    },

    // -------- MT management --------

    // Set the next MT as the active MT
    #[pda(storage_account, StorageAccount, { writable, multi_accounts })]
    #[pda(commitment_hash_queue, CommitmentQueueAccount, { writable })]
    #[pda(active_nullifier_account, NullifierAccount, pda_offset = Some(active_mt_index), { writable, multi_accounts, ignore_sub_accounts })]
    ResetActiveMerkleTree {
        active_mt_index: u32,
    },

    // Archives a `NullifierAccount` into a N-SMT (Nullifier-Sparse-Merkle-Tree)
    #[acc(payer, { writable, signer })]
    #[pda(storage_account, StorageAccount, { writable, multi_accounts })]
    #[pda(nullifier_account, NullifierAccount, pda_offset = Some(closed_mt_index), { writable, multi_accounts })]
    #[acc(archived_tree_account, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    ArchiveClosedMerkleTree {
        closed_mt_index: u32,
    },

    // -------- Program State Setup/Management --------

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
        pda_offset: u32,
    },

    #[acc(payer, { writable, signer })]
    #[acc(pda_account, { writable })]
    #[acc(token_account, { writable, signer })]
    #[acc(mint_account)]
    #[sys(system_program, key = system_program::ID, { ignore })]
    #[sys(token_program, key = spl_token::ID, { ignore })]
    EnableTokenAccount {
        kind: TokenAuthorityAccountKind,
        token_id: u16,
    },

    #[pda(storage_account, StorageAccount, { account_info, writable })]
    #[acc(sub_account, { owned, writable })]
    EnableStorageSubAccount {
        sub_account_index: u32,
    },

    #[pda(nullifier_account, NullifierAccount, pda_offset = Some(mt_index), { account_info, writable })]
    #[acc(sub_account, { owned, writable })]
    EnableNullifierSubAccount {
        mt_index: u32,
        sub_account_index: u32,
    },

    #[pda(precomputes_account, PrecomputesAccount, { account_info, writable })]
    #[acc(sub_account, { owned, writable })]
    EnablePrecomputeSubAccount {
        sub_account_index: u32,
    },

    #[pda(precomputes_account, PrecomputesAccount, { writable, multi_accounts })]
    PrecomputeVKeys,

    #[acc(payer, { writable, signer })]
    #[acc(governor, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    SetupGovernorAccount,

    #[acc(authority, { signer })]
    #[pda(governor, GovernorAccount, { writable })]
    #[pda(commitment_hash_queue, CommitmentQueueAccount)]
    UpgradeGovernorState {
        fee_version: u32,
        batching_rate: u32,
    },

    #[acc(payer, { writable, signer })]
    #[pda(governor, GovernorAccount, { writable })]
    #[pda(fee, FeeAccount, pda_offset = Some(fee_version), { writable, account_info, find_pda })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    InitNewFeeVersion {
        fee_version: u32,
        program_fee: ProgramFee,
    },

    // -------- NOP --------
    Nop,
}

fn nop() -> ProgramResult {
    Ok(())
}

#[cfg(feature = "instruction-abi")]
use solana_program::pubkey::Pubkey;

#[cfg(feature = "instruction-abi")]
pub fn open_all_initial_accounts(payer: Pubkey) -> Vec<solana_program::instruction::Instruction> {
    vec![
        // Governor
        ElusivInstruction::setup_governor_account_instruction(
            WritableSignerAccount(payer),
            WritableUserAccount(GovernorAccount::find(None).0)
        ),

        // SOL pool
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::PoolAccount,
            WritableSignerAccount(payer),
            WritableUserAccount(PoolAccount::find(None).0)
        ),

        // Fee collector
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::FeeCollectorAccount,
            WritableSignerAccount(payer),
            WritableUserAccount(FeeCollectorAccount::find(None).0)
        ),

        // Commitment hashing
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::CommitmentHashingAccount,
            WritableSignerAccount(payer),
            WritableUserAccount(CommitmentHashingAccount::find(None).0)
        ),

        // Commitment queue
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::CommitmentQueueAccount,
            WritableSignerAccount(payer),
            WritableUserAccount(CommitmentQueueAccount::find(None).0)
        ),

        // Precomputes account
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::PrecomputesAccount,
            WritableSignerAccount(payer),
            WritableUserAccount(PrecomputesAccount::find(None).0)
        ),
    ]
}

#[cfg(feature = "instruction-abi")]
impl ElusivInstruction {
    pub fn store_base_commitment_sol_instruction(
        hash_account_index: u32,
        request: BaseCommitmentHashRequest,
        client: Pubkey,
        warden: Pubkey,
    ) -> solana_program::instruction::Instruction {
        ElusivInstruction::store_base_commitment_instruction(
            hash_account_index,
            request,
            SignerAccount(client),
            WritableUserAccount(client),
            WritableSignerAccount(warden),
            WritableUserAccount(warden),
            WritableUserAccount(PoolAccount::find(None).0),
            WritableUserAccount(FeeCollectorAccount::find(None).0),
            UserAccount(system_program::id()),
            UserAccount(system_program::id()),
            UserAccount(system_program::id()),
        )
    }

    pub fn init_verification_transfer_fee_sol_instruction(
        verification_account_index: u32,
        warden: Pubkey,
    ) -> solana_program::instruction::Instruction {
        ElusivInstruction::init_verification_transfer_fee_instruction(
            verification_account_index,
            WritableSignerAccount(warden),
            WritableUserAccount(warden),
            WritableUserAccount(PoolAccount::find(None).0),
            WritableUserAccount(FeeCollectorAccount::find(None).0),
            UserAccount(spl_token::id()),
            UserAccount(spl_token::id()),
            UserAccount(spl_token::id()),
        )
    }

    pub fn init_verification_transfer_fee_token_instruction(
        verification_account_index: u32,
        token_id: u16,
        warden: Pubkey,
        warden_account: Pubkey,
        pool_account: Pubkey,
        fee_collector_account: Pubkey,
    ) -> solana_program::instruction::Instruction {
        use crate::token::elusiv_token;

        ElusivInstruction::init_verification_transfer_fee_instruction(
            verification_account_index,
            WritableSignerAccount(warden),
            WritableUserAccount(warden_account),
            WritableUserAccount(pool_account),
            WritableUserAccount(fee_collector_account),
            UserAccount(elusiv_token(0).unwrap().pyth_usd_price_key),
            UserAccount(elusiv_token(token_id).unwrap().pyth_usd_price_key),
            UserAccount(spl_token::id()),
        )
    }
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
        assert_eq!(1, get_variant_tag!(ElusivInstruction::ComputeBaseCommitmentHash { hash_account_index: 123, nonce: 0, }));
    }
}