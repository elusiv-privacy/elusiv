use super::instruction::ElusivInstruction;
use super::processor;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

#[cfg(target_arch = "bpf")]

solana_program::entrypoint!(process_instruction);
pub fn process_instruction(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    //solana_program::log::sol_log_compute_units();
    let instruction = ElusivInstruction::unpack(&instruction_data)?;
    //solana_program::log::sol_log_compute_units();
    processor::process(program_id, &accounts, instruction)
}