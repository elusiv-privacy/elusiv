use super::instruction::ElusivInstruction;
use super::processor::Processor;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
};

#[cfg(target_arch = "bpf")]

entrypoint!(process_instruction);
pub fn process_instruction(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    msg!(
        "process_instruction: {}: {} accounts, data={:?}",
        program_id,
        accounts.len(),
        instruction_data
    );

    let instruction = ElusivInstruction::unpack(&instruction_data)?;
    Processor::process(program_id, &accounts, instruction)
}