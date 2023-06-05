#![allow(clippy::too_many_arguments)]

use super::processor;
use super::processor::BaseCommitmentHashRequest;
use crate::macros::*;
use crate::processor::{FinalizeSendData, ProofRequest, VKeyAccountDataPacket, MAX_MT_COUNT};
use crate::state::{
    commitment::{
        BaseCommitmentBufferAccount, BaseCommitmentHashingAccount, CommitmentBufferAccount,
        CommitmentHashingAccount, CommitmentQueueAccount,
    },
    fee::{FeeAccount, ProgramFee},
    governor::{FeeCollectorAccount, GovernorAccount, PoolAccount},
    metadata::{CommitmentMetadata, MetadataAccount, MetadataQueueAccount},
    nullifier::NullifierAccount,
    proof::VerificationAccount,
    storage::StorageAccount,
    vkey::VKeyAccount,
};
use crate::types::Proof;
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_types::{AccountRepr, ElusivOption};
use solana_program::{pubkey::Pubkey, system_program, sysvar::instructions};

#[cfg(feature = "elusiv-client")]
pub use elusiv_types::accounts::{
    SignerAccount, UserAccount, WritableSignerAccount, WritableUserAccount,
};

#[repr(u8)]
#[derive(BorshDeserialize, BorshSerialize, ElusivInstruction)]
#[allow(clippy::large_enum_variant)]
pub enum ElusivInstruction {
    // -------- Base commitment hashing --------
    /// Client sends `base_commitment` and `amount` to be stored in the Elusiv program
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
    #[pda(storage_account, StorageAccount)]
    #[pda(hashing_account, BaseCommitmentHashingAccount, pda_offset = Some(hash_account_index), { writable, skip_pda_verification, account_info })]
    #[pda(buffer, BaseCommitmentBufferAccount, { writable })]
    #[acc(token_program)] // if `token_id = 0` { `system_program` } else { `token_program` }
    #[sys(system_program, key = system_program::ID)]
    StoreBaseCommitment {
        hash_account_index: u32,
        hash_account_bump: u8,
        request: BaseCommitmentHashRequest,
        metadata: CommitmentMetadata,
    },

    #[pda(hashing_account, BaseCommitmentHashingAccount, pda_offset = Some(hash_account_index), { writable })]
    ComputeBaseCommitmentHash { hash_account_index: u32 },

    #[acc(original_fee_payer, { writable })]
    #[pda(pool, PoolAccount, { writable, account_info })]
    #[pda(fee, FeeAccount, pda_offset = Some(fee_version))]
    #[pda(hashing_account, BaseCommitmentHashingAccount, pda_offset = Some(hash_account_index), { writable, account_info })]
    #[pda(commitment_hash_queue, CommitmentQueueAccount, { writable })]
    #[pda(metadata_queue, MetadataQueueAccount, { writable })]
    FinalizeBaseCommitmentHash {
        hash_account_index: u32,
        fee_version: u32,
    },

    // -------- Commitment hashing --------
    /// Hashes commitments in a new MT-root
    #[pda(commitment_hashing_account, CommitmentHashingAccount, { writable })]
    #[pda(storage_account, StorageAccount, { include_child_accounts })]
    InitCommitmentHashSetup { insertion_can_fail: bool },

    #[pda(commitment_hash_queue, CommitmentQueueAccount, { writable })]
    #[pda(metadata_queue, MetadataQueueAccount, { writable })]
    #[pda(commitment_hashing_account, CommitmentHashingAccount, { writable })]
    #[pda(metadata_account, MetadataAccount, { writable, include_child_accounts })]
    InitCommitmentHash { insertion_can_fail: bool },

    #[acc(fee_payer, { writable, signer })]
    #[pda(fee, FeeAccount, pda_offset = Some(fee_version))]
    #[pda(pool, PoolAccount, { writable, account_info })]
    #[pda(commitment_hashing_account, CommitmentHashingAccount, { writable })]
    ComputeCommitmentHash { fee_version: u32, nonce: u32 },

    #[pda(commitment_hashing_account, CommitmentHashingAccount, { writable })]
    #[pda(storage_account, StorageAccount, { include_child_accounts, writable })]
    FinalizeCommitmentHash,

