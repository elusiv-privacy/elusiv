use borsh::{BorshSerialize, BorshDeserialize};
use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    sysvar::Sysvar,
    rent::Rent, program_error::ProgramError,
};
use crate::{state::{
    governor::{GovernorAccount, PoolAccount, FeeCollectorAccount},
    program_account::{MultiAccountAccount, ProgramAccount, MultiAccountAccountData, SubAccount},
    StorageAccount,
    queue::{CommitmentQueueAccount, BaseCommitmentQueueAccount, CommitmentQueue, Queue},
    fee::{FeeAccount, ProgramFee}, NullifierAccount, MT_COMMITMENT_COUNT,
}, commitment::DEFAULT_COMMITMENT_BATCHING_RATE, bytes::usize_as_u32_safe, processor::MATH_ERR, proof::PendingNullifierHashesAccount};
use crate::commitment::{CommitmentHashingAccount};
use crate::error::ElusivError::{
    InvalidAccount,
    InvalidInstructionData,
    InvalidFeeVersion,
    MerkleTreeIsNotFullYet,
    SubAccountAlreadyExists
};
use crate::macros::*;
use crate::bytes::{BorshSerDeSized, ElusivOption, is_zero};
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
    setup_sub_account::<StorageAccount, {StorageAccount::COUNT}>(
        storage_account,
        sub_account,
        sub_account_index as usize,
        false
    )
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
    setup_sub_account::<NullifierAccount, {NullifierAccount::COUNT}>(
        nullifier_account,
        sub_account,
        sub_account_index as usize,
        false
    )?;

    // Set map size to zero
    reset_map_sub_account(sub_account);

    Ok(())
}

/// Closes the active MT and activates the next one
/// - there are two scenarios in which this is required/allowed:
///     1. the active MT is full
///     2. the active MT is not full but the remaining places in the MT are < than the batching rate of the next commitment in the commitment queue
pub fn reset_active_merkle_tree(
    storage_account: &mut StorageAccount,
    queue: &mut CommitmentQueueAccount,
    active_nullifier_account: &mut NullifierAccount,

    active_merkle_tree_index: u64,
) -> ProgramResult {
    guard!(storage_account.get_trees_count() == active_merkle_tree_index, InvalidInstructionData);

    let queue = CommitmentQueue::new(queue);
    guard!(is_mt_full(storage_account, &queue)?, MerkleTreeIsNotFullYet);

    storage_account.reset();
    storage_account.set_trees_count(&(active_merkle_tree_index.checked_add(1).ok_or(MATH_ERR)?));
    active_nullifier_account.set_root(&storage_account.get_root());

    Ok(())
}

/// Opens a `PendingNullifierHashesAccount` and activates the single sub-account for a specific MT
pub fn open_pending_nullifier_hashes_account<'a>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    data_account: &AccountInfo,

    mt_index: u64,
) -> ProgramResult {
    open_pda_account_with_offset::<PendingNullifierHashesAccount>(payer, pda_account, mt_index)?;
    setup_sub_account::<PendingNullifierHashesAccount, 1>(pda_account, data_account, 0, false)?;

    // Set map size to zero
    reset_map_sub_account(data_account);

    Ok(())
}

fn is_mt_full(
    storage_account: &StorageAccount,
    queue: &CommitmentQueue,
) -> Result<bool, ProgramError> {
    let commitments = storage_account.get_next_commitment_ptr() as usize;
    if commitments >= MT_COMMITMENT_COUNT {
        return Ok(true)
    }

    if commitments + queue.next_batch()?.0.len() >= MT_COMMITMENT_COUNT {
        return Ok(true)
    }

    Ok(false)
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
    panic!("N-SMT not implemented yet");
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
    _authority: &AccountInfo,
    _governor_account: &mut GovernorAccount,
    _commitment_queue: &CommitmentQueueAccount,

    _fee_version: u64,
    _batching_rate: u32,
) -> ProgramResult {
    todo!("Not implemented yet");
    // TODO: changes in the batching rate are only possible when checking the commitment queue
    // TODO: fee changes require empty queues
}

