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
#[cfg(not(tarpaulin_include))]
pub fn process_instruction(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    if instruction_data.len() == 0 { return Err(ProgramError::InvalidInstructionData) }

    // We parse the ix length based on the first byte -> this allows our transactions to contain extra data, that the program can ignore but the client requires
    let len = instruction::ElusivInstruction::len(instruction_data[0]);
    match instruction::ElusivInstruction::try_from_slice(&instruction_data[..len + 1]) {
        Ok(instruction) => instruction::process_instruction(program_id, accounts, instruction),
        Err(_) => Err(ProgramError::InvalidInstructionData)
    }
}