use solana_program::{
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    native_token::LAMPORTS_PER_SOL,
    account_info::AccountInfo,
};
use crate::macros::guard;
use crate::types::{ ProofDataBinary, U256 };
use crate::error::ElusivError::{
    InvalidAmount,
    InvalidAccount,
    InvalidRecipient,
    InvalidMerkleRoot,
};
use crate::state::*;
use crate::queue::state::*;
use crate::queue::proof_request::{ProofRequest, ProofRequestKind};
use super::utils::{ check_shared_public_inputs, compute_fee };

/// Enqueues a send proof that should be verified
pub fn send(
    storage_account: &StorageAccount,
    nullifier_accounts: [&NullifierAccount; 2],
    queue_account: &mut QueueAccount,
    proof_data: ProofDataBinary,
    amount: u64,
    recipient: U256,
) -> ProgramResult {
    // Check public inputs
    check_shared_public_inputs(proof_data, storage_account, nullifier_accounts)?;
    
    // Check amount
    guard!(
        proof_data.amount >= super::store::MINIMUM_AMOUNT,
        InvalidAmount
    );
    
    // Compute fee
    // TODO: What do we do with the fee at this point?
    // TODO: How do we pay the relayer?
    // - what if a proof fails?
    //      - we can be sure that it is the relayers fault, because he had to check the proof for validity
    //      - lock the fees from the relayers balance (who enqueued it) and pay all other relayers from his balance
    //      - if proof now fails, he loses his money
    //      - if proof is approved, he get's paid his stake back and a fee (same fee as the others plus maybe a small bonus for the risk)
    let fee = compute_fee(0);

    // Add send request to queue


    Ok(())
}

/// Transfers the funds for a send request in the queue
pub fn finalize_send(
    queue_account: &mut QueueAccount,
    pool: &AccountInfo,
    recipient: &AccountInfo,
) -> ProgramResult {
    Ok(())
}