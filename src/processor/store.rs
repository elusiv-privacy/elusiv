use solana_program::entrypoint::ProgramResult;
use solana_program::account_info::AccountInfo;
use crate::types::U256;
use crate::state::StorageAccount;
use crate::queue::state::QueueAccount;
use crate::error::ElusivError::{ InvalidAmount, CommitmentAlreadyExists };
use crate::commitment::commitment::UnverifiedCommitment;
use super::utils::{ compute_fee, send_with_system_program };

pub const MINIMUM_AMOUNT: u64 = LAMPORTS_PER_SOL / 10;

pub fn store<'a>(
    sender: &AccountInfo<'a>,
    storage_account: &StorageAccount,
    queue_account: &mut QueueAccount,
    pool: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    commitment_core: U256,
    amount: u64,
    commitment: U256,
) -> ProgramResult {
    // Check amount
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
    queue_account.unverified_commitment_queue.enqueue(
        UnverifiedCommitment {
            commitment_core,
            amount,
            commitment,
        }
    )?;

    // Compute fee
    let fee = compute_fee(0);
    
    // Transfer funds + fees
    let lamports = proof_data.amount + fee;
    send_with_system_program(sender, pool, system_program, lamports)
}