/// Setup a new `FeeAccount`
/// - Note: there is no way of upgrading the program fees atm
pub fn init_new_fee_version<'a>(
    payer: &AccountInfo<'a>,
    governor: &mut GovernorAccount,
    new_fee: &AccountInfo<'a>,

    fee_version: u64,
    program_fee: ProgramFee,
) -> ProgramResult {
    // Note: we have no upgrade-authroity check here since with the current setup it's impossible to have a fee version higher than zero, so will be added once that changes
    guard!(fee_version == governor.get_fee_version(), InvalidFeeVersion);
    guard!(program_fee.is_valid(), InvalidInstructionData);
    open_pda_account_with_offset::<FeeAccount>(payer, new_fee, fee_version)?;

    let mut data = new_fee.data.borrow_mut();
    let mut fee = FeeAccount::new(&mut data[..])?;
    fee.set_program_fee(&program_fee);
    governor.set_program_fee(&program_fee);

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

    if account_data.pubkeys[sub_account_index].option().is_some() {
        return Err(SubAccountAlreadyExists.into())
    }

    verify_extern_data_account(sub_account, T::ACCOUNT_SIZE, check_zeroness)?;
    account_data.pubkeys[sub_account_index] = ElusivOption::Some(*sub_account.key);
    MultiAccountAccountData::override_slice(&account_data, data)?;

    // Check that the sub-account is not already in use (=> global duplicate check)
    let data = &mut sub_account.data.borrow_mut()[..];
    let mut acc = SubAccount::new(data);
    guard!(!acc.get_is_in_use(), InvalidAccount);
    acc.set_is_in_use(true);

    Ok(())
}

