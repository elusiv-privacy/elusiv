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
use crate::error::ElusivError::{InvalidAccount, InvalidAmount, InvalidAccountBalance};
use crate::macros::guard;

/// Sends lamports from the sender Sender to the recipient
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
        &sender.key,
        recipient.key,
        lamports 
    );
    
    //let (_, bump_seed) = Pubkey::find_program_address(&[b"elusiv"], &super::super::id());
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
        .ok_or(ProgramError::from(InvalidAmount))?;

    **recipient.try_borrow_mut_lamports()? = recipient.lamports().checked_add(amount)
        .ok_or(ProgramError::from(InvalidAmount))?;

    Ok(())
}


pub fn create_pda_account<'a>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    account_size: usize,
    bump: u8,
    signers_seeds: &[&[u8]],
) -> ProgramResult {
    let lamports_required = Rent::get()?.minimum_balance(account_size);
    let space: u64 = account_size.try_into().unwrap();
    guard!(payer.lamports() >= lamports_required, InvalidAccountBalance);

    invoke_signed(
        &system_instruction::create_account(
            &payer.key,
            &pda_account.key,
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

    let data = &mut pda_account.data.borrow_mut()[..];

    // Save `bump_seed`
    data[0] = bump;
    // Set `initialized` flag
    data[1] = 1;

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