    // -------- Proof Verification --------
    /// Proof verification initialization
    #[acc(fee_payer, { writable, signer })]
    #[pda(verification_account, VerificationAccount, pda_pubkey = fee_payer.pubkey(), pda_offset = Some(verification_account_index.into()), { writable, account_info, find_pda })]
    #[pda(vkey_account, VKeyAccount, pda_offset = Some(vkey_id))]
    #[acc(nullifier_duplicate_account, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    #[acc(identifier_account)]
    #[pda(storage_account, StorageAccount)]
    #[pda(buffer, CommitmentBufferAccount, { writable })]
    #[pda(nullifier_account0, NullifierAccount, pda_offset = Some(tree_indices[0]), { include_child_accounts })]
    #[pda(nullifier_account1, NullifierAccount, pda_offset = Some(tree_indices[1]), { include_child_accounts })]
    InitVerification {
        verification_account_index: u8,
        vkey_id: u32,
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
    #[pda(verification_account, VerificationAccount, pda_pubkey = fee_payer.pubkey(), pda_offset = Some(verification_account_index.into()), { writable })]
    #[acc(token_program)] // if `token_id = 0` { `system_program` } else { `token_program` }
    #[sys(system_program, key = system_program::ID)]
    InitVerificationTransferFee { verification_account_index: u8 },

    #[acc(fee_payer, { signer })]
    #[pda(verification_account, VerificationAccount, pda_pubkey = fee_payer.pubkey(), pda_offset = Some(verification_account_index.into()), { writable })]
    InitVerificationProof {
        verification_account_index: u8,
        proof: Proof,
    },

    /// Proof verification computation
    #[acc(original_fee_payer, { ignore })]
    #[pda(verification_account, VerificationAccount, pda_pubkey = original_fee_payer.pubkey(), pda_offset = Some(verification_account_index.into()), { writable })]
    #[pda(vkey_account, VKeyAccount, pda_offset = Some(vkey_id), { include_child_accounts })]
    #[sys(instructions_account, key = instructions::ID)]
    ComputeVerification {
        verification_account_index: u8,
        vkey_id: u32,
    },

    /// Finalizing proofs
    #[acc(recipient)]
    #[acc(identifier_account)]
    #[acc(transaction_reference_account)]
    #[acc(original_fee_payer, { ignore })]
    #[pda(commitment_hash_queue, CommitmentQueueAccount, { writable })]
    #[pda(verification_account, VerificationAccount, pda_pubkey = original_fee_payer.pubkey(), pda_offset = Some(verification_account_index.into()), { writable })]
    #[pda(storage_account, StorageAccount)]
    #[pda(buffer, CommitmentBufferAccount, { writable })]
    #[sys(instructions_account, key = instructions::ID)]
    FinalizeVerificationSend {
        verification_account_index: u8,
        data: FinalizeSendData,
        uses_memo: bool,
    },

    #[acc(original_fee_payer, { ignore })]
    #[pda(verification_account, VerificationAccount, pda_pubkey = original_fee_payer.pubkey(), pda_offset = Some(verification_account_index.into()), { writable })]
    #[pda(nullifier_account, NullifierAccount, pda_offset = Some(verification_account.get_tree_indices(0)), { writable, include_child_accounts, skip_abi })]
    FinalizeVerificationInsertNullifier { verification_account_index: u8 },

    #[acc(original_fee_payer, { signer, writable })]
    #[acc(recipient, { writable })]
    #[pda(pool, PoolAccount, { account_info, writable })]
    #[pda(fee_collector, FeeCollectorAccount, { account_info, writable })]
    #[acc(optional_fee_collector, { account_info, writable })]
    #[pda(commitment_hash_queue, CommitmentQueueAccount, { writable })]
    #[pda(metadata_queue, MetadataQueueAccount, { writable })]
    #[pda(verification_account, VerificationAccount, pda_pubkey = original_fee_payer.pubkey(), pda_offset = Some(verification_account_index.into()), { writable, account_info })]
    #[acc(nullifier_duplicate_account, { writable, owned })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    #[sys(instructions_account, key = instructions::ID)]
    FinalizeVerificationTransferLamports { verification_account_index: u8 },

    #[acc(original_fee_payer, { signer, writable })]
    #[acc(original_fee_payer_account, { writable })]
    #[acc(recipient, { writable })]
    #[acc(recipient_wallet)]
    #[pda(pool, PoolAccount, { account_info, writable })]
    #[acc(pool_account, { writable })]
    #[pda(fee_collector, FeeCollectorAccount, { account_info, writable })]
    #[acc(fee_collector_account, { writable })]
    #[acc(optional_fee_collector, { account_info, writable })]
    #[pda(commitment_hash_queue, CommitmentQueueAccount, { writable })]
    #[pda(metadata_queue, MetadataQueueAccount, { writable })]
    #[pda(verification_account, VerificationAccount, pda_pubkey = original_fee_payer.pubkey(), pda_offset = Some(verification_account_index.into()), { writable, account_info })]
    #[acc(nullifier_duplicate_account, { writable, owned })]
    #[sys(a_token_program, key = spl_associated_token_account::ID, { ignore })]
    #[sys(token_program, key = spl_token::ID)]
    #[sys(system_program, key = system_program::ID, { ignore })]
    #[acc(mint_account)]
    #[sys(instructions_account, key = instructions::ID)]
    FinalizeVerificationTransferToken { verification_account_index: u8 },

    // -------- Verifying key management --------
    #[acc(signer, { writable, signer })]
    #[pda(vkey_account, VKeyAccount, pda_offset = Some(vkey_id), { writable, account_info, find_pda })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    CreateVkeyAccount {
        vkey_id: u32,
        public_inputs_count: u32,
        deploy_authority: ElusivOption<Pubkey>,
    },

