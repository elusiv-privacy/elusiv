use solana_program::{
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    native_token::LAMPORTS_PER_SOL,
    account_info::AccountInfo,
};
use crate::macros::guard;
use crate::types::{ ProofData, U256 };
use crate::error::ElusivError::{
    InvalidAmount,
    InvalidAccount,
    InvalidRecipient,
};
use crate::state::*;
use crate::queue::state::*;
use crate::queue::proof_request::{ProofRequest, ProofRequestKind};

pub fn store<'a>(
    sender: &AccountInfo<'a>,
    storage_account: &StorageAccount,
    nullifier_account: &NullifierAccount,
    queue_account: &mut QueueAccount,
    pool: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    proof_data: ProofData,
    commitments: Vec<U256>,
) -> ProgramResult {
    // Check public inputs
    check_public_inputs(
        storage_account,
        nullifier_account,
        &proof_data,
        &commitments,
    )?;

    let fee = compute_fee(0);

    // Add store/bind request to queue
    queue_account.proof_queue.enqueue(
        ProofRequest {
            proof_data,
            nullifier_account: nullifier_account.get_key(),
            fee,
            kind: 
                if commitments.len() == 1 {
                    ProofRequestKind::Store { commitment: commitments[0] }
                } else {
                    ProofRequestKind::Bind { unbound_commitment: commitments[0], bound_commitment: commitments[1] }
                }
        }
    )?;

    // Transfer funds + fees
    let lamports = proof_data.amount + fee;
    send_with_system_program(sender, pool, system_program, lamports)
}

fn check_public_inputs(
    storage_account: &StorageAccount,
    nullifier_account: &NullifierAccount,
    proof_data: &ProofData,
    commitments: &[U256],
) -> ProgramResult {
    // Check if commitments are new
    for &commitment in commitments {
        storage_account.can_insert_commitment(commitment)?;
    }

    // Check if root is valid by matching the nullifier account
    storage_account.is_root_valid(nullifier_account, proof_data.root)?;

    // Check if nullifier is new and not in queue
    // Note:
    // - This check itself does not prevent double spending, just a check to prevent spam/mistaken double
    // - The important check happens at the end of the proof verification
    nullifier_account.can_insert_nullifier(proof_data.nullifier)?;

    // Check amount
    guard!(
        proof_data.amount >= MINIMUM_AMOUNT,
        InvalidAmount
    );

    Ok(())
}



fn send_with_system_program<'a>(
    sender: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>,
    system_program: & AccountInfo<'a>,
    lamports: u64,
) -> ProgramResult {
    

    
}

pub fn send(
    storage_account: &StorageAccount,
    nullifier_account: &NullifierAccount,
    queue_account: &mut QueueAccount,
    proof_data: ProofData,
    recipient: U256,
) -> ProgramResult {
    // Check public inputs
    check_public_inputs(
        storage_account,
        nullifier_account,
        &proof_data,
        &vec![],
    )?;

    let fee = compute_fee(0);

    // Add send request to queue
    queue_account.proof_queue.enqueue(
        ProofRequest {
            proof_data,
            nullifier_account: nullifier_account.get_key(),
            fee,
            kind: ProofRequestKind::Send { recipient }
        }
    )?;

    Ok(())
}

pub fn finalize_send(
    queue_account: &mut QueueAccount,
    pool: &AccountInfo,
    recipient: &AccountInfo,
) -> ProgramResult {
    // Dequeue request
    let request = queue_account.send_queue.dequeue_first()?;

    // Check recipient
    guard!(
        *recipient.key == Pubkey::new_from_array(request.recipient),
        InvalidRecipient
    );

    // Transfer funds from pool to recipient
    let pool_balance = pool.try_lamports()?;
    let recipient_balance = pool.try_lamports()?;

    match pool_balance.checked_sub(request.amount) {
        Some(balance) => { **pool.try_borrow_mut_lamports()? = balance; },
        None => { return Err(InvalidAmount.into()); }
    }
    match recipient_balance.checked_add(request.amount) {
        Some(balance) => { **recipient.try_borrow_mut_lamports()? = balance; },
        None => { return Err(InvalidAmount.into()); }
    }

    Ok(())
}