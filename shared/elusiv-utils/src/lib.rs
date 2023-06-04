pub mod macros;

use elusiv_types::{
    accounts::{PDAAccount, PDAAccountData, SizedAccount},
    PDAOffset,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program::invoke_signed,
    program_error::ProgramError, pubkey::Pubkey, rent::Rent, system_instruction, sysvar::Sysvar,
};

#[cfg(feature = "sdk")]
use solana_sdk::compute_budget::ComputeBudgetInstruction;

#[cfg(feature = "sdk")]
use solana_program::instruction::Instruction;

pub const MATH_ERR: ProgramError = ProgramError::Custom(222);

#[macro_export]
macro_rules! signers_seeds {
    ($seeds: ident) => {
        $seeds.iter().map(|x| &x[..]).collect::<Vec<&[u8]>>()
    };
}

pub fn open_pda_account_with_offset<'a, T: PDAAccount + SizedAccount>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    pda_offset: u32,
    bump: Option<u8>,
) -> ProgramResult {
    open_pda_account::<T>(
        program_id,
        payer,
        pda_account,
        None,
        Some(pda_offset),
        bump,
        T::SIZE,
    )
}

pub fn open_pda_account_without_offset<'a, T: PDAAccount + SizedAccount>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    bump: Option<u8>,
) -> ProgramResult {
    open_pda_account::<T>(program_id, payer, pda_account, None, None, bump, T::SIZE)
}

pub fn open_pda_account_with_associated_pubkey<'a, T: PDAAccount + SizedAccount>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    pubkey: &Pubkey,
    pda_offset: PDAOffset,
    bump: Option<u8>,
) -> ProgramResult {
    open_pda_account::<T>(
        program_id,
        payer,
        pda_account,
        Some(*pubkey),
        pda_offset,
        bump,
        T::SIZE,
    )
}

pub fn open_pda_account<'a, T: PDAAccount>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    pda_pubkey: Option<Pubkey>,
    pda_offset: PDAOffset,
    bump: Option<u8>,
    account_size: usize,
) -> ProgramResult {
    let (pk, bump) = if let Some(bump) = bump {
        let pk = match pda_pubkey {
            Some(pubkey) => T::create_with_pubkey(pubkey, pda_offset, bump)?,
            None => T::create(pda_offset, bump)?,
        };

        (pk, bump)
    } else {
        match pda_pubkey {
            Some(pubkey) => T::find_with_pubkey(pubkey, pda_offset),
            None => T::find(pda_offset),
        }
    };

    guard!(pk == *pda_account.key, ProgramError::InvalidSeeds);
    let seeds = T::signers_seeds(pda_pubkey, pda_offset, bump);
    let signers_seeds = signers_seeds!(seeds);

    create_pda_account(
        program_id,
        payer,
        pda_account,
        account_size,
        bump,
        &signers_seeds,
    )
}

pub fn create_pda_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    account_size: usize,
    bump: u8,
    signers_seeds: &[&[u8]],
) -> ProgramResult {
    // We require the test-unit feature since cfg!(test) does not work in deps
    if cfg!(feature = "test-unit") {
        return Ok(());
    }

    let lamports_required = Rent::get()?.minimum_balance(account_size);
    let space: u64 = account_size.try_into().unwrap();
    guard!(
        payer.lamports() >= lamports_required,
        ProgramError::AccountNotRentExempt
    );

    invoke_signed(
        &system_instruction::create_account(
            payer.key,
            pda_account.key,
            lamports_required,
            space,
            program_id,
        ),
        &[payer.clone(), pda_account.clone()],
        &[signers_seeds],
    )?;

    // Assign default fields
    let mut data = &mut pda_account.data.borrow_mut()[..];
    borsh::BorshSerialize::serialize(
        &PDAAccountData {
            bump_seed: bump,
            version: 0,
        },
        &mut data,
    )?;

    Ok(())
}

pub fn transfer_with_system_program<'a>(
    source: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    lamports: u64,
) -> ProgramResult {
    guard!(
        *system_program.key == solana_program::system_program::ID,
        ProgramError::IncorrectProgramId
    );

    let instruction =
        solana_program::system_instruction::transfer(source.key, destination.key, lamports);

    solana_program::program::invoke(
        &instruction,
        &[source.clone(), destination.clone(), system_program.clone()],
    )
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn transfer_lamports_from_pda<'a>(
    pda: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>,
    lamports: u64,
) -> ProgramResult {
    **pda.try_borrow_mut_lamports()? = pda.lamports().checked_sub(lamports).ok_or(MATH_ERR)?;

    **recipient.try_borrow_mut_lamports()? =
        recipient.lamports().checked_add(lamports).ok_or(MATH_ERR)?;

    Ok(())
}

pub fn transfer_lamports_from_pda_checked<'a>(
    pda: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>,
    lamports: u64,
) -> ProgramResult {
    let pda_lamports = pda.lamports();
    let pda_size = pda.data_len();

    if !cfg!(feature = "test-unit") {
        let rent_lamports = Rent::get()?.minimum_balance(pda_size);
        if pda_lamports.checked_sub(lamports).ok_or(MATH_ERR)? < rent_lamports {
            return Err(ProgramError::AccountNotRentExempt);
        }
    }

    unsafe { transfer_lamports_from_pda(pda, recipient, lamports) }
}

pub fn close_account<'a>(payer: &AccountInfo<'a>, account: &AccountInfo<'a>) -> ProgramResult {
    unsafe { transfer_lamports_from_pda(account, payer, account.lamports()) }
}

#[cfg(feature = "sdk")]
/// Batches multiple identical instructions together
pub fn batch_instructions(
    total_ix_count: usize,
    compute_units_per_ix: u32,
    ix: Instruction,
) -> Vec<Vec<Instruction>> {
    let mut v = Vec::new();

    let batch_size = 1_400_000 / compute_units_per_ix as usize;
    let mut ixs = vec![ComputeBudgetInstruction::set_compute_unit_limit(
        batch_size as u32 * compute_units_per_ix,
    )];
    for _ in 0..batch_size {
        ixs.push(ix.clone());
    }

    for _ in 0..total_ix_count / batch_size {
        v.push(ixs.clone());
    }

    let remaining_ix_count = total_ix_count % batch_size;
    if remaining_ix_count > 0 {
        let mut ixs = vec![ComputeBudgetInstruction::set_compute_unit_limit(
            batch_size as u32 * compute_units_per_ix,
        )];
        for _ in 0..remaining_ix_count {
            ixs.push(ix.clone());
        }
        v.push(ixs);
    }

    v
}

#[cfg(feature = "computation")]
pub fn batched_instructions_tx_count(total_ix_count: usize, compute_units_per_ix: u32) -> usize {
    let batch_size =
        elusiv_computation::MAX_COMPUTE_UNIT_LIMIT as usize / compute_units_per_ix as usize;
    total_ix_count / batch_size + usize::from(total_ix_count % batch_size != 0)
}
