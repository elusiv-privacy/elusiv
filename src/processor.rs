use super::instruction::{
    ElusivInstruction,
    ElusivInstruction::Deposit,
    ElusivInstruction::Withdraw,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    program_error::ProgramError::InvalidInstructionData,
    account_info::next_account_info
};

pub struct Processor;

impl Processor {
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], instruction: ElusivInstruction) -> ProgramResult {
        //0. [signer, writable]
        let sender = next_account_info(&mut accounts.iter())?;
        if !sender.is_signer { return Err(InvalidInstructionData); }

        //1. [owned, writable]
        let bank = next_account_info(&mut accounts.iter())?;
        if bank.owner != program_id { return Err(InvalidInstructionData); }

        match instruction {
            Deposit { amount } =>  {
                Self::deposit(&sender, &bank, amount)
            },
            Withdraw => {
                Self::withdraw()
            }
        }
    }

    fn deposit(sender: &AccountInfo, bank: &AccountInfo, amount: u64) -> ProgramResult {
        //Check balance
        if sender.lamports() < amount { return Err(InvalidInstructionData); }

        //Transfer funds
        sender.lamports.borrow_mut().checked_sub(amount).ok_or(InvalidInstructionData)?;
        bank.lamports.borrow_mut().checked_add(amount).ok_or(InvalidInstructionData)?;

        Ok(())
    }

    fn withdraw() -> ProgramResult {
        Ok(())
    }
}