use solana_program::{entrypoint::ProgramResult, account_info::AccountInfo};
use crate::state::{
    StorageAccount,
    NullifierAccount,
    ArchiveAccount,
};

// If there is no active nullifier account, this creates a new one and assigns it to the Storage account
pub fn init_storage(
    storage_account: &mut StorageAccount,
    reserve: &AccountInfo,
) -> ProgramResult {
    // Check that nullifier account is None

    // Rent new nullifier account
    // Set nullifier account's key
    // Save nullifier account in storage account

    Ok(())
}

// If the storage account (the tree) is full, the tree is emptied and the nullifier account is archived
pub fn archive_storage(
    storage_account: &mut StorageAccount,
    archive_account: &mut ArchiveAccount,
    nullifier_account: &NullifierAccount,
) -> ProgramResult {
    // Check that storage account is full
    // Save root in nullifier account
    // Save nullifier account in archive account
    // Reset storage account

    Ok(())
}