use solana_program::{
    entrypoint::ProgramResult,
};
use crate::proof::{
    SendVerificationKey,
    partial_prepare_inputs,
    partial_miller_loop,
    partial_final_exponentiation,
    verify_proof,
    VerificationKey,
    MILLER_LOOP_ITERATIONS,
    FINAL_EXPONENTIATION_ITERATIONS
};
use crate::queue::proof_request::ProofRequest;
use crate::queue::proof_request::ProofRequest::*;
use crate::error::ElusivError;

use crate::queue::state::*;
use crate::proof::state::*;

/// Reset proof account or verify proof
pub fn compute_proof(
    queue_account: &mut QueueAccount,
    proof_account: &mut ProofAccount,
) -> ProgramResult {
    // Get first proof request from queue
    let request = queue_account.proof_queue.first()?;

    match request {
        Store { .. } => { compute_with_verification_key::<SendVerificationKey>(proof_account, request) },
        Bind { .. } => { compute_with_verification_key::<SendVerificationKey>(proof_account, request) },
        Send { .. } => { compute_with_verification_key::<SendVerificationKey>(proof_account, request) },
    }
}

fn compute_with_verification_key<VKey: VerificationKey>(
    proof_account: &mut ProofAccount,
    request: ProofRequest,
) -> ProgramResult {
    if !proof_account.get_is_finished() {   // Computation
        let iteration = proof_account.get_iteration() as usize;

        // Check whether verification is complete
        if iteration >= VKey::FULL_ITERATIONS {
            return Err(ElusivError::ProofComputationIsAlreadyFinished.into());
        }

        // Prepare inputs
        if iteration < VKey::PREPARE_INPUTS_ITERATIONS {
            if iteration == 0 { proof_account.set_round(0); }

            partial_prepare_inputs::<VKey>(proof_account, iteration)?;
        } else

        // Miller loop
        if iteration < VKey::PREPARE_INPUTS_ITERATIONS + MILLER_LOOP_ITERATIONS {
            let base = VKey::PREPARE_INPUTS_ITERATIONS;
            if iteration == base { proof_account.set_round(0); }

            partial_miller_loop::<VKey>(proof_account, iteration - base)?;
        } else

        // Final exponentiation
        if iteration < VKey::PREPARE_INPUTS_ITERATIONS + MILLER_LOOP_ITERATIONS + FINAL_EXPONENTIATION_ITERATIONS {
            let base = VKey::PREPARE_INPUTS_ITERATIONS + MILLER_LOOP_ITERATIONS;
            if iteration == base { proof_account.set_round(0); }

            partial_final_exponentiation(proof_account, iteration - base);
        } else {
            return Ok(())
        }

        proof_account.set_iteration(iteration as u64 + 1);
        proof_account.serialize();

        Ok(())
    } else {    // Reset
        proof_account.reset_with_request::<VKey>(request)
    }    
}

/// Check if proof is valid, store nullifier_hash, enqueue commitments
pub fn finalize_proof(
    queue_account: &mut QueueAccount,
    proof_account: &mut ProofAccount,
) -> ProgramResult {
    // Get first proof request from queue
    let request = queue_account.proof_queue.first()?;

    // Verify that proof computation is complete

    // Verify proof

    // Verify public inputs
    // Check for nullifier_hash
    // Check for commitments in storage account
    // Check for commitments in queue

    // Reset proof account (set is_finished to true)
    proof_account.set_is_finished(true);

    // Try init proof
    // Dequeue request

    Ok(())
}