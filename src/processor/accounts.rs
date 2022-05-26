use borsh::{BorshSerialize, BorshDeserialize};
use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    system_instruction,
    program::{invoke_signed, invoke},
    sysvar::Sysvar,
    rent::Rent,
};
use crate::{state::{pool::PoolAccount, program_account::{PDAAccount, SizedAccount, MultiInstanceAccount, PDAAccountFields}, queue::{FinalizeSendQueueAccount}}, bytes::is_zero};
use crate::state::queue::{QueueManagementAccount, CommitmentQueueAccount, BaseCommitmentQueueAccount, SendProofQueueAccount, MergeProofQueueAccount, MigrateProofQueueAccount};
use crate::proof::{VerificationAccount};
use crate::commitment::{BaseCommitmentHashingAccount, CommitmentHashingAccount};
use crate::error::ElusivError::{InvalidAccountBalance, InvalidInstructionData};
use crate::macros::*;
use crate::bytes::{BorshSerDeSized, ElusivOption};

#[derive(BorshSerialize, BorshDeserialize, BorshSerDeSized)]
pub enum SingleInstancePDAAccountKind {
    Pool,
    QueueManagementAccount,
    CommitmentHashing,
}

#[derive(BorshSerialize, BorshDeserialize, BorshSerDeSized)]
pub enum MultiInstancePDAAccountKind {
    Verification,
    BaseCommitmentHashing,
}

macro_rules! single_instance_account {
    ($v: ident, $e: ident) => {
        match $v {
            SingleInstancePDAAccountKind::Pool => PoolAccount::$e,
            SingleInstancePDAAccountKind::QueueManagementAccount => QueueManagementAccount::$e,
            SingleInstancePDAAccountKind::CommitmentHashing => CommitmentHashingAccount::$e,
        }
    };
}

/// Used to open the PDA accounts, of which types there always only exist one instance
pub fn open_single_instance_account<'a>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    _system_program: &AccountInfo<'a>,

    kind: SingleInstancePDAAccountKind,
) -> ProgramResult {
    let account_size = single_instance_account!(kind, SIZE);
    let (pk, bump) = single_instance_account!(kind, find)(None);
    let seed = vec![single_instance_account!(kind, SEED).to_vec(), vec![bump]];
    let signers_seeds: Vec<&[u8]> = seed.iter().map(|x| &x[..]).collect();
    guard!(pk == *pda_account.key, InvalidInstructionData);
    
    create_pda_account(payer, pda_account, account_size, &signers_seeds)?;

    // Save bump to account
    let data = &mut pda_account.data.borrow_mut()[..];
    data[0] = bump;

    Ok(())
}

macro_rules! multi_instance_account {
    ($v: ident, $e: ident) => {
        match $v {
            MultiInstancePDAAccountKind::Verification => VerificationAccount::$e,
            MultiInstancePDAAccountKind::BaseCommitmentHashing => BaseCommitmentHashingAccount::$e,
        }
    };
}

/// Used to open the PDA accounts, of which types there can exist multipe (that satisfy the trait: MultiInstanceAccount)
pub fn open_multi_instance_account<'a>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    _system_program: &AccountInfo<'a>,

    pda_offset: u64,
    kind: MultiInstancePDAAccountKind,
) -> ProgramResult {
    guard!(pda_offset < multi_instance_account!(kind, MAX_INSTANCES), InvalidInstructionData);

    let account_size = multi_instance_account!(kind, SIZE);
    let (pk, bump) = multi_instance_account!(kind, find)(Some(pda_offset));
    let seed = vec![
        multi_instance_account!(kind, SEED).to_vec(),
        u64::to_le_bytes(pda_offset).to_vec(),
        vec![bump]
    ];
    let signers_seeds: Vec<&[u8]> = seed.iter().map(|x| &x[..]).collect();
    guard!(pk == *pda_account.key, InvalidInstructionData);

    create_pda_account(payer, pda_account, account_size, &signers_seeds)
}

macro_rules! verify_queue {
    ($account: ident, $ty: ty, $manager: ident, $setter: ident) => {
        guard!(is_zero(&$account.data.borrow()[..]), InvalidInstructionData);
        guard!($account.data_len() == <$ty>::SIZE, InvalidInstructionData);
        guard!($account.lamports() >= Rent::get()?.minimum_balance(<$ty>::SIZE), InvalidInstructionData);
        guard!(*$account.owner == crate::id(), InvalidInstructionData);

        $manager.$setter(&$account.key.to_bytes());
    };
}

pub fn setup_queue_accounts(
    base_commitment_queue: &AccountInfo,
    commitment_queue: &AccountInfo,
    send_proof_queue: &AccountInfo,
    merge_proof_queue: &AccountInfo,
    migrate_proof_queue: &AccountInfo,
    finalize_send_queue: &AccountInfo,
    queue_manager: &mut QueueManagementAccount,
) -> ProgramResult {
    guard!(!queue_manager.get_is_initialized(), InvalidInstructionData);

    // Check for account non-ownership, size, zero-ness, rent-excemption and assign queue
    verify_queue!(base_commitment_queue, BaseCommitmentQueueAccount, queue_manager, set_base_commitment_queue);
    verify_queue!(commitment_queue, CommitmentQueueAccount, queue_manager, set_commitment_queue);
    verify_queue!(send_proof_queue, SendProofQueueAccount, queue_manager, set_send_proof_queue);
    verify_queue!(merge_proof_queue, MergeProofQueueAccount, queue_manager, set_merge_proof_queue);
    verify_queue!(migrate_proof_queue, MigrateProofQueueAccount, queue_manager, set_migrate_proof_queue);
    verify_queue!(finalize_send_queue, FinalizeSendQueueAccount, queue_manager, set_finalize_send_queue);

    queue_manager.set_is_initialized(&true);

    Ok(())
}

fn create_pda_account<'a>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    account_size: usize,
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
    )
}

/*fn create_account<'a>(
    payer: &AccountInfo<'a>,
    account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    account_size: usize,
) -> ProgramResult {
    let lamports_required = Rent::get()?.minimum_balance(account_size);
    let space: u64 = account_size.try_into().unwrap();
    guard!(payer.lamports() >= lamports_required, InvalidAccountBalance);

    invoke(
        &system_instruction::create_account(
            &payer.key,
            &account.key,
            lamports_required,
            space,
            &crate::id(),
        ),
        &[
            payer.clone(),
            account.clone(),
            system_program.clone(),
        ],
    )
}*/