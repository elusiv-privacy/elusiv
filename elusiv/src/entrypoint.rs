use crate::instruction;
use borsh::BorshDeserialize;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

crate::macros::declare_program_id!();

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    match instruction::ElusivInstruction::deserialize(&mut &instruction_data[..]) {
        Ok(instruction) => {
            instruction::ElusivInstruction::process(program_id, accounts, instruction)
        }
        Err(_) => Err(ProgramError::InvalidInstructionData),
    }
}
