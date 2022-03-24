use solana_program::{
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    native_token::LAMPORTS_PER_SOL,
    account_info::AccountInfo,
};
use crate::types::{ProofData, U256};
use crate::error::ElusivError;

use crate::state::*;
use crate::queue::state::*;
use crate::queue::proof_request::ProofRequest;

const MINIMUM_AMOUNT: u64 = LAMPORTS_PER_SOL / 10;

pub fn store<'a>(
    sender: &AccountInfo<'a>,
    storage_account: &StorageAccount,
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

    // TODO: Compute fee
    let fee = 0;

    // Add store request to queue
    queue_account.proof_queue.enqueue(
        ProofRequest::Store { proof_data, fee, commitment }
    )?;

    // Transfer funds + fees
    let lamports = proof_data.amount + fee;
    send_with_system_program(sender, pool, system_program, lamports)
}

pub fn bind<'a>(
    sender: &AccountInfo<'a>,
    storage_account: &StorageAccount,
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

    // TODO: Compute fee
    let fee = 0;

    // Add bind request to queue
    queue_account.proof_queue.enqueue(
        ProofRequest::Bind { proof_data, fee, unbound_commitment: commitments[0], bound_commitment: commitments[1] }
    )?;

    // Transfer funds + fees
    let lamports = proof_data.amount + fee;
    send_with_system_program(sender, pool, system_program, lamports)
}

fn check_public_inputs(
    storage_account: &StorageAccount,
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

pub fn send() -> ProgramResult {
    Ok(())
}

pub fn finalize_send() -> ProgramResult {
    // Check for nullifier_hash

    Ok(())
}