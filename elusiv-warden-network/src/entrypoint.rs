use solana_program::{
    pubkey::Pubkey,
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
};
use borsh::BorshDeserialize;
use crate::instruction;

solana_program::declare_id!("11111111111111111111111111111111");
solana_program::entrypoint!(process_instruction);

#[cfg(not(tarpaulin_include))]
pub fn process_instruction(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    match instruction::ElusivWardenNetworkInstruction::deserialize(&mut &instruction_data[..]) {
        Ok(instruction) => {
            instruction::ElusivWardenNetworkInstruction::process(program_id, accounts, instruction)
        }
        Err(_) => {
            Err(ProgramError::InvalidInstructionData)
        }
    }
}