use solana_program::{
    pubkey::Pubkey,
    account_info::AccountInfo,
    entrypoint::ProgramResult,
};
use crate::instruction;
use crate::bytes::SerDe;

solana_program::declare_id!("AQJN5bDobGyooyURYGfhFCWK6pfEdEf17gLxixEvY6y7");

solana_program::entrypoint!(process_instruction);
pub fn process_instruction(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let data = &mut &instruction_data;
    let instruction = instruction::ElusivInstruction::deserialize(data);

    instruction::process_instruction(program_id, accounts, instruction)
}