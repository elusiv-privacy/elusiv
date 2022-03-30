use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    program::invoke_signed,
    system_instruction,
    sysvar::Sysvar,
    rent::Rent,
};
use crate::state::{
    StorageAccount,
    NullifierAccount,
    ArchiveAccount,
};
use crate::macros::guard;
use crate::error::ElusivError::{
    NullifierAccountDoesNotExist,
    InvalidNullifierAccount,
    MerkleTreeIsNotFullYet,
};
use crate::state::TREE_LEAF_COUNT;

// If there is no active nullifier account, this creates a new one and assigns it to the Storage account
pub fn init_storage<'a>(
    storage_account: &mut StorageAccount,
    reserve: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    bump_seed: u8,
) -> ProgramResult {
    // Check that nullifier account is `None`
    guard!(
        matches!(storage_account.get_nullifier_account(), None),
        NullifierAccountDoesNotExist
    );

    // Create new nullifier account
    let space = NullifierAccount::TOTAL_SIZE;
    let rent_lamports = Rent::get()?.minimum_balance(space);
    invoke_signed(
        &system_instruction::create_account(
            &reserve.key,
            &pda_account.key,
            rent_lamports,
            space as u64,
            &crate::id(),
        ),
        &[
            reserve.clone(),
            pda_account.clone()
        ],
        &[&[&reserve.key.as_ref(), &[bump_seed]]]
    )?;
    
    // Save nullifier account in storage account
    storage_account.set_nullifier_account(Some(pda_account.key.to_bytes()));

    Ok(())
}

// If the storage account (the tree) is full, the tree is emptied and the nullifier account is archived
pub fn archive_storage(
    storage_account: &mut StorageAccount,
    archive_account: &mut ArchiveAccount,
    nullifier_account: &mut NullifierAccount,
) -> ProgramResult {
    // Check that storage account is full
    guard!(
        storage_account.get_next_commitment() as usize >= TREE_LEAF_COUNT,
        MerkleTreeIsNotFullYet
    );

    // Check that nullifier account is active
    guard!(
        storage_account.try_get_nullifier_account()? == nullifier_account.get_key(),
        InvalidNullifierAccount
    );

    // Save root in nullifier account
    let root = storage_account.get_tree_node(0, 0)?;
    nullifier_account.set_root(&root);

    // Save nullifier account + root in archive account
    archive_account.archive_nullifier_account(nullifier_account.get_key(), root)?;

    // Reset storage account to zero
    storage_account.reset();

    Ok(())
}