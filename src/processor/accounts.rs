use std::collections::HashSet;
use borsh::{BorshSerialize, BorshDeserialize};
use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    sysvar::Sysvar,
    rent::Rent,
};
use crate::state::{
    governor::{GovernorAccount, PoolAccount, FeeCollectorAccount, DEFAULT_COMMITMENT_BATCHING_RATE},
    program_account::{PDAAccount, SizedAccount, MultiAccountAccount, BigArrayAccount, ProgramAccount},
    StorageAccount,
    queue::{CommitmentQueueAccount, BaseCommitmentQueueAccount},
    fee::FeeAccount,
};
use crate::commitment::{CommitmentHashingAccount};
use crate::error::ElusivError::{InvalidInstructionData, InvalidFeeVersion};
use crate::macros::*;
use crate::bytes::{BorshSerDeSized, is_zero};
use crate::types::U256;
use super::utils::*;

#[derive(BorshSerialize, BorshDeserialize, BorshSerDeSized)]
pub enum SingleInstancePDAAccountKind {
    CommitmentHashingAccount,
    CommitmentQueueAccount,
    PoolAccount,
    FeeCollectorAccount,
    StorageAccount,
}

/// Used to open the PDA accounts, of which types there always only exist one instance
pub fn open_single_instance_account<'a>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,

    kind: SingleInstancePDAAccountKind,
) -> ProgramResult {
    match kind {
        SingleInstancePDAAccountKind::CommitmentHashingAccount => {
            open_pda_account_without_offset::<CommitmentHashingAccount>(payer, pda_account)
        }
        SingleInstancePDAAccountKind::CommitmentQueueAccount => {
            open_pda_account_without_offset::<CommitmentQueueAccount>(payer, pda_account)
        }
        SingleInstancePDAAccountKind::PoolAccount => {
            open_pda_account_without_offset::<PoolAccount>(payer, pda_account)
        }
        SingleInstancePDAAccountKind::FeeCollectorAccount => {
            open_pda_account_without_offset::<FeeCollectorAccount>(payer, pda_account)
        }
        SingleInstancePDAAccountKind::StorageAccount => {
            open_pda_account_without_offset::<StorageAccount>(payer, pda_account)
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, BorshSerDeSized)]
pub enum MultiInstancePDAAccountKind {
    BaseCommitmentQueueAccount
}

pub fn open_multi_instance_account<'a>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,

    kind: MultiInstancePDAAccountKind,
    pda_offset: u64,
) -> ProgramResult {
    match kind {
        MultiInstancePDAAccountKind::BaseCommitmentQueueAccount => {
            open_pda_account_with_offset::<BaseCommitmentQueueAccount>(payer, pda_account, pda_offset)
        }
    }
}

pub fn open_pda_account_with_offset<'a, T: PDAAccount + SizedAccount>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    pda_offset: u64,
) -> ProgramResult {
    let account_size = T::SIZE;
    let (pk, bump) = T::find(Some(pda_offset));
    let seed = vec![
        T::SEED.to_vec(),
        u64::to_le_bytes(pda_offset).to_vec(),
        vec![bump]
    ];
    let signers_seeds: Vec<&[u8]> = seed.iter().map(|x| &x[..]).collect();
    guard!(pk == *pda_account.key, InvalidInstructionData);

    create_pda_account(payer, pda_account, account_size, bump, &signers_seeds)
}

pub fn open_pda_account_without_offset<'a, T: PDAAccount + SizedAccount>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
) -> ProgramResult {
    let account_size = T::SIZE;
    let (pk, bump) = T::find(None);
    let seed = vec![
        T::SEED.to_vec(),
        vec![bump]
    ];
    let signers_seeds: Vec<&[u8]> = seed.iter().map(|x| &x[..]).collect();
    guard!(pk == *pda_account.key, InvalidInstructionData);

    create_pda_account(payer, pda_account, account_size, bump, &signers_seeds)
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

pub fn setup_governor_account<'a>(
    payer: &AccountInfo<'a>,
    governor_account: &AccountInfo<'a>,
) -> ProgramResult {
    open_pda_account_without_offset::<GovernorAccount>(payer, governor_account)?;

    let mut data = &mut governor_account.data.borrow_mut()[..];
    let mut governor = GovernorAccount::new(&mut data)?;

    governor.set_commitment_batching_rate(&DEFAULT_COMMITMENT_BATCHING_RATE);

    Ok(())
}

pub fn init_new_fee_version<'a>(
    payer: &AccountInfo<'a>,
    governor: &GovernorAccount,
    new_fee: &AccountInfo<'a>,

    fee_version: u64,

    lamports_per_tx: u64,
    base_commitment_fee: u64,
    proof_fee: u64,
    relayer_hash_tx_fee: u64,
    relayer_proof_tx_fee: u64,
    relayer_proof_reward: u64,
) -> ProgramResult {
    guard!(fee_version == governor.get_fee_version(), InvalidFeeVersion);
    open_pda_account_with_offset::<FeeAccount>(payer, new_fee, fee_version)?;

    let mut data = &mut new_fee.data.borrow_mut()[..];
    let mut fee = FeeAccount::new(&mut data)?;

    fee.setup(lamports_per_tx, base_commitment_fee, proof_fee, relayer_hash_tx_fee, relayer_proof_tx_fee, relayer_proof_reward)
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

pub struct IntermediaryStorageSubAccount { }
impl SizedAccount for IntermediaryStorageSubAccount {
    const SIZE: usize = StorageAccount::INTERMEDIARY_ACCOUNT_SIZE;
}

pub struct LastStorageSubAccount { }
impl SizedAccount for LastStorageSubAccount {
    const SIZE: usize = StorageAccount::LAST_ACCOUNT_SIZE;
}

/// Verify the storage account sub-accounts
/// - we do not check for zero-ness (nice our merkle-tree logic handles this)
fn verify_storage_sub_accounts(storage_account: &StorageAccount) -> ProgramResult {
    for i in 0..StorageAccount::COUNT {
        if i < StorageAccount::COUNT - 1 { 
            // note: we do not zero-check these accounts, since we will never access data that has not been set by the program
            verify_data_account!(storage_account.get_account(i), IntermediaryStorageSubAccount, false);
        } else { 
            verify_data_account!(storage_account.get_account(i), LastStorageSubAccount, false);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::program_account::MultiAccountProgramAccount;

    #[test]
    fn test_storage_account_valid() {
        let mut data = vec![0; StorageAccount::SIZE];
        generate_storage_accounts_valid_size!(accounts);
        let mut storage_account = StorageAccount::new(&mut data, accounts).unwrap();
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

        let mut storage_account = StorageAccount::new(&mut data, accounts).unwrap();
        verify_storage_sub_accounts(&mut storage_account).unwrap();
    }
}