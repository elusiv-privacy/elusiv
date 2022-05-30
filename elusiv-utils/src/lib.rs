pub mod error;

use error::UtilsError;
use solana_program::{
    instruction::Instruction,
    pubkey::Pubkey,
    system_instruction,
};
use solana_sdk::{
    signature::Signer,
    signer::keypair::Keypair,
};
use elusiv::state::{
    StorageAccount,
    program_account::{MultiAccountAccount, MultiAccountAccountFields},
};

/// Creates a new data account with `account_size` data
/// - `amount` needs to be at least the amount required for rent-exemption
pub fn create_account(
    payer: &Keypair,
    program_id: &Pubkey,
    account_size: usize,
    amount: u64,
) -> Result<(Instruction, Keypair), UtilsError> {
    let new_account_keypair = Keypair::new();

    let create_account_ix = system_instruction::create_account(
        &payer.pubkey(),
        &new_account_keypair.pubkey(),
        amount,
        account_size as u64,
        program_id,
    );

    Ok((create_account_ix, new_account_keypair))
}

/// Returns the `StorageAccount::COUNT` storage account sub-accounts
pub fn get_storage_account_sub_accounts(
    storage_account_data: &[u8]
) -> Result<Vec<Pubkey>, UtilsError> {
    let acc = match MultiAccountAccountFields::<{StorageAccount::COUNT}>::new(storage_account_data) {
        Ok(v) => v,
        Err(_) => return Err(UtilsError::InvalidAccount)
    };
    let pks = acc.pubkeys;
    Ok(pks.iter().map(|x| Pubkey::new(x)).collect())
}