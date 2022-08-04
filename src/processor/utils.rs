use solana_program::pubkey::Pubkey;
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
    InsufficientFunds
};
use crate::macros::guard;
use crate::state::program_account::{
    PDAAccountData,
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
    pda_offset: u32,
) -> ProgramResult {
    let account_size = T::SIZE;
    let (pk, bump) = T::find(Some(pda_offset));
    let seed = vec![
        T::SEED.to_vec(),
        u32::to_le_bytes(pda_offset).to_vec(),
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
    let seeds = vec![T::SEED.to_vec(), vec![bump]];
    let signers_seeds: Vec<&[u8]> = seeds.iter().map(|x| &x[..]).collect();
    guard!(pk == *pda_account.key, InvalidInstructionData);

    create_pda_account(payer, pda_account, account_size, bump, &signers_seeds)
}

pub fn open_pda_account<'a>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    account_size: usize,
    seeds: &[&[u8]],
) -> ProgramResult {
    let mut signers_seeds = seeds.to_owned();
    let (pubkey, bump) = Pubkey::find_program_address(&signers_seeds[..], &crate::ID);
    let b = vec![bump];
    signers_seeds.push(&b);
    guard!(pubkey == *pda_account.key, InvalidInstructionData);

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
    guard!(payer.lamports() >= lamports_required, InsufficientFunds);

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
    PDAAccountData::override_slice(
        &PDAAccountData {
            bump_seed: bump,
            version: 0,
            initialized: false,
        },
        data
    )?;

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
    use super::*;
    use crate::{macros::{test_account_info, account}, proof::VerificationAccount};
    use assert_matches::assert_matches;
    use solana_program::pubkey::Pubkey;

    #[test]
    fn test_send_with_system_program() {
        test_account_info!(sender, 0);
        test_account_info!(recipient, 0);

        let invalid_id =  Pubkey::new_unique();
        account!(invalid_system_program, invalid_id, vec![]);

        let valid_id = system_program::ID;
        account!(system_program, valid_id, vec![]);

        assert_matches!(send_with_system_program(&sender, &recipient, &invalid_system_program, 1), Err(_));
        send_with_system_program(&sender, &recipient, &system_program, 1).unwrap();
    }

    #[test]
    fn test_open_pda_account_with_offset() {
        test_account_info!(payer, 0);
        let correct_pda = VerificationAccount::find(Some(3)).0;
        account!(pda_account, correct_pda, vec![]);
        open_pda_account_with_offset::<VerificationAccount>(&payer, &pda_account, 3).unwrap();
    }

    #[test]
    fn test_open_pda_account_with_offset_invalid_pda() {
        test_account_info!(payer, 0);
        let correct_pda = VerificationAccount::find(Some(3)).0;
        account!(pda_account, correct_pda, vec![]);

        assert_matches!(open_pda_account_with_offset::<VerificationAccount>(&payer, &pda_account, 2), Err(_));
    }

    #[test]
    fn test_open_pda_account_without_offset() {
        test_account_info!(payer, 0);
        let correct_pda = VerificationAccount::find(None).0;
        account!(pda_account, correct_pda, vec![]);
        open_pda_account_without_offset::<VerificationAccount>(&payer, &pda_account).unwrap();
    }

    #[test]
    fn test_open_pda_account_without_offset_invalid_pda() {
        test_account_info!(payer, 0);
        let wrong_pda = VerificationAccount::find(Some(0)).0;
        account!(pda_account, wrong_pda, vec![]);

        assert_matches!(open_pda_account_without_offset::<VerificationAccount>(&payer, &pda_account), Err(_));
    }

    #[test]
    fn test_open_pda_account() {
        test_account_info!(payer, 0);
        let seed = b"test";
        let seeds = vec![&seed[..], &seed[..]];
        let pda = Pubkey::find_program_address(&seeds, &crate::ID).0;
        let wrong_pda = VerificationAccount::find(Some(0)).0;

        account!(pda_account, wrong_pda, vec![]);
        assert_matches!(open_pda_account(&payer, &pda_account, 1, &seeds), Err(_));

        account!(pda_account, pda, vec![]);
        assert_matches!(open_pda_account(&payer, &pda_account, 1, &seeds), Ok(_));
    }
}