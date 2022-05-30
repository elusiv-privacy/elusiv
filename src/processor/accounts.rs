use std::collections::HashSet;
use borsh::{BorshSerialize, BorshDeserialize};
use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    system_instruction,
    program::invoke_signed,
    sysvar::Sysvar,
    rent::Rent, pubkey::Pubkey,
};
use crate::state::{
    pool::PoolAccount,
    program_account::{PDAAccount, SizedAccount, MultiInstanceAccount, MultiAccountAccount, BigArrayAccount},
    queue::{FinalizeSendQueueAccount, QueueManagementAccount, CommitmentQueueAccount, BaseCommitmentQueueAccount, SendProofQueueAccount, MergeProofQueueAccount, MigrateProofQueueAccount},
    StorageAccount,
};
use crate::proof::{VerificationAccount};
use crate::commitment::{BaseCommitmentHashingAccount, CommitmentHashingAccount};
use crate::error::ElusivError::{InvalidAccountBalance, InvalidInstructionData};
use crate::macros::*;
use crate::bytes::{BorshSerDeSized, is_zero};
use crate::types::U256;

#[derive(BorshSerialize, BorshDeserialize, BorshSerDeSized)]
pub enum SingleInstancePDAAccountKind {
    Pool,
    QueueManagement,
    CommitmentHashing,
    Storage,
}

#[derive(BorshSerialize, BorshDeserialize, BorshSerDeSized)]
pub enum MultiInstancePDAAccountKind {
    Verification,
    BaseCommitmentHashing,
}

macro_rules! single_instance_account {
    ($v: ident, $e: ident) => {
        match $v {
            SingleInstancePDAAccountKind::Pool => PoolAccount::$e,
            SingleInstancePDAAccountKind::QueueManagement => QueueManagementAccount::$e,
            SingleInstancePDAAccountKind::CommitmentHashing => CommitmentHashingAccount::$e,
            SingleInstancePDAAccountKind::Storage => StorageAccount::$e,
        }
    };
}

/// Used to open the PDA accounts, of which types there always only exist one instance
pub fn open_single_instance_account<'a>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,

    kind: SingleInstancePDAAccountKind,
    _nonce: u8,
) -> ProgramResult {
    let account_size = single_instance_account!(kind, SIZE);
    let (pk, bump) = single_instance_account!(kind, find)(None);
    let seed = vec![single_instance_account!(kind, SEED).to_vec(), vec![bump]];
    let signers_seeds: Vec<&[u8]> = seed.iter().map(|x| &x[..]).collect();
    guard!(pk == *pda_account.key, InvalidInstructionData);
    
    create_pda_account(payer, pda_account, account_size, bump, &signers_seeds)
}

macro_rules! multi_instance_account {
    ($v: ident, $e: ident) => {
        match $v {
            MultiInstancePDAAccountKind::Verification => VerificationAccount::$e,
            MultiInstancePDAAccountKind::BaseCommitmentHashing => BaseCommitmentHashingAccount::$e,
        }
    };
}

/// Used to open the PDA accounts, of which types there can exist multipe (that satisfy the trait: MultiInstanceAccount)
pub fn open_multi_instance_account<'a>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,

    pda_offset: u64,
    kind: MultiInstancePDAAccountKind,
    _nonce: u8,
) -> ProgramResult {
    guard!(pda_offset < multi_instance_account!(kind, MAX_INSTANCES), InvalidInstructionData);

    let account_size = multi_instance_account!(kind, SIZE);
    let (pk, bump) = multi_instance_account!(kind, find)(Some(pda_offset));
    let seed = vec![
        multi_instance_account!(kind, SEED).to_vec(),
        u64::to_le_bytes(pda_offset).to_vec(),
        vec![bump]
    ];
    let signers_seeds: Vec<&[u8]> = seed.iter().map(|x| &x[..]).collect();
    guard!(pk == *pda_account.key, InvalidInstructionData);

    create_pda_account(payer, pda_account, account_size, bump, &signers_seeds)
}

macro_rules! verify_data_account {
    ($account: expr, $ty: ty, $check_zero: literal) => {
        // Check zeroness
        if $check_zero { guard!(is_zero(&$account.data.borrow()[..]), InvalidInstructionData); }

        // Check data size
        guard!($account.data_len() == <$ty>::SIZE, InvalidInstructionData);

        // Check rent-exemption
        if cfg!(test) { // only unit-testing
            guard!($account.lamports() >= u64::MAX / 2, InvalidInstructionData);
        } else {
            guard!($account.lamports() >= Rent::get()?.minimum_balance(<$ty>::SIZE), InvalidInstructionData);
        }

        // Check ownership
        guard!(*$account.owner == crate::id(), InvalidInstructionData);
    };
}

