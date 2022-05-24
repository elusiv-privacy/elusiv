use solana_program::{
    pubkey::Pubkey,
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
};
use crate::instruction;
use borsh::BorshDeserialize;

solana_program::declare_id!("AQJN5bDobGyooyURYGfhFCWK6pfEdEf17gLxixEvY6y7");

solana_program::entrypoint!(process_instruction);
pub fn process_instruction(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    match instruction::ElusivInstruction::try_from_slice(instruction_data) {
        Ok(instruction) => instruction::process_instruction(program_id, accounts, instruction),
        Err(_) => Err(ProgramError::InvalidInstructionData)
    }
}