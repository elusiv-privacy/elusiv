use crate::instruction;
use borsh::BorshDeserialize;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

crate::macros::declare_program_id!();

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

#[cfg(not(feature = "no-entrypoint"))]
use {default_env::default_env, solana_security_txt::security_txt};

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Elusiv-Warden-Network",
    project_url: "https://elusiv.io",
    contacts: "email:security@elusiv.io",
    policy: "https://github.com/elusiv-privacy/elusiv/blob/main/SECURITY.md",
    preferred_languages: "en",
    source_code: "https://github.com/elusiv-privacy/elusiv/blob/elusiv-warden-network",
    source_revision: default_env!("GITHUB_SHA", "")
}

#[cfg(not(tarpaulin_include))]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction::ElusivWardenNetworkInstruction::deserialize(&mut &instruction_data[..]) {
        Ok(instruction) => {
            instruction::ElusivWardenNetworkInstruction::process(program_id, accounts, instruction)
        }
        Err(_) => Err(ProgramError::InvalidInstructionData),
    }
}
