use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    system_program,
    program_error::ProgramError,
    system_instruction,
    program::invoke_signed,
    rent::Rent,
    sysvar::Sysvar,
};
use crate::bytes::BorshSerDeSized;
use crate::error::ElusivError::{
    InvalidAccount,
    InvalidInstructionData,
    InvalidAmount,
    InvalidAccountBalance
};
use crate::macros::guard;
use crate::state::program_account::{
    PDAAccountFields,
    PDAAccount,
    SizedAccount
};

/// Sends `lamports` from `sender` to `recipient`
pub fn send_with_system_program<'a>(
    sender: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    lamports: u64,
) -> ProgramResult {
    // Check that system_program is correct
    guard!(*system_program.key == system_program::ID, InvalidAccount);

    // Transfer funds from sender
    let instruction = solana_program::system_instruction::transfer(
        sender.key,
        recipient.key,
        lamports 
    );
    
    solana_program::program::invoke_signed(
        &instruction,
        &[
            sender.clone(),
            recipient.clone(),
            system_program.clone(),
        ],
        &[],
    )    
}

/// Sends from a program owned pool
pub fn send_from_pool<'a>(
    pool: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>,
    amount: u64,
) -> ProgramResult {
    **pool.try_borrow_mut_lamports()? = pool.lamports().checked_sub(amount)
        .ok_or_else(|| ProgramError::from(InvalidAmount))?;

    **recipient.try_borrow_mut_lamports()? = recipient.lamports().checked_add(amount)
        .ok_or_else(|| ProgramError::from(InvalidAmount))?;

    Ok(())
}

pub fn open_pda_account_with_offset<'a, T: PDAAccount + SizedAccount>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    pda_offset: u64,
) -> ProgramResult {
    let account_size = T::SIZE;
    let (pk, bump) = T::find(Some(pda_offset));
    let seed = vec![
        T::SEED.to_vec(),
        u64::to_le_bytes(pda_offset).to_vec(),
        vec![bump]
    ];
    let signers_seeds: Vec<&[u8]> = seed.iter().map(|x| &x[..]).collect();
    guard!(pk == *pda_account.key, InvalidInstructionData);

    create_pda_account(payer, pda_account, account_size, bump, &signers_seeds)
}

pub fn open_pda_account_without_offset<'a, T: PDAAccount + SizedAccount>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
) -> ProgramResult {
    let account_size = T::SIZE;
    let (pk, bump) = T::find(None);
    let seed = vec![
        T::SEED.to_vec(),
        vec![bump]
    ];
    let signers_seeds: Vec<&[u8]> = seed.iter().map(|x| &x[..]).collect();
    guard!(pk == *pda_account.key, InvalidInstructionData);

    create_pda_account(payer, pda_account, account_size, bump, &signers_seeds)
}

pub fn create_pda_account<'a>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    account_size: usize,
    bump: u8,
    signers_seeds: &[&[u8]],
) -> ProgramResult {
    // For unit testing we exit
    if cfg!(test) {
        return Ok(());
    }

    let lamports_required = Rent::get()?.minimum_balance(account_size);
    let space: u64 = account_size.try_into().unwrap();
    guard!(payer.lamports() >= lamports_required, InvalidAccountBalance);

    // Additional (redundant) check that account does not already exist
    guard!(
        match pda_account.try_data_len() {
            Ok(l) => l == 0,
            Err(_) => true
        },
        InvalidAccount
    );

    invoke_signed(
        &system_instruction::create_account(
            payer.key,
            pda_account.key,
            lamports_required,
            space,
            &crate::id(),
        ),
        &[
            payer.clone(),
            pda_account.clone(),
        ],
        &[signers_seeds]
    )?;

    // Assign default fields
    let data = &mut pda_account.data.borrow_mut()[..];
    let mut fields = PDAAccountFields::new(data)?;
    fields.bump_seed = bump;
    fields.version = 0;
    fields.initialized = false;
    PDAAccountFields::override_slice(&fields, data);

    Ok(())
}

pub fn close_account<'a>(
    payer: &AccountInfo<'a>,
    account: &AccountInfo<'a>,
) -> ProgramResult {
    let lamports = account.lamports();
    send_from_pool(account, payer, lamports)
}

#[cfg(test)]
mod tests {
    /*#[test]
    fn test_public_inputs_commitment_duplicate() {
        panic!()
    }

    #[test]
    fn test_public_inputs_different_roots_same_tree() {
        panic!()
    }

    #[test]
    fn test_public_inputs_different_nullifiers() {
        panic!()
    }

    #[test]
    fn test_public_inputs_valid() {
        panic!()
    }*/
}