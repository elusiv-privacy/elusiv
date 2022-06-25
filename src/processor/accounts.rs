use std::collections::HashSet;
use borsh::{BorshSerialize, BorshDeserialize};
use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    sysvar::Sysvar,
    rent::Rent,
};
use crate::{state::{
    governor::{GovernorAccount, PoolAccount, FeeCollectorAccount, GOVERNOR_UPGRADE_AUTHORITY},
    program_account::{MultiAccountAccount, ProgramAccount, MultiAccountAccountData},
    StorageAccount,
    queue::{CommitmentQueueAccount, BaseCommitmentQueueAccount},
    fee::{FeeAccount, ProgramFee}, NullifierAccount, MT_COMMITMENT_COUNT,
}, commitment::DEFAULT_COMMITMENT_BATCHING_RATE, bytes::usize_as_u32_safe};
use crate::commitment::{CommitmentHashingAccount};
use crate::error::ElusivError::{
    InvalidInstructionData,
    InvalidFeeVersion,
    InvalidAccount,
    MerkleTreeIsNotFullYet,
    SubAccountAlreadyExists
};
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

/// Opens one single instance `PDAAccount`, as long this PDA does not already exist
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
    BaseCommitmentQueueAccount,
    NullifierAccount,
}

/// Opens one multi instance `PDAAccount` with the offset `Some(pda_offset)`, as long this PDA does not already exist
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
        MultiInstancePDAAccountKind::NullifierAccount => {
            open_pda_account_with_offset::<NullifierAccount>(payer, pda_account, pda_offset)
        }
    }
}

/// Enables the supplied sub-account for the `StorageAccount`
pub fn enable_storage_sub_account(
    storage_account: &AccountInfo,
    sub_account: &AccountInfo,

    sub_account_index: u32,
) -> ProgramResult {
    // Note: we don't zero-check these accounts, since we will never access data that has not been set by the program
    setup_sub_account::<StorageAccount, {StorageAccount::COUNT}>(storage_account, sub_account, sub_account_index as usize, false)
}

/// Enables the supplied sub-account for a `NullifierAccount`
/// - Note: requires a prior call to `open_multi_instance_account`
/// - Note: the `NullifierAccount` will be useless until the MT with `index = merkle_tree_index - 1` is closed
pub fn enable_nullifier_sub_account(
    nullifier_account: &AccountInfo,
    sub_account: &AccountInfo,

    _merkle_tree_index: u64,
    sub_account_index: u32,
) -> ProgramResult {
    // Note: we don't zero-check these accounts, BUT we need to manipulate the maps we store in each account and set the size to zero 
    setup_sub_account::<NullifierAccount, {NullifierAccount::COUNT}>(nullifier_account, sub_account, sub_account_index as usize, false)?;

    // Set map size to zero (leading u32)
    let data = &mut sub_account.data.borrow_mut()[..];
    for b in data.iter_mut().take(4) {
        *b = 0;
    }

    Ok(())
}

/// Closes the active MT and activates the next one
/// - there are two scenarios in which this is required/allowed:
///     1. the active MT is full
///     2. the active MT is not full but the remaining places in the MT are < than the batching rate of the next commitment in the commitment queue
pub fn reset_active_merkle_tree(
    storage_account: &mut StorageAccount,
    active_nullifier_account: &mut NullifierAccount,

    active_merkle_tree_index: u64,
) -> ProgramResult {
    guard!(storage_account.get_trees_count() == active_merkle_tree_index, InvalidInstructionData);

    // Note: since batching is not yet implemented, we only close a MT when it's full
    guard!(storage_account.get_next_commitment_ptr() as usize >= MT_COMMITMENT_COUNT, MerkleTreeIsNotFullYet);

    storage_account.reset();
    storage_account.set_trees_count(&(active_merkle_tree_index + 1));
    active_nullifier_account.set_root(&storage_account.get_root());

    Ok(())
}

/// Archives a closed MT by creating creating a N-SMT in an `ArchivedTreeAccount`
pub fn archive_closed_merkle_tree<'a>(
    _payer: &AccountInfo<'a>,
    storage_account: &mut StorageAccount,
    _nullifier_account: &mut NullifierAccount,
    _archived_tree_account: &AccountInfo<'a>,

    closed_merkle_tree_index: u64,
) -> ProgramResult {
    guard!(storage_account.get_trees_count() > closed_merkle_tree_index, InvalidInstructionData);
    todo!("N-SMT not implemented yet");
}

