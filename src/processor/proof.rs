use solana_program::{
    entrypoint::ProgramResult,
};
use crate::proof::SendVerificationKey;
use crate::queue::proof_request::ProofRequest;
use super::super::error::ElusivError;

use super::super::queue::state::*;
use super::super::proof::state::*;

/// Store first store, bind or send request from queue in proof account
pub fn init_proof(
    queue_account: &mut QueueAccount,
    proof_account: &mut ProofAccount,
) -> ProgramResult {
    // Check if proof account is in reset state
    if !proof_account.get_is_finished() {
        return Err(ElusivError::ProofAccountCannotBeReset.into());
    }

    // Get first proof request from queue
    let request = queue_account.proof_queue.first()?;

    // Reset proof account with matching verification key
    match request {
        ProofRequest::Store { .. } => {
            proof_account.reset_with_request::<SendVerificationKey>(request)
        },
        ProofRequest::Bind { .. } => {
            proof_account.reset_with_request::<SendVerificationKey>(request)
        },
        ProofRequest::Send { .. } => {
            proof_account.reset_with_request::<SendVerificationKey>(request)
        },
    }
}

/// Verify proof
pub fn compute_proof() -> ProgramResult {
    Ok(())
}

/// Check if proof is valid, store nullifier_hash, enqueue commitments
pub fn finalize_proof(
    _queue_account: &mut QueueAccount,
    proof_account: &mut ProofAccount,
) -> ProgramResult {
    // Check for nullifier_hash
    // Check for commitments in storage account
    // Check for commitments in queue

    // Reset proof account (set is_finished to true)
    proof_account.set_is_finished(true);

    // Try init proof
    // Dequeue request

    Ok(())
}