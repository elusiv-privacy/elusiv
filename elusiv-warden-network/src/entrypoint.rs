use solana_program::{
    pubkey::Pubkey,
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
};
use borsh::BorshDeserialize;
use crate::instruction;

crate::macros::program_id!();
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