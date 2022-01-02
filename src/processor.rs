use super::instruction::{
    ElusivInstruction,
    ElusivInstruction::Deposit,
    ElusivInstruction::Withdraw,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    program_error::ProgramError::{
        InvalidAccountData,
        IllegalOwner,
        IncorrectProgramId
    },
    account_info::next_account_info,
    system_instruction::transfer,
    program::invoke_signed,
    system_program
};

pub struct Processor;

impl Processor {
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], instruction: ElusivInstruction) -> ProgramResult {
        match instruction {
            Deposit { amount } =>  {
                Self::deposit(program_id, &accounts, amount)
            },
            Withdraw { amount } => {
                Self::withdraw(program_id, &accounts, amount)
            }
        }
    }

    fn deposit(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        // 0. [signer, writable] Signer and Sender
        let sender = next_account_info(account_info_iter)?;
        if !sender.is_signer { return Err(InvalidAccountData); }
        if !sender.is_writable { return Err(InvalidAccountData); }

        // 1. [owned, writable] Program main account
        let bank = next_account_info(account_info_iter)?;
        if bank.owner != program_id { return Err(IllegalOwner); }

        // 2. System program
        let system_program = next_account_info(account_info_iter)?;
        if *system_program.key != system_program::id() { return Err(IncorrectProgramId); }

        // TODO: Check if commitment is unique

        // TODO: Insert commitment in merkle tree

        // Transfer funds using system program
        let instruction = transfer(&sender.key, &bank.key, amount);
        let (_, bump_seed) = Pubkey::find_program_address(&[b"deposit"], program_id);
        invoke_signed(
            &instruction,
            &[
                sender.clone(),
                bank.clone(),
            ],
            &[&[&b"deposit"[..], &[bump_seed]]],
        )?;

        Ok(())
    }

    fn withdraw(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        // 0. [signer] Signer
        let sender = next_account_info(account_info_iter)?;
        if !sender.is_signer { return Err(InvalidAccountData); }

        // 1. [writable] Recipient
        let recipient = next_account_info(account_info_iter)?;
        if !sender.is_writable { return Err(InvalidAccountData); }

        // 2. [owned, writable] Program main account
        let bank = next_account_info(account_info_iter)?;
        if bank.owner != program_id { return Err(IllegalOwner); }

        // TODO: Check if nullifier does not already exist

        // TODO: Validate proof

        // TODO: Check if new commitment does not already exist

        // TODO: Save nullifier and commitment

        // Transfer funds using owned bank account
        **bank.try_borrow_mut_lamports()? -= amount;
        **recipient.try_borrow_mut_lamports()? += amount;

        Ok(())
    }
}