/// Setup all queue accounts (they have exactly one instance each)
pub fn setup_queue_accounts(
    base_commitment_queue: &AccountInfo,
    commitment_queue: &AccountInfo,
    send_proof_queue: &AccountInfo,
    merge_proof_queue: &AccountInfo,
    migrate_proof_queue: &AccountInfo,
    finalize_send_queue: &AccountInfo,
    queue_manager: &mut QueueManagementAccount,
) -> ProgramResult {
    guard!(!queue_manager.get_finished_setup(), InvalidInstructionData);

    // Check for account non-ownership, size, zero-ness, rent-exemption and assign queue
    verify_data_account!(base_commitment_queue, BaseCommitmentQueueAccount, true);
    verify_data_account!(commitment_queue, CommitmentQueueAccount, true);
    verify_data_account!(send_proof_queue, SendProofQueueAccount, true);
    verify_data_account!(merge_proof_queue, MergeProofQueueAccount, true);
    verify_data_account!(migrate_proof_queue, MigrateProofQueueAccount, true);
    verify_data_account!(finalize_send_queue, FinalizeSendQueueAccount, true);

    // Check for duplicates
    let keys = vec![
        *base_commitment_queue.key,
        *commitment_queue.key,
        *send_proof_queue.key,
        *merge_proof_queue.key,
        *migrate_proof_queue.key,
        *finalize_send_queue.key,
    ];
    let set: HashSet<Pubkey> = keys.clone().drain(..).collect();
    assert!(set.len() == keys.len());

    queue_manager.set_base_commitment_queue(&keys[0].to_bytes());
    queue_manager.set_commitment_queue(&keys[1].to_bytes());
    queue_manager.set_send_proof_queue(&keys[2].to_bytes());
    queue_manager.set_merge_proof_queue(&keys[3].to_bytes());
    queue_manager.set_migrate_proof_queue(&keys[4].to_bytes());
    queue_manager.set_finalize_send_queue(&keys[5].to_bytes());

    queue_manager.set_finished_setup(&true);

    Ok(())
}

pub struct IntermediaryStorageSubAccount { }
impl SizedAccount for IntermediaryStorageSubAccount {
    const SIZE: usize = StorageAccount::INTERMEDIARY_ACCOUNT_SIZE;
}

pub struct LastStorageSubAccount { }
impl SizedAccount for LastStorageSubAccount {
    const SIZE: usize = StorageAccount::LAST_ACCOUNT_SIZE;
}

/// Setup the StorageAccount with it's 7 subaccounts
pub fn setup_storage_account(
    storage_account: &mut StorageAccount,
) -> ProgramResult {
    guard!(!storage_account.get_finished_setup(), InvalidInstructionData);

    verify_storage_sub_accounts(&storage_account)?;

    // Assign pubkeys
    for i in 0..StorageAccount::COUNT {
        storage_account.set_pubkeys(i, &storage_account.get_account(i).key.to_bytes());
    }

    // Check for duplicates
    let set: HashSet<U256> = storage_account.get_all_pubkeys().clone().drain(..).collect();
    assert!(set.len() == StorageAccount::COUNT);

    storage_account.set_finished_setup(&true);

    Ok(())   
}

/// Verify the storage account sub-accounts
/// - we do not check for zero-ness (nice our merkle-tree logic handles this)
fn verify_storage_sub_accounts(storage_account: &StorageAccount) -> ProgramResult {
    for i in 0..StorageAccount::COUNT {
        if i < StorageAccount::COUNT - 1 { 
            verify_data_account!(storage_account.get_account(i), IntermediaryStorageSubAccount, false);
        } else { 
            verify_data_account!(storage_account.get_account(i), LastStorageSubAccount, false);
        }
    }

    Ok(())
}

fn create_pda_account<'a>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    account_size: usize,
    bump: u8,
    signers_seeds: &[&[u8]],
) -> ProgramResult {
    let lamports_required = Rent::get()?.minimum_balance(account_size);
    let space: u64 = account_size.try_into().unwrap();
    guard!(payer.lamports() >= lamports_required, InvalidAccountBalance);

    invoke_signed(
        &system_instruction::create_account(
            &payer.key,
            &pda_account.key,
            lamports_required,
            space,
            &crate::id(),
        ),
        &[
            payer.clone(),
            pda_account.clone(),
        ],
        &[signers_seeds]
    )?;

    let data = &mut pda_account.data.borrow_mut()[..];

    // Save `bump_seed`
    data[0] = bump;
    // Set `initialized` flag
    data[1] = 1;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_account_valid() {
        let mut data = vec![0; StorageAccount::SIZE];
        generate_storage_accounts_valid_size!(accounts);
        let mut storage_account = StorageAccount::new(&mut data, &accounts[..]).unwrap();
        verify_storage_sub_accounts(&mut storage_account).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_storage_account_invalid_size() {
        let mut data = vec![0; StorageAccount::SIZE];

        generate_storage_accounts!(accounts, [
            StorageAccount::INTERMEDIARY_ACCOUNT_SIZE,
            StorageAccount::INTERMEDIARY_ACCOUNT_SIZE,
            StorageAccount::INTERMEDIARY_ACCOUNT_SIZE,
            StorageAccount::INTERMEDIARY_ACCOUNT_SIZE,
            StorageAccount::INTERMEDIARY_ACCOUNT_SIZE,
            StorageAccount::INTERMEDIARY_ACCOUNT_SIZE,
            StorageAccount::LAST_ACCOUNT_SIZE - 1,
        ]);

        let mut storage_account = StorageAccount::new(&mut data, &accounts[..]).unwrap();
        verify_storage_sub_accounts(&mut storage_account).unwrap();
    }
}