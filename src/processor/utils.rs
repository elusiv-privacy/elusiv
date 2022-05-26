use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    system_program,
    program_error::ProgramError,
};
use crate::error::ElusivError::{InvalidAccount, InvalidAmount};
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
    
    let (_, bump_seed) = Pubkey::find_program_address(&[b"elusiv"], &super::super::id());
    solana_program::program::invoke_signed(
        &instruction,
        &[
            sender.clone(),
            recipient.clone(),
            system_program.clone(),
        ],
        &[&[&b"elusiv"[..], &[bump_seed]]],
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