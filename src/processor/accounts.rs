use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    system_instruction,
    program::invoke_signed,
    sysvar::Sysvar,
    rent::Rent,
};
use crate::state::{StorageAccount, pool::PoolAccount, reserve::ReserveAccount, program_account::{PDAAccount, SizedAccount, MultiAccountAccount, MAX_ACCOUNT_SIZE}};
use crate::state::queue::{CommitmentQueueAccount, BaseCommitmentQueueAccount, SendProofQueueAccount, MergeProofQueueAccount, MigrateProofQueueAccount};
use crate::proof::{VerificationAccount, MAX_VERIFICATION_ACCOUNTS_COUNT};
use crate::commitment::{BaseCommitmentHashingAccount, MAX_BASE_COMMITMENT_ACCOUNTS_COUNT, CommitmentHashingAccount};
use crate::error::ElusivError::{InvalidAccountBalance, InvalidInstructionData};
use crate::macros::guard;

/// Used to open the PDA accounts, of which types there always only exist one instance
pub fn open_unique_accounts<'a>(
    payer: &AccountInfo<'a>,
    pool: &AccountInfo<'a>,
    reserve: &AccountInfo<'a>,
    commitment_queue: &AccountInfo<'a>,
    base_commitment_queue: &AccountInfo<'a>,
    send_queue: &AccountInfo<'a>,
    merge_queue: &AccountInfo<'a>,
    migrate_queue: &AccountInfo<'a>,
    storage_account: Vec<&AccountInfo<'a>>,
    commitment_hash_account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
) -> ProgramResult {
    create_pda_account::<PoolAccount>(
        payer,
        pool,
        system_program,
        0,
        vec![0]
    )?;
    create_pda_account::<ReserveAccount>(
        payer,
        reserve, 
        system_program,
        0,
        vec![0]
    )?;

    create_pda_account::<CommitmentQueueAccount>(
        payer,
        commitment_queue,
        system_program,
        CommitmentQueueAccount::SIZE,
        vec![0]
    )?;
    create_pda_account::<BaseCommitmentQueueAccount>(
        payer,
        base_commitment_queue,
        system_program, BaseCommitmentQueueAccount::SIZE,
        vec![0]
    )?;
    create_pda_account::<SendProofQueueAccount>(
        payer,
        send_queue,
        system_program, SendProofQueueAccount::SIZE,
        vec![0]
    )?;
    create_pda_account::<MergeProofQueueAccount>(
        payer,
        send_queue,
        system_program, MergeProofQueueAccount::SIZE,
        vec![0]
    )?;
    create_pda_account::<MigrateProofQueueAccount>(
        payer,
        send_queue, 
        system_program, MigrateProofQueueAccount::SIZE,
        vec![0]
    )?;

    create_pda_account::<CommitmentHashingAccount>(
        payer,
        commitment_hash_account,
        system_program, CommitmentHashingAccount::SIZE,
        vec![0]
    )?;

    crate_multi_pda_account::<StorageAccount>(
        payer,
        storage_account,
        system_program,
        vec![0]
    )
}

/// Opens a new proof verification account
pub fn open_proof_verification_account<'a>(
    reserve: &AccountInfo<'a>,
    verification_account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    verification_account_index: u64,
) -> ProgramResult {
    guard!(verification_account_index < MAX_VERIFICATION_ACCOUNTS_COUNT, InvalidInstructionData);

    create_pda_account::<VerificationAccount>(
        reserve,
        verification_account,
        system_program,
        VerificationAccount::SIZE,
        vec![0]
    )
}

/// Opens a new commitment hashing account
pub fn open_base_commitment_hash_account<'a>(
    reserve: &AccountInfo<'a>,
    base_commitment_hash_account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    base_commitment_hash_account_index: u64,
) -> ProgramResult {
    guard!(base_commitment_hash_account_index < MAX_BASE_COMMITMENT_ACCOUNTS_COUNT, InvalidInstructionData);

    create_pda_account::<BaseCommitmentHashingAccount>(
        reserve,
        base_commitment_hash_account,
        system_program,
        BaseCommitmentHashingAccount::SIZE,
        vec![0]
    )
}

/// Create a multi-account PDA account and all sub-accounts
fn crate_multi_pda_account<'a, 'b, T: PDAAccount + MultiAccountAccount<'b> + SizedAccount>(
    payer: &AccountInfo<'a>,
    pda_accounts: Vec<&AccountInfo<'a>>,
    system_program: &AccountInfo<'a>,
    base_offsets: Vec<u64>,
) -> ProgramResult {
    assert!(pda_accounts.len() == T::COUNT + 1);

    create_pda_account::<StorageAccount>(
        payer,
        pda_accounts[0],
        system_program,
        T::SIZE,
        base_offsets.clone()
    )?;
    
    for i in 0..T::COUNT {
        let mut offsets = base_offsets.clone();
        offsets.push(i as u64);
        create_pda_account::<StorageAccount>(
            payer,
            pda_accounts[i + 1],
            system_program,
            MAX_ACCOUNT_SIZE,
            offsets
        )?;
    }

    Ok(())
}

fn create_pda_account<'a, T: PDAAccount>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    account_size: usize,
    offsets: Vec<u64>,
) -> ProgramResult {
    let lamports_required = Rent::get()?.minimum_balance(account_size);

    let signers_seeds = T::offset_seed(&offsets);
    let signers_seeds: Vec<&[u8]> = signers_seeds.iter().map(|x| &x[..]).collect();

    guard!(payer.lamports() >= lamports_required, InvalidAccountBalance);
    guard!(T::pubkey(&offsets).0 == *pda_account.key, InvalidInstructionData);

    let space: u64 = account_size.try_into().unwrap();

    let create_pda_account_ix = system_instruction::create_account(
        &payer.key,
        &pda_account.key,
        lamports_required,
        space,
        &crate::id(),
    );

    invoke_signed(
        &create_pda_account_ix,
        &[
            payer.clone(),
            pda_account.clone(),
            system_program.clone(),
        ],
        &[&signers_seeds],
    )
}