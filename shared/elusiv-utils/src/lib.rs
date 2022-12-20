pub mod macros;

use solana_program::{
    pubkey::Pubkey, system_instruction, entrypoint::ProgramResult, account_info::AccountInfo, rent::Rent, program::invoke_signed, sysvar::Sysvar,
    program_error::ProgramError::{self, InvalidInstructionData, InsufficientFunds}, 
};
use elusiv_types::accounts::{PDAAccount, SizedAccount, PDAAccountData};
use elusiv_types::bytes::BorshSerDeSized;

#[cfg(feature = "sdk")]
use solana_sdk::compute_budget::ComputeBudgetInstruction;

#[cfg(feature = "sdk")]
use solana_program::instruction::Instruction;

pub const MATH_ERR: ProgramError = ProgramError::InvalidArgument;

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
) -> ProgramResult {
    let account_size = T::SIZE;
    let (pk, bump) = T::find(Some(pda_offset));
    let seeds = T::signers_seeds(Some(pda_offset), bump);
    let signers_seeds = signers_seeds!(seeds);
    guard!(pk == *pda_account.key, InvalidInstructionData);

    create_pda_account(program_id, payer, pda_account, account_size, bump, &signers_seeds)
}

pub fn open_pda_account_without_offset<'a, T: PDAAccount + SizedAccount>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
) -> ProgramResult {
    let account_size = T::SIZE;
    let (pk, bump) = T::find(None);
    let seeds = T::signers_seeds(None, bump);
    let signers_seeds = signers_seeds!(seeds);
    guard!(pk == *pda_account.key, InvalidInstructionData);

    create_pda_account(program_id, payer, pda_account, account_size, bump, &signers_seeds)
}

pub fn open_pda_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    account_size: usize,
    seeds: &[&[u8]],
) -> ProgramResult {
    let mut signers_seeds = seeds.to_owned();
    let (pubkey, bump) = Pubkey::find_program_address(&signers_seeds[..], program_id);
    let b = vec![bump];
    signers_seeds.push(&b);
    guard!(pubkey == *pda_account.key, InvalidInstructionData);

    create_pda_account(program_id, payer, pda_account, account_size, bump, &signers_seeds)
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
    guard!(payer.lamports() >= lamports_required, InsufficientFunds);

    invoke_signed(
        &system_instruction::create_account(
            payer.key,
            pda_account.key,
            lamports_required,
            space,
            program_id,
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
        },
        data
    )?;

    Ok(())
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn transfer_lamports_from_pda<'a>(
    pda: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>,
    lamports: u64,
) -> ProgramResult {
    **pda.try_borrow_mut_lamports()? = pda.lamports().checked_sub(lamports)
        .ok_or(MATH_ERR)?;

    **recipient.try_borrow_mut_lamports()? = recipient.lamports().checked_add(lamports)
        .ok_or(MATH_ERR)?;

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
            return Err(ProgramError::AccountNotRentExempt)
        }
    }

    unsafe {
        transfer_lamports_from_pda(pda, recipient, lamports)
    }
}

pub fn close_account<'a>(
    payer: &AccountInfo<'a>,
    account: &AccountInfo<'a>,
) -> ProgramResult {
    unsafe {
        transfer_lamports_from_pda(account, payer, account.lamports())
    }
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
    let mut ixs = vec![
        ComputeBudgetInstruction::set_compute_unit_limit(batch_size as u32 * compute_units_per_ix),
    ];
    for _ in 0..batch_size {
        ixs.push(ix.clone());
    }

    for _ in 0..total_ix_count / batch_size {
        v.push(ixs.clone());
    }
    
    let remaining_ix_count = total_ix_count % batch_size;
    if remaining_ix_count > 0 {
        let mut ixs = vec![
            ComputeBudgetInstruction::set_compute_unit_limit(batch_size as u32 * compute_units_per_ix),
        ];
        for _ in 0..remaining_ix_count {
            ixs.push(ix.clone());
        }
        v.push(ixs);
    }

    v
}

#[cfg(feature = "computation")]
pub fn batched_instructions_tx_count(
    total_ix_count: usize,
    compute_units_per_ix: u32,
) -> usize {
    let batch_size = elusiv_computation::MAX_COMPUTE_UNIT_LIMIT as usize / compute_units_per_ix as usize;
    total_ix_count / batch_size + usize::from(total_ix_count % batch_size != 0)
}