    #[acc(signer, { signer })]
    #[pda(vkey_account, VKeyAccount, pda_offset = Some(vkey_id), { writable })]
    #[acc(vkey_binary_data_account, { writable })]
    CreateNewVkeyVersion { vkey_id: u32 },

    #[acc(signer, { signer, writable })]
    #[pda(vkey_account, VKeyAccount, pda_offset = Some(vkey_id), { writable })]
    #[acc(old_vkey_binary_data_account, { writable })]
    #[sys(system_program, key = system_program::ID)]
    UpdateVkeyVersion { vkey_id: u32 },

    #[acc(signer, { signer })]
    #[pda(vkey_account, VKeyAccount, pda_offset = Some(vkey_id), { writable, include_child_accounts })]
    SetVkeyData {
        vkey_id: u32,
        data_position: u32,
        packet: VKeyAccountDataPacket,
    },

    #[acc(signer, { signer })]
    #[pda(vkey_account, VKeyAccount, pda_offset = Some(vkey_id), { writable })]
    FreezeVkey { vkey_id: u32 },

    #[acc(signer, { signer })]
    #[pda(vkey_account, VKeyAccount, pda_offset = Some(vkey_id), { writable })]
    ChangeVkeyAuthority { vkey_id: u32, authority: Pubkey },

    // -------- MT management --------
    /// Set the next MT as the active MT
    #[pda(storage_account, StorageAccount, { writable, include_child_accounts })]
    #[pda(commitment_hash_queue, CommitmentQueueAccount, { writable })]
    #[pda(active_nullifier_account, NullifierAccount, pda_offset = Some(active_mt_index), { writable })]
    ResetActiveMerkleTree { active_mt_index: u32 },

    /// Archives a `NullifierAccount` into a N-SMT
    #[acc(payer, { writable, signer })]
    #[pda(storage_account, StorageAccount, { writable, include_child_accounts })]
    #[pda(nullifier_account, NullifierAccount, pda_offset = Some(closed_mt_index), { writable, include_child_accounts })]
    #[acc(archived_tree_account, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    ArchiveClosedMerkleTree { closed_mt_index: u32 },

