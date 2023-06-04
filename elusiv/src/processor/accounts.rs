use super::utils::*;
use crate::bytes::{is_zero, BorshSerDeSized, ElusivOption};
use crate::commitment::DEFAULT_COMMITMENT_BATCHING_RATE;
use crate::error::ElusivError;
use crate::macros::*;
use crate::state::commitment::{
    BaseCommitmentBufferAccount, CommitmentBufferAccount, CommitmentHashingAccount,
    CommitmentQueue, CommitmentQueueAccount,
};
use crate::state::metadata::{MetadataAccount, MetadataQueueAccount};
use crate::state::queue::RingQueue;
use crate::state::{
    fee::{FeeAccount, ProgramFee},
    governor::{FeeCollectorAccount, GovernorAccount, PoolAccount},
    nullifier::{NullifierAccount, NullifierChildAccount},
    queue::Queue,
    storage::{StorageAccount, MT_COMMITMENT_COUNT},
};
use crate::{bytes::usize_as_u32_safe, map::ElusivMap};
use elusiv_types::{
    split_child_account_data_mut, ChildAccount, ChildAccountConfig, ParentAccount, SizedAccount,
    UnverifiedAccountInfo,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError, rent::Rent,
    sysvar::Sysvar,
};

/// Opens one single instance [`elusiv_types::PDAAccount`], as long this PDA does not already exist
pub fn open_single_instance_accounts<'a, 'b>(
    payer: &AccountInfo<'b>,
    pool_account: UnverifiedAccountInfo<'a, 'b>,
    fee_collector_account: UnverifiedAccountInfo<'a, 'b>,
    commitment_hashing_account: UnverifiedAccountInfo<'a, 'b>,
    commitment_queue_account: UnverifiedAccountInfo<'a, 'b>,
    storage_account: UnverifiedAccountInfo<'a, 'b>,
    base_commitment_buffer_account: UnverifiedAccountInfo<'a, 'b>,
) -> ProgramResult {
    open_pda_account_without_offset::<PoolAccount>(
        &crate::id(),
        payer,
        pool_account.get_unsafe(),
        None,
    )?;
    open_pda_account_without_offset::<FeeCollectorAccount>(
        &crate::id(),
        payer,
        fee_collector_account.get_unsafe(),
        None,
    )?;
    open_pda_account_without_offset::<CommitmentHashingAccount>(
        &crate::id(),
        payer,
        commitment_hashing_account.get_unsafe(),
        None,
    )?;
    open_pda_account_without_offset::<CommitmentQueueAccount>(
        &crate::id(),
        payer,
        commitment_queue_account.get_unsafe(),
        None,
    )?;
    open_pda_account_without_offset::<StorageAccount>(
        &crate::id(),
        payer,
        storage_account.get_unsafe(),
        None,
    )?;
    open_pda_account_without_offset::<BaseCommitmentBufferAccount>(
        &crate::id(),
        payer,
        base_commitment_buffer_account.get_unsafe(),
        None,
    )?;

    Ok(())
}

pub fn open_nullifier_account<'b>(
    payer: &AccountInfo<'b>,
    nullifier_account: UnverifiedAccountInfo<'_, 'b>,

    mt_index: u32,
) -> ProgramResult {
    open_pda_account_with_offset::<NullifierAccount>(
        &crate::id(),
        payer,
        nullifier_account.get_unsafe(),
        mt_index,
        None,
    )
}

/// Enables the supplied child-account for the [`StorageAccount`]
pub fn enable_storage_child_account(
    storage_account: &mut StorageAccount,
    child_account: &AccountInfo,

    child_index: u32,
) -> ProgramResult {
    // Note: we don't zero-check these accounts, since we will never access data that has not been set by the program
    setup_child_account(
        storage_account,
        child_account,
        child_index as usize,
        false,
        None,
    )
}

