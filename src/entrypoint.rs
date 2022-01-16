use super::instruction::ElusivInstruction;
use super::processor::Processor;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

#[cfg(target_arch = "bpf")]

solana_program::entrypoint!(process_instruction);
pub fn process_instruction(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let instruction = ElusivInstruction::unpack(&instruction_data)?;
    Processor::process(program_id, &accounts, instruction)
}