    // -------- Program state management --------
    #[acc(payer, { writable, signer })]
    #[pda(pool_account, PoolAccount, { writable, skip_pda_verification, account_info })]
    #[pda(fee_collector_account, FeeCollectorAccount, { writable, skip_pda_verification, account_info })]
    #[pda(commitment_hashing_account, CommitmentHashingAccount, { writable, skip_pda_verification, account_info })]
    #[pda(commitment_queue_account, CommitmentQueueAccount, { writable, skip_pda_verification, account_info })]
    #[pda(storage_account, StorageAccount, { writable, skip_pda_verification, account_info })]
    #[pda(base_commitment_buffer_account, BaseCommitmentBufferAccount, { writable, skip_pda_verification, account_info })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    OpenSingleInstanceAccounts,

    #[acc(payer, { writable, signer })]
    #[pda(nullifier_account, NullifierAccount, pda_offset = Some(mt_index), { writable, skip_pda_verification, account_info })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    OpenNullifierAccount { mt_index: u32 },

    #[pda(storage_account, StorageAccount, { writable })]
    #[acc(child_account, { owned, writable })]
    EnableStorageChildAccount { child_index: u32 },

    #[pda(nullifier_account, NullifierAccount, pda_offset = Some(mt_index), { writable })]
    #[acc(child_account, { owned, writable })]
    EnableNullifierChildAccount { mt_index: u32, child_index: u32 },

    #[pda(metadata_account, MetadataAccount, { writable })]
    #[acc(child_account, { owned, writable })]
    EnableMetadataChildAccount { child_index: u32 },

    #[acc(payer, { writable, signer })]
    #[pda(governor, GovernorAccount, { writable, skip_pda_verification, account_info })]
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
    #[pda(fee, FeeAccount, pda_offset = Some(fee_version), { writable, skip_pda_verification, account_info })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    InitNewFeeVersion {
        fee_version: u32,
        program_fee: ProgramFee,
    },

    #[cfg(not(feature = "mainnet"))]
    #[acc(payer, { signer })]
    #[acc(recipient, { writable })]
    #[acc(program_account, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    CloseProgramAccount,

    #[acc(payer, { writable, signer })]
    #[pda(buffer, CommitmentBufferAccount, { writable, skip_pda_verification, account_info })]
    #[pda(metadata_queue, MetadataQueueAccount, { writable, skip_pda_verification, account_info })]
    #[pda(metadata_account, MetadataAccount, { writable, skip_pda_verification, account_info })]
    #[pda(storage_account, StorageAccount)]
    #[pda(commitment_hashing_account, CommitmentHashingAccount)]
    #[pda(commitment_queue_account, CommitmentQueueAccount, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    CreateNewAccountsV1,

    // -------- NOP --------
    /// NOP-instruction
    Nop,
}

#[cfg(feature = "elusiv-client")]
use elusiv_types::accounts::PDAAccount;

#[cfg(feature = "elusiv-client")]
impl ElusivInstruction {
    pub fn store_base_commitment_sol_instruction(
        hash_account_index: u32,
        request: BaseCommitmentHashRequest,
        metadata: CommitmentMetadata,
        client: Pubkey,
        warden: Pubkey,
    ) -> solana_program::instruction::Instruction {
        let hash_account_bump = BaseCommitmentHashingAccount::find(Some(hash_account_index)).1;

        ElusivInstruction::store_base_commitment_instruction(
            hash_account_index,
            hash_account_bump,
            request,
            metadata,
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
        verification_account_index: u8,
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
        verification_account_index: u8,
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
        assert_eq!(
            1,
            get_variant_tag!(ElusivInstruction::ComputeBaseCommitmentHash {
                hash_account_index: 123,
            })
        );
    }

    #[test]
    fn test_elusiv_instruction_tag() {
        // Tests used to ensure correctness of the Warden-Network stats tracking tags

        assert_eq!(2, ElusivInstruction::FINALIZE_BASE_COMMITMENT_HASH_INDEX);
        assert_eq!(
            13,
            ElusivInstruction::FINALIZE_VERIFICATION_TRANSFER_LAMPORTS_INDEX
        );
        assert_eq!(
            14,
            ElusivInstruction::FINALIZE_VERIFICATION_TRANSFER_TOKEN_INDEX
        );
    }
}