fn reset_map_sub_account(sub_account: &AccountInfo) {
    let data = &mut sub_account.data.borrow_mut()[..];
    let acc = SubAccount::new(data);
    for b in acc.data.iter_mut().take(4) { *b = 0 }
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use assert_matches::assert_matches;
    use solana_program::pubkey::Pubkey;
    use crate::{macros::account, state::{program_account::{PDAAccount, SizedAccount, MultiAccountProgramAccount}, queue::RingQueue}, processor::CommitmentHashRequest};

    #[test]
    fn test_open_single_instance_account() {
        let valid_pda = PoolAccount::find(None).0;
        let invalid_pda = PoolAccount::find(Some(0)).0;

        let payer_pk = Pubkey::new_unique();
        account!(payer, payer_pk, vec![]);

        // Invalid PDA
        account!(pda_account, invalid_pda, vec![]);
        assert_matches!(
            open_single_instance_account(&payer, &pda_account, SingleInstancePDAAccountKind::PoolAccount),
            Err(_)
        );

        // Valid PDA
        account!(pda_account, valid_pda, vec![]);
        assert_matches!(
            open_single_instance_account(&payer, &pda_account, SingleInstancePDAAccountKind::PoolAccount),
            Ok(())
        );
    }

    #[test]
    fn test_open_multi_instance_account() {
        let valid_pda = BaseCommitmentQueueAccount::find(Some(0)).0;
        account!(pda_account, valid_pda, vec![]);

        let payer_pk = Pubkey::new_unique();
        account!(payer, payer_pk, vec![]);

        // Invalid offset
        assert_matches!(
            open_multi_instance_account(&payer, &pda_account, MultiInstancePDAAccountKind::BaseCommitmentQueueAccount, 1),
            Err(_)
        );

        // Valid offset
        account!(pda_account, valid_pda, vec![]);
        assert_matches!(
            open_multi_instance_account(&payer, &pda_account, MultiInstancePDAAccountKind::BaseCommitmentQueueAccount, 0),
            Ok(_)
        );
    }

    #[test]
    fn test_enable_storage_sub_account() {
        let mut data = vec![0; StorageAccount::SIZE];
        let mut storage_account = StorageAccount::new(&mut data, HashMap::new()).unwrap();
        let mut d = storage_account.get_multi_account_data();
        d.pubkeys[0] = ElusivOption::Some(Pubkey::new_unique());
        storage_account.set_multi_account_data(&d);

        let pk = StorageAccount::find(None).0;
        account!(storage, pk, data);

        // Account has invalid size
        let pk = Pubkey::new_unique();
        account!(sub_account, pk, vec![0; StorageAccount::ACCOUNT_SIZE - 1]);
        assert_matches!(enable_storage_sub_account(&storage, &sub_account, 0), Err(_));

        // Account has already been setup
        let pk = Pubkey::new_unique();
        account!(sub_account, pk, vec![0; StorageAccount::ACCOUNT_SIZE]);
        assert_matches!(enable_storage_sub_account(&storage, &sub_account, 0), Err(_));

        // Success at different index
        assert_matches!(enable_storage_sub_account(&storage, &sub_account, 3), Ok(()));
    }

    #[test]
    fn test_enable_nullifier_sub_account() {
        let mut data = vec![0; NullifierAccount::SIZE];
        let mut nullifier_account = NullifierAccount::new(&mut data, HashMap::new()).unwrap();
        let mut d = nullifier_account.get_multi_account_data();
        d.pubkeys[0] = ElusivOption::Some(Pubkey::new_unique());
        nullifier_account.set_multi_account_data(&d);

        let pk = NullifierAccount::find(Some(0)).0;
        account!(nullifier, pk, data);

        // Account has invalid size
        let pk = Pubkey::new_unique();
        account!(sub_account, pk, vec![0; NullifierAccount::ACCOUNT_SIZE - 1]);
        assert_matches!(enable_nullifier_sub_account(&nullifier, &sub_account, 0, 0), Err(_));

        // Account has already been setup
        let pk = Pubkey::new_unique();
        account!(sub_account, pk, vec![0; NullifierAccount::ACCOUNT_SIZE]);
        assert_matches!(enable_nullifier_sub_account(&nullifier, &sub_account, 0, 0), Err(_));

        // Success at different index
        assert_matches!(enable_nullifier_sub_account(&nullifier, &sub_account, 0, 3), Ok(()));
    }

    #[test]
    fn test_open_pending_nullifier_hashes_account() {
        let payer_pk = Pubkey::new_unique();
        let sub_account_pk = Pubkey::new_unique();
        let pk = PendingNullifierHashesAccount::find(Some(0)).0;

        account!(payer, payer_pk, vec![]);

        // Sub-Account has invalid size
        account!(account, pk, vec![0; PendingNullifierHashesAccount::SIZE]);
        account!(sub_account, sub_account_pk, vec![0; PendingNullifierHashesAccount::ACCOUNT_SIZE - 1]);
        assert_matches!(open_pending_nullifier_hashes_account(&payer, &account, &sub_account, 0), Err(_));

        // Account has already been setup
        let mut data = vec![0; PendingNullifierHashesAccount::SIZE];
        let mut account = PendingNullifierHashesAccount::new(&mut data, HashMap::new()).unwrap();
        let mut d = account.get_multi_account_data();
        d.pubkeys[0] = ElusivOption::Some(Pubkey::new_unique());
        account.set_multi_account_data(&d);
        account!(account, pk, data);
        account!(sub_account, sub_account_pk, vec![0; PendingNullifierHashesAccount::ACCOUNT_SIZE]);
        assert_matches!(open_pending_nullifier_hashes_account(&payer, &account, &sub_account, 0), Err(_));

        // Success if empty
        let data = vec![0; PendingNullifierHashesAccount::SIZE];
        account!(account, pk, data);
        assert_matches!(open_pending_nullifier_hashes_account(&payer, &account, &sub_account, 0), Ok(()));
    }

    #[test]
    fn test_is_mt_full() {
        let mut data = vec![0; StorageAccount::SIZE];
        let mut storage_account = StorageAccount::new(&mut data, HashMap::new()).unwrap();
        storage_account.set_next_commitment_ptr(&(MT_COMMITMENT_COUNT as u32));

        let mut q_data = vec![0; CommitmentQueueAccount::SIZE];
        let mut queue = CommitmentQueueAccount::new(&mut q_data).unwrap();
        let mut queue = CommitmentQueue::new(&mut queue);
        queue.enqueue(CommitmentHashRequest { min_batching_rate: 1, commitment: [0; 32], fee_version: 0 }).unwrap();
        queue.enqueue(CommitmentHashRequest { min_batching_rate: 1, commitment: [0; 32], fee_version: 0 }).unwrap();

        assert!(is_mt_full(&storage_account, &queue).unwrap());

        storage_account.set_next_commitment_ptr(&(MT_COMMITMENT_COUNT as u32 - 3));
        assert!(!is_mt_full(&storage_account, &queue).unwrap());

        storage_account.set_next_commitment_ptr(&(MT_COMMITMENT_COUNT as u32 - 2));
        assert!(is_mt_full(&storage_account, &queue).unwrap());
    }

    #[test]
    #[should_panic]
    fn test_archive_closed_merkle_tree() {
        test_account_info!(payer, 0);
        let mut data = vec![0; StorageAccount::SIZE];
        let mut storage_account = StorageAccount::new(&mut data, HashMap::new()).unwrap();
        let mut data = vec![0; NullifierAccount::SIZE];
        let mut nullifier_account = NullifierAccount::new(&mut data, HashMap::new()).unwrap();
        test_account_info!(archived_tree_account, 0);

        archive_closed_merkle_tree(&payer, &mut storage_account, &mut nullifier_account, &archived_tree_account, 0).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_upgrade_governor_state() {
        test_account_info!(authority, 0);
        zero_account!(mut governor_account, GovernorAccount);
        zero_account!(commitment_queue, CommitmentQueueAccount);

        upgrade_governor_state(&authority, &mut governor_account, &commitment_queue, 1, 1).unwrap();
    }

    #[test]
    fn test_verify_extern_data_account() {
        let pk = Pubkey::new_unique();

        // Mismatched size
        account!(account, pk, vec![0; 100]);
        assert_matches!(verify_extern_data_account(&account, 99, true), Err(_));

        // Non-zero
        account!(account, pk, vec![1; 100]);
        assert_matches!(verify_extern_data_account(&account, 100, true), Err(_));

        // Ignore zero
        assert_matches!(verify_extern_data_account(&account, 100, false), Ok(()));

        // Check zero
        account!(account, pk, vec![0; 100]);
        assert_matches!(verify_extern_data_account(&account, 100, true), Ok(()));
    }
}