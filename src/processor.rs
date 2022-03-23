use solana_program::{
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    native_token::LAMPORTS_PER_SOL,
    account_info::AccountInfo,
};
use super::state::*;
use super::instruction::ElusivInstruction;
use elusiv_account::account;

const MINIMUM_AMOUNT: u64 = LAMPORTS_PER_SOL / 10;

pub fn process(_program_id: &Pubkey, accounts: &[AccountInfo], instruction: ElusivInstruction) -> ProgramResult {
    use ElusivInstruction::*;

    let account_info_iter = &mut accounts.iter();

    match instruction {
        Store { proof_data, unbound_commitment } => {
            account!(Sender, signer);
            account!(Storage);
            account!(Pool, pool);

            store(sender, storage_account, pool)
        },
        FinalizeSend => {
            finalize_send()
        }
    }
}

fn store(
    sender: &AccountInfo,
    storage_account: StorageAccount,
    pool: &AccountInfo,
) -> ProgramResult {

    Ok(())
}

fn finalize_send(

) -> ProgramResult {
    Ok(())
}