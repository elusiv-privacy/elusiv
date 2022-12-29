use solana_program::{pubkey::Pubkey, account_info::AccountInfo, entrypoint::ProgramResult};

solana_program::entrypoint!(process_instruction);

pub fn process_instruction(_: &Pubkey, _: &[AccountInfo], _: &[u8]) -> ProgramResult {
    Ok(())
}