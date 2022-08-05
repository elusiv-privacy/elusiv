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
    InsufficientFunds
};
use crate::macros::{guard, pda_account};
use crate::state::governor::{PoolAccount, FeeCollectorAccount};
use crate::state::program_account::{
    PDAAccountData,
    PDAAccount,
    SizedAccount, ProgramAccount
};
use crate::token::{Token, Lamports, TokenAuthorityAccount};

use super::MATH_ERR;

pub fn transfer_token<'a>(
    source: &AccountInfo<'a>,
    source_token_account: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    token: Token,
) -> ProgramResult {
    if token.token_id() == 0 {
        if *source.owner == crate::ID && !source.key.is_on_curve() {
            transfer_lamports_from_pda_checked(
                source,
                destination,
                Lamports(token.amount()),
            )
        } else {
            transfer_with_system_program(
                source,
                destination,
                token_program,
                Lamports(token.amount()),
            )
        }
    } else {
        transfer_with_token_program(
            source,
            source_token_account,
            destination,
            token_program,
            token.amount(),
        )
    }
}

fn transfer_with_system_program<'a>(
    source: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    lamports: Lamports,
) -> ProgramResult {
    guard!(*system_program.key == system_program::ID, InvalidAccount);

    let instruction = solana_program::system_instruction::transfer(
        source.key,
        destination.key,
        lamports.0,
    );
    
    solana_program::program::invoke_signed(
        &instruction,
        &[
            source.clone(),
            destination.clone(),
            system_program.clone(),
        ],
        &[],
    )    
}

fn transfer_with_token_program<'a>(
    source: &AccountInfo<'a>,
    source_token_account: &AccountInfo<'a>,
    destination_token_account: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    amount: u64,
) -> ProgramResult {
    guard!(*token_program.key == spl_token::ID, InvalidAccount);
    guard!(*source_token_account.owner == spl_token::ID, InvalidAccount);

    let instruction = spl_token::instruction::transfer(
        token_program.key,
        destination_token_account.key,
        destination_token_account.key,
        source.key,
        &[source.key],
        amount,
    )?;

    solana_program::program::invoke_signed(
        &instruction,
        &[
            source.clone(),
            source_token_account.clone(),
            destination_token_account.clone(),
            token_program.clone(),
        ],
        &[],
    )
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

pub unsafe fn transfer_lamports_from_pda<'a>(
    pda: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>,
    lamports: Lamports,
) -> ProgramResult {
    **pda.try_borrow_mut_lamports()? = pda.lamports().checked_sub(lamports.0)
        .ok_or(MATH_ERR)?;

    **recipient.try_borrow_mut_lamports()? = recipient.lamports().checked_add(lamports.0)
    .ok_or(MATH_ERR)?;

    Ok(())
}

pub fn transfer_lamports_from_pda_checked<'a>(
    pda: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>,
    lamports: Lamports,
) -> ProgramResult {
    let pda_lamports = pda.lamports();
    let pda_size = pda.data_len();
    let rent_lamports = Rent::get()?.minimum_balance(pda_size);

    if pda_lamports.checked_sub(lamports.0).ok_or(MATH_ERR)? < rent_lamports {
        return Err(ProgramError::AccountNotRentExempt)
    }

    unsafe {
        transfer_lamports_from_pda(pda, recipient, lamports)
    }
}

pub fn close_account<'a>(
    payer: &AccountInfo<'a>,
    account: &AccountInfo<'a>,
) -> ProgramResult {
    let lamports = Lamports(account.lamports());
    unsafe {
        transfer_lamports_from_pda(account, payer, lamports)
    }
}

pub fn verify_program_token_accounts(
    fee_collector: &AccountInfo,
    fee_collector_account: &AccountInfo,
    pool: &AccountInfo,
    pool_account: &AccountInfo,
    token_id: u16,
) -> ProgramResult {
    pda_account!(fee_collector, FeeCollectorAccount, fee_collector);
    fee_collector.enforce_token_account(token_id, fee_collector_account)?;

    pda_account!(pool, PoolAccount, pool);
    pool.enforce_token_account(token_id, pool_account)?;

    Ok(())
}

/*#[cfg(test)]
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
}*/