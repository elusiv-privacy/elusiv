//! Processor instructions executed by client transactions

use solana_program::{
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    native_token::LAMPORTS_PER_SOL,
    account_info::AccountInfo,
};
use crate::types::{ProofData, U256};
use super::super::error::ElusivError;

use super::super::state::*;
use super::super::queue::state::*;

const MINIMUM_AMOUNT: u64 = LAMPORTS_PER_SOL / 10;

pub fn store<'a>(
    sender: &AccountInfo<'a>,
    storage_account: StorageAccount,
    queue_account: &mut QueueAccount,
    pool: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    proof_data: ProofData,
    commitment: U256,
) -> ProgramResult {
    // Check public inputs
    check_public_inputs(
        storage_account,
        &proof_data,
        &vec![commitment],
    )?;

    // Compute fee
    let fee = 0;

    // Add store request to queue
    queue_account.store_queue.enqueue(
        StoreRequest {
            proof: proof_data.proof,
            nullifier_hash: proof_data.nullifier_hash,
            commitment,
            fee,
        }
    )?;

    // Transfer funds + fees
    let lamports = proof_data.amount + fee;
    send_with_system_program(sender, pool, system_program, lamports)
}

pub fn bind<'a>(
    sender: &AccountInfo<'a>,
    storage_account: StorageAccount,
    queue_account: &mut QueueAccount,
    pool: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    proof_data: ProofData,
    commitments: [U256; 2],
) -> ProgramResult {
    // Check public inputs
    check_public_inputs(
        storage_account,
        &proof_data,
        &commitments,
    )?;

    // Compute fee
    let fee = 0;

    // Add store request to queue
    queue_account.bind_queue.enqueue(
        BindRequest {
            proof: proof_data.proof,
            nullifier_hash: proof_data.nullifier_hash,
            commitments,
            fee,
        }
    )?;

    // Transfer funds + fees
    let lamports = proof_data.amount + fee;
    send_with_system_program(sender, pool, system_program, lamports)
}

fn check_public_inputs(
    storage_account: StorageAccount,
    proof_data: &ProofData,
    commitments: &[U256],
) -> ProgramResult {
    // Check if commitments are new
    for &commitment in commitments {
        storage_account.can_insert_commitment(commitment)?;
    }

    // Check if nullifier_hash is new
    storage_account.can_insert_nullifier_hash(proof_data.nullifier_hash)?;

    // Check root
    storage_account.is_root_valid(proof_data.root)?;

    // Check amount
    if proof_data.amount < MINIMUM_AMOUNT {
        return Err(ElusivError::InvalidAmount.into());
    }

    Ok(())
}

fn send_with_system_program<'a>(
    sender: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>,
    system_program: & AccountInfo<'a>,
    lamports: u64,
) -> ProgramResult {
    // Check if system_program is correct
    if *system_program.key != solana_program::system_program::ID {
        return Err(ElusivError::InvalidAccount.into());
    }

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
