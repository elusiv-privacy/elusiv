use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    native_token::LAMPORTS_PER_SOL,
};
use crate::types::U256;
use crate::state::StorageAccount;
use crate::queue::state::QueueAccount;
use crate::error::ElusivError::{ InvalidAmount, CommitmentAlreadyExists };
use crate::commitment::commitment::UnverifiedCommitment;
use super::utils::{ compute_fee, send_with_system_program };

pub const MINIMUM_STORE_AMOUNT: u64 = LAMPORTS_PER_SOL / 10;

/// Places a base commitment and amount in the queue and takes the funds from the sender
pub fn store<'a>(
    base_commitment: U256,
    amount: u64,
    commitment: U256,
) -> ProgramResult {
    // Check amount (zero amounts are allowed since the user may need multiple commitments for some proofs)
    guard!(
        amount >= MINIMUM_AMOUNT || amount == 0,
        InvalidAmount
    );

    // Check that commitment is new in tree
    storage_account.can_insert_commitment(commitment)?;

    // Check that request is not already enqueued
    guard!(
        !queue_account.unverified_commitment_queue.contains(),
        CommitmentAlreadyExists
    );

    // Enqueue unverified commitment
    queue_account.base_commitment_queue.enqueue(
        CommitmentHashRequest { base_commitment, amount, commitment, }
    )?;

    // Compute fee
    let fee = compute_fee(0);
    
    // Transfer funds + fees
    let lamports = proof_data.amount + fee;
    send_with_system_program(sender, pool, system_program, lamports)
}