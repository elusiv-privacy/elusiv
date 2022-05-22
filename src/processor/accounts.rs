use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    system_instruction,
    program::invoke_signed,
    sysvar::Sysvar,
    rent::Rent,
};
use crate::state::{StorageAccount, pool::PoolAccount, reserve::ReserveAccount, program_account::{PDAAccount, SizedAccount, MultiAccountAccount, MAX_ACCOUNT_SIZE, MultiInstanceAccount}};
use crate::state::queue::{CommitmentQueueAccount, BaseCommitmentQueueAccount, SendProofQueueAccount, MergeProofQueueAccount, MigrateProofQueueAccount};
use crate::proof::{VerificationAccount};
use crate::commitment::{BaseCommitmentHashingAccount, CommitmentHashingAccount};
use crate::error::ElusivError::{InvalidAccountBalance, InvalidInstructionData};
use crate::macros::*;
use crate::bytes::SerDe;

#[derive(SerDe)]
pub enum SingleInstanceAccountKind {
    Pool,
    Reserve,
    CommitmentQueue,
    SendQueue,
    MergeQueue,
    MigrateQueue,
    CommitmentHashing,
}

macro_rules! single_instance_account {
    ($v: ident, $e: ident) => {
        match $v {
            SingleInstanceAccountKind::Pool => PoolAccount::$e,
            SingleInstanceAccountKind::Reserve => ReserveAccount::$e,
            SingleInstanceAccountKind::CommitmentQueue => CommitmentQueueAccount::$e,
            SingleInstanceAccountKind::SendQueue => SendProofQueueAccount::$e,
            SingleInstanceAccountKind::MergeQueue => MergeProofQueueAccount::$e,
            SingleInstanceAccountKind::MigrateQueue => MigrateProofQueueAccount::$e,
            SingleInstanceAccountKind::CommitmentHashing => CommitmentHashingAccount::$e,
        }
    };
}

/// Used to open the PDA accounts, of which types there always only exist one instance
pub fn open_single_instance_account<'a>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,

    kind: SingleInstanceAccountKind,
) -> ProgramResult {
    let account_size = single_instance_account!(kind, SIZE);
    let offsets = &vec![0];
    let signers_seeds = single_instance_account!(kind, offset_seed)(offsets);
    let signers_seeds: Vec<&[u8]> = signers_seeds.iter().map(|x| &x[..]).collect();

    guard!(single_instance_account!(kind, pubkey)(offsets).0 == *pda_account.key, InvalidInstructionData);

    create_pda_account(payer, pda_account, system_program, account_size, &signers_seeds)
}

#[derive(SerDe)]
pub enum MultiInstanceAccountKind {
    Verification,
    BaseCommitmentHashing,
}

macro_rules! multi_instance_account {
    ($v: ident, $e: ident) => {
        match $v {
            MultiInstanceAccountKind::Verification => VerificationAccount::$e,
            MultiInstanceAccountKind::BaseCommitmentHashing => BaseCommitmentHashingAccount::$e,
        }
    };
}

/// Used to open the PDA accounts, of which types there can exist multipe (that satisfy the trait: MultiInstanceAccount)
pub fn open_multi_instance_account<'a>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,

    pda_offset: u64,
    kind: MultiInstanceAccountKind,
) -> ProgramResult {
    guard!(pda_offset < multi_instance_account!(kind, MAX_INSTANCES), InvalidInstructionData);

    panic!("TODO: Implement Intermediary account size");
}

fn create_pda_account<'a>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    account_size: usize,
    signers_seeds: &[&[u8]],
) -> ProgramResult {
    let lamports_required = Rent::get()?.minimum_balance(account_size);

    guard!(payer.lamports() >= lamports_required, InvalidAccountBalance);

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