/// Setup the `GovernorAccount` with the default values
/// - Note: there is no way of upgrading it atm
pub fn setup_governor_account<'a>(
    payer: &AccountInfo<'a>,
    governor_account: &AccountInfo<'a>,
) -> ProgramResult {
    open_pda_account_without_offset::<GovernorAccount>(payer, governor_account)?;

    let data = &mut governor_account.data.borrow_mut()[..];
    let mut governor = GovernorAccount::new(data)?;

    governor.set_commitment_batching_rate(&usize_as_u32_safe(DEFAULT_COMMITMENT_BATCHING_RATE));

    Ok(())
}

/// Changes the state of the `GovernorAccount`
pub fn upgrade_governor_state(
    authority: &AccountInfo,
    _governor_account: &mut GovernorAccount,
    _commitment_queue: &mut CommitmentQueueAccount,

    _fee_version: u64,
    _batching_rate: u32,
) -> ProgramResult {
    guard!(*authority.key == GOVERNOR_UPGRADE_AUTHORITY, InvalidAccount);
    todo!("Not implemented yet");
    // TODO: changes in the batching rate are only possible when checking the commitment queue
    // TODO: fee changes require empty queues
}

/// Setup a new `FeeAccount`
/// - Note: there is no way of upgrading the program fees atm
#[allow(clippy::too_many_arguments)]
pub fn init_new_fee_version<'a>(
    payer: &AccountInfo<'a>,
    governor: &GovernorAccount,
    new_fee: &AccountInfo<'a>,

    fee_version: u64,
    program_fee: ProgramFee,
) -> ProgramResult {
    guard!(fee_version == governor.get_fee_version(), InvalidFeeVersion);
    open_pda_account_with_offset::<FeeAccount>(payer, new_fee, fee_version)?;

    let mut data = new_fee.data.borrow_mut();
    let mut fee = FeeAccount::new(&mut data[..])?;
    fee.set_program_fee(&program_fee);
    Ok(())
}

/// Verifies a single user-supplied sub-account and then saves it's pubkey in the `main_account`
fn setup_sub_account<'a, T: MultiAccountAccount<'a>, const COUNT: usize>(
    main_account: &AccountInfo,
    sub_account: &AccountInfo,
    sub_account_index: usize,
    check_zeroness: bool,
) -> ProgramResult {
    let data = &mut main_account.data.borrow_mut()[..];
    let mut account_data = <MultiAccountAccountData<{COUNT}>>::new(data)?;

    if account_data.pubkeys[sub_account_index].is_some() {
        return Err(SubAccountAlreadyExists.into())
    }

    verify_extern_data_account(sub_account, T::ACCOUNT_SIZE, check_zeroness)?;
    account_data.pubkeys[sub_account_index] = Some(sub_account.key.to_bytes());

    // Check for pubkey duplicates
    let a: Vec<U256> = account_data.pubkeys.iter().filter_map(|&x| x).collect();
    let b: HashSet<U256> = a.clone().drain(..).collect();
    guard!(a.len() == b.len(), InvalidInstructionData);

    MultiAccountAccountData::override_slice(&account_data, data)?;

    Ok(())
}

/// Verifies that an account with `data_len` > 10 KiB (non PDA) is formatted correctly
fn verify_extern_data_account(
    account: &AccountInfo,
    data_len: usize,
    check_zeroness: bool,
) -> ProgramResult {
    guard!(account.data_len() == data_len, InvalidInstructionData);
    if check_zeroness {
        guard!(is_zero(&account.data.borrow()[..]), InvalidInstructionData);
    }

    // Check rent-exemption
    if cfg!(test) { // only unit-testing (since we have no ledger there)
        guard!(account.lamports() >= u64::MAX / 2, InvalidInstructionData);
    } else {
        guard!(account.lamports() >= Rent::get()?.minimum_balance(data_len), InvalidInstructionData);
    }

    // Check ownership
    guard!(*account.owner == crate::id(), InvalidInstructionData);

    Ok(())
}