/// Enables the supplied child-account for a [`NullifierAccount`]
///
/// # Notes
///
/// Requires a prior call to [`open_multi_instance_account`].
///
/// The [`NullifierAccount`] will be useless until the MT with `index = merkle_tree_index - 1` is closed.
pub fn enable_nullifier_child_account(
    nullifier_account: &mut NullifierAccount,
    child_account: &AccountInfo,

    _merkle_tree_index: u32,
    child_index: u32,
) -> ProgramResult {
    // Note: we don't zero-check these accounts, BUT we need to manipulate the maps we store in each account and set the size to zero
    setup_child_account(
        nullifier_account,
        child_account,
        child_index as usize,
        false,
        None,
    )?;

    // Set map size to zero
    reset_map_child_account::<NullifierChildAccount>(child_account)?;

    Ok(())
}

/// Enables the supplied child-account for the [`MetadataAccount`]
pub fn enable_metadata_child_account(
    metadata_account: &mut MetadataAccount,
    child_account: &AccountInfo,

    child_index: u32,
) -> ProgramResult {
    // Note: we don't zero-check these accounts, since we will never access data that has not been set by the program
    setup_child_account(
        metadata_account,
        child_account,
        child_index as usize,
        false,
        None,
    )
}

/// Closes the active MT and activates the next one
///
/// # Notes
///
/// There are two scenarios in which this is required/allowed:
///     1. the active MT is full
///     2. the active MT is not full but the remaining places in the MT are < than the batching rate of the next commitment in the commitment queue
pub fn reset_active_merkle_tree(
    storage_account: &mut StorageAccount,
    queue: &mut CommitmentQueueAccount,
    active_nullifier_account: &mut NullifierAccount,

    active_merkle_tree_index: u32,
) -> ProgramResult {
    guard!(
        storage_account.get_trees_count() == active_merkle_tree_index,
        ElusivError::InvalidInstructionData
    );

    let queue = CommitmentQueue::new(queue);
    guard!(
        is_mt_full(storage_account, &queue)?,
        ElusivError::MerkleTreeIsNotFullYet
    );

    storage_account.set_trees_count(&(active_merkle_tree_index.checked_add(1).ok_or(MATH_ERR)?));
    active_nullifier_account.set_root(&storage_account.get_root()?);
    storage_account.reset();

    Ok(())
}

pub fn create_new_accounts_v1<'a, 'b>(
    payer: &AccountInfo<'b>,
    commitment_buffer_account: UnverifiedAccountInfo<'a, 'b>,
    metadata_queue: UnverifiedAccountInfo<'a, 'b>,
    metadata_account: UnverifiedAccountInfo<'a, 'b>,
    storage_account: &StorageAccount,
    commitment_hashing_account: &CommitmentHashingAccount,
    commitment_queue_account: &mut CommitmentQueueAccount,
) -> ProgramResult {
    // TODO: add this functionality to the single instance initialization
    open_pda_account_without_offset::<CommitmentBufferAccount>(
        &crate::id(),
        payer,
        commitment_buffer_account.get_unsafe(),
        None,
    )?;

    open_pda_account_without_offset::<MetadataQueueAccount>(
        &crate::id(),
        payer,
        metadata_queue.get_unsafe(),
        None,
    )?;

    open_pda_account_without_offset::<MetadataAccount>(
        &crate::id(),
        payer,
        metadata_account.get_unsafe(),
        None,
    )?;

    // Ensure that there is no commitment that is being hashed
    guard!(
        !commitment_hashing_account.get_is_active(),
        ElusivError::InvalidAccountState
    );

    // Ensure that there is no commitment awaiting insertion
    let commitment_queue = CommitmentQueue::new(commitment_queue_account);
    guard!(
        commitment_queue.is_empty(),
        ElusivError::InvalidAccountState
    );

    // Transfer the commitment index from the storage-account (required for the upgrade of the deployed program)
    let next_commitment_ptr = storage_account.get_next_commitment_ptr();
    let metadata_account = metadata_account.get_unsafe();
    pda_account!(mut metadata_account, MetadataAccount, metadata_account);
    metadata_account.set_next_metadata_ptr(&next_commitment_ptr);

    Ok(())
}

fn is_mt_full(
    storage_account: &StorageAccount,
    queue: &CommitmentQueue,
) -> Result<bool, ProgramError> {
    if storage_account.is_full() {
        return Ok(true);
    }

    let commitments_count = storage_account.get_next_commitment_ptr() as usize;
    let queue_len = queue.next_batch()?.0.len();
    if commitments_count + queue_len >= MT_COMMITMENT_COUNT {
        return Ok(true);
    }

    Ok(false)
}

/// Archives a closed MT by creating creating a N-SMT in an [`ArchivedNullifierAccount`]
pub fn archive_closed_merkle_tree<'a>(
    _payer: &AccountInfo<'a>,
    storage_account: &mut StorageAccount,
    _nullifier_account: &mut NullifierAccount,
    _archived_nullifier_account: &AccountInfo<'a>,

    closed_merkle_tree_index: u32,
) -> ProgramResult {
    guard!(
        storage_account.get_trees_count() > closed_merkle_tree_index,
        ElusivError::InvalidInstructionData
    );
    panic!("N-SMT not implemented yet");
}

/// Setup the [`GovernorAccount`] with the default values
///
/// # Note
///
/// There is no way of upgrading it atm.
pub fn setup_governor_account<'b>(
    payer: &AccountInfo<'b>,
    governor_account: UnverifiedAccountInfo<'_, 'b>,
) -> ProgramResult {
    open_pda_account_without_offset::<GovernorAccount>(
        &crate::id(),
        payer,
        governor_account.get_unsafe(),
        None,
    )?;

    pda_account!(mut governor, GovernorAccount, governor_account.get_unsafe());
    governor.set_commitment_batching_rate(&usize_as_u32_safe(DEFAULT_COMMITMENT_BATCHING_RATE));

    Ok(())
}

/// Changes the state of the [`GovernorAccount`]
pub fn upgrade_governor_state(
    _authority: &AccountInfo,
    _governor_account: &mut GovernorAccount,
    _commitment_queue: &CommitmentQueueAccount,

    _fee_version: u32,
    _batching_rate: u32,
) -> ProgramResult {
    todo!("Not implemented yet");
    // TODO: changes in the batching rate are only possible when checking the commitment queue
    // TODO: fee changes require empty queues
}

/// Setup a new [`FeeAccount`]
///
/// # Note
///
/// There is no way of upgrading the program fees atm.
pub fn init_new_fee_version<'b>(
    payer: &AccountInfo<'b>,
    governor: &mut GovernorAccount,
    mut new_fee_account: UnverifiedAccountInfo<'_, 'b>,

    fee_version: u32,
    program_fee: ProgramFee,
) -> ProgramResult {
    // Note: we have no upgrade-authroity check here since with the current setup it's impossible to have a fee version higher than zero, so will be added once that changes
    guard!(
        fee_version == governor.get_fee_version(),
        ElusivError::InvalidFeeVersion
    );
    guard!(program_fee.is_valid(), ElusivError::InvalidInstructionData);

    open_pda_account_with_offset::<FeeAccount>(
        &crate::id(),
        payer,
        new_fee_account.get_unsafe_and_set_is_verified(),
        fee_version,
        None,
    )?;

    pda_account!(mut fee_account, FeeAccount, new_fee_account.get_safe()?);
    fee_account.set_program_fee(&program_fee);
    governor.set_program_fee(&program_fee);

    Ok(())
}

/// Closes a program owned account in devnet and localhost
///
/// # Note
///
/// - `signer` needs to be the program's keypair
/// - `recipient` receives the accounts Lamports
#[cfg(not(feature = "mainnet"))]
pub fn close_program_account<'a>(
    signer: &AccountInfo,
    recipient: &AccountInfo<'a>,
    program_account: &AccountInfo<'a>,
) -> ProgramResult {
    assert!(!cfg!(feature = "mainnet"));
    assert_eq!(*signer.key, crate::ID);

    elusiv_utils::close_account(recipient, program_account)
}

/// Verifies a single user-supplied [`ChildAccount`] and then saves it's pubkey in the `parent_account`
///
/// # Note
///
/// If `size` is manually supplied (not the default [`C::SIZE`] is used) [`elusiv_types::ChildAccountConfig::SIZE`] needs to be contained in the size.
pub fn setup_child_account<'a, 'b, 't, P: ParentAccount<'a, 'b, 't>>(
    parent_account: &mut P,
    child_account: &AccountInfo,
    child_index: usize,
    check_zeroness: bool,
    size: Option<usize>,
) -> ProgramResult {
    if parent_account.get_child_pubkey(child_index).is_some() {
        return Err(ElusivError::ChildAccountAlreadyExists.into());
    }

    verify_extern_data_account(
        child_account,
        size.unwrap_or(<P::Child as SizedAccount>::SIZE),
        check_zeroness,
    )?;
    parent_account.set_child_pubkey(child_index, ElusivOption::Some(*child_account.key));
    P::Child::try_start_using_account(child_account)?;

    Ok(())
}

fn reset_map_child_account<C: ChildAccount>(child_account: &AccountInfo) -> ProgramResult {
    let data = &mut child_account.data.borrow_mut()[..];
    let (_, inner_data) = split_child_account_data_mut(data)?;
    let mut map = ElusivMap::<(), (), 1>::new(&mut inner_data[..ElusivMap::<(), (), 1>::SIZE]);
    map.reset();

    Ok(())
}

/// Verifies that an account with `data_len` > 10 KiB (non PDA) is formatted correctly
fn verify_extern_data_account(
    account: &AccountInfo,
    data_len: usize,
    check_zeroness: bool,
) -> ProgramResult {
    guard!(
        account.data_len() == data_len,
        ProgramError::InvalidAccountData
    );

    guard!(
        data_len >= ChildAccountConfig::SIZE,
        ProgramError::InvalidAccountData
    );

    if check_zeroness {
        guard!(
            is_zero(&account.data.borrow()[..]),
            ProgramError::InvalidAccountData
        );
    }

    // Check rent-exemption
    if cfg!(test) {
        // only unit-testing (since we have no ledger there)
        guard!(
            account.lamports() >= u32::MAX as u64,
            ProgramError::AccountNotRentExempt
        );
    } else {
        guard!(
            account.lamports() >= Rent::get()?.minimum_balance(data_len),
            ProgramError::AccountNotRentExempt
        );
    }

    // Check ownership
    guard!(*account.owner == crate::id(), ProgramError::IllegalOwner);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        macros::account_info,
        processor::CommitmentHashRequest,
        state::{program_account::SizedAccount, queue::RingQueue, storage::StorageChildAccount},
        types::U256,
    };
    use elusiv_types::ProgramAccount;
    use solana_program::{pubkey::Pubkey, system_program};

    #[test]
    fn test_enable_storage_child_account() {
        let mut data = vec![0; StorageAccount::SIZE];
        let mut storage_account = StorageAccount::new(&mut data).unwrap();

        // Account has invalid size
        account_info!(
            child_account,
            Pubkey::new_unique(),
            vec![0; StorageChildAccount::SIZE - 1]
        );
        assert_eq!(
            enable_storage_child_account(&mut storage_account, &child_account, 0),
            Err(ProgramError::InvalidAccountData)
        );

        storage_account.set_child_pubkey(0, ElusivOption::Some(Pubkey::new_unique()));

        // Account has already been setup
        account_info!(
            child_account,
            Pubkey::new_unique(),
            vec![0; StorageChildAccount::SIZE]
        );
        assert_eq!(
            enable_storage_child_account(&mut storage_account, &child_account, 0),
            Err(ElusivError::ChildAccountAlreadyExists.into())
        );

        // Success at different index
        assert_eq!(
            enable_storage_child_account(&mut storage_account, &child_account, 3),
            Ok(())
        );
        assert_eq!(child_account.data.borrow()[0], 1);

        // Account already is use
        assert_eq!(
            enable_storage_child_account(&mut storage_account, &child_account, 1),
            Err(ProgramError::AccountAlreadyInitialized)
        );
    }

    #[test]
    fn test_enable_nullifier_child_account() {
        let mut data = vec![0; NullifierAccount::SIZE];
        let mut nullifier_account = NullifierAccount::new(&mut data).unwrap();

        // Account has invalid size
        account_info!(
            child_account,
            Pubkey::new_unique(),
            vec![0; NullifierChildAccount::SIZE - 1]
        );
        assert_eq!(
            enable_nullifier_child_account(&mut nullifier_account, &child_account, 0, 0),
            Err(ProgramError::InvalidAccountData)
        );

        nullifier_account.set_child_pubkey(0, ElusivOption::Some(Pubkey::new_unique()));

        // Account has already been setup
        account_info!(
            child_account,
            Pubkey::new_unique(),
            vec![0; NullifierChildAccount::SIZE]
        );
        assert_eq!(
            enable_nullifier_child_account(&mut nullifier_account, &child_account, 0, 0),
            Err(ElusivError::ChildAccountAlreadyExists.into())
        );

        // Success at different index with
        assert_eq!(
            enable_nullifier_child_account(&mut nullifier_account, &child_account, 0, 3),
            Ok(())
        );
        assert_eq!(child_account.data.borrow()[0], 1);

        // Account already is use
        assert_eq!(
            enable_nullifier_child_account(&mut nullifier_account, &child_account, 0, 1),
            Err(ProgramError::AccountAlreadyInitialized)
        );
    }

    #[test]
    fn test_is_mt_full() {
        let mut data = vec![0; StorageAccount::SIZE];
        let mut storage_account = StorageAccount::new(&mut data).unwrap();
        storage_account.set_next_commitment_ptr(&(MT_COMMITMENT_COUNT as u32));

        let mut q_data = vec![0; CommitmentQueueAccount::SIZE];
        let mut queue = CommitmentQueueAccount::new(&mut q_data).unwrap();
        let mut queue = CommitmentQueue::new(&mut queue);
        queue
            .enqueue(CommitmentHashRequest {
                min_batching_rate: 1,
                commitment: [0; 32],
                fee_version: 0,
            })
            .unwrap();
        queue
            .enqueue(CommitmentHashRequest {
                min_batching_rate: 1,
                commitment: [0; 32],
                fee_version: 0,
            })
            .unwrap();

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
        let mut storage_account = StorageAccount::new(&mut data).unwrap();
        let mut data = vec![0; NullifierAccount::SIZE];
        let mut nullifier_account = NullifierAccount::new(&mut data).unwrap();
        test_account_info!(archived_tree_account, 0);

        archive_closed_merkle_tree(
            &payer,
            &mut storage_account,
            &mut nullifier_account,
            &archived_tree_account,
            0,
        )
        .unwrap();
    }

    #[test]
    #[should_panic]
    fn test_upgrade_governor_state() {
        test_account_info!(authority, 0);
        zero_program_account!(mut governor_account, GovernorAccount);
        zero_program_account!(commitment_queue, CommitmentQueueAccount);

        upgrade_governor_state(&authority, &mut governor_account, &commitment_queue, 1, 1).unwrap();
    }

    #[test]
    fn test_verify_extern_data_account() {
        let pk = Pubkey::new_unique();

        // Mismatched size
        account_info!(account, pk, vec![0; 100]);
        assert_eq!(
            verify_extern_data_account(&account, 99, true),
            Err(ProgramError::InvalidAccountData)
        );

        // Non-zero
        account_info!(account, pk, vec![1; 100]);
        assert_eq!(
            verify_extern_data_account(&account, 100, true),
            Err(ProgramError::InvalidAccountData)
        );

        // Ignore zero
        assert_eq!(verify_extern_data_account(&account, 100, false), Ok(()));

        // Invalid owner
        account_info!(account, pk, vec![0; 100], system_program::id(), false);
        assert_eq!(
            verify_extern_data_account(&account, 100, true),
            Err(ProgramError::IllegalOwner)
        );

        // Check zero
        account_info!(account, pk, vec![0; 100]);
        assert_eq!(verify_extern_data_account(&account, 100, true), Ok(()));
    }

    struct TestChildAccount;

    impl ChildAccount for TestChildAccount {
        const INNER_SIZE: usize = 0;
    }

    #[test]
    fn test_reset_map_child_account() {
        type Map<'a> = ElusivMap<'a, U256, (), 1>;

        let pk = Pubkey::new_unique();
        let mut data = vec![0; Map::SIZE];
        let mut map = Map::new(&mut data[..]);
        map.try_insert_default([1; 32]).unwrap();
        assert!(map.is_full());

        let mut d = vec![1];
        d.extend(data);
        account_info!(map_account, pk, d);
        reset_map_child_account::<TestChildAccount>(&map_account).unwrap();

        let data = &mut map_account.data.borrow_mut()[1..];
        let mut map = Map::new(data);
        assert!(map.is_empty());
    }
}
