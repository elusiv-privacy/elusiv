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
        account_size.try_into().unwrap(),
        program_id,
    );

    Ok((create_account_ix, new_account_keypair))
}