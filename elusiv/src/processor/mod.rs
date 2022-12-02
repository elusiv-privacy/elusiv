mod proof;
mod commitment;
mod accounts;
mod utils;

pub use commitment::*;
pub use proof::*;
pub use accounts::*;
pub use utils::program_token_account_address;

pub fn nop() -> solana_program::entrypoint::ProgramResult {
    Ok(())
}