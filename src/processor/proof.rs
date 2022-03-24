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
};
use crate::queue::proof_request::ProofRequest;
use crate::queue::proof_request::ProofRequest::*;
use crate::error::ElusivError;

use crate::queue::send_finalization_request::SendFinalizationRequest;
use crate::queue::state::*;
use crate::proof::state::*;
use crate::state::StorageAccount;

macro_rules! execute_with_vkey {
    ($request: expr, $fun: ident, $proof_account: expr) => {
        match $request {
            Store { .. } => { $fun::<SendVerificationKey>($proof_account, $request) },
            Bind { .. } => { $fun::<SendVerificationKey>($proof_account, $request) },
            Send { .. } => { $fun::<SendVerificationKey>($proof_account, $request) },
        }
    };
}

/// Reset proof account or verify proof
pub fn compute_proof(
    queue_account: &mut QueueAccount,
    proof_account: &mut ProofAccount,
) -> ProgramResult {
    let request = queue_account.proof_queue.first()?;
    execute_with_vkey!(request, compute_proof_with_vkey, proof_account)
}

macro_rules! match_partial_computation {
    ($range: expr, $fun: expr, $iteration: expr, $proof_account: expr) => { 
        if $iteration >= $range.0 && $iteration < $range.1 {
            // Reset round before first iteration
            if $iteration == $range.0 {
                $proof_account.set_round(0);
            }

            // Partial computation call
            $fun($proof_account, $iteration - $range.0)?;

            // Inc iteration and serialize proof_account changes
            $proof_account.set_iteration($iteration as u64 + 1);
            $proof_account.serialize();

            return Ok(());
        }
    };
}

fn compute_proof_with_vkey<VKey: VerificationKey>(
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
        match_partial_computation!(VKey::PREPARE_INPUTS, partial_prepare_inputs::<VKey>, iteration, proof_account);

        // Miller loop
        match_partial_computation!(VKey::MILLER_LOOP, partial_miller_loop::<VKey>, iteration, proof_account);

        // Final exponentiation
        match_partial_computation!(VKey::FINAL_EXPONENTIATION, partial_final_exponentiation, iteration, proof_account);
        
        Ok(())
    } else {    // Reset
        proof_account.reset_with_request::<VKey>(request)
    }    
}

/// Check if proof is valid, store nullifier_hash, enqueue commitments or send finalization request
pub fn finalize_proof(
    storage_account: &mut StorageAccount,
    queue_account: &mut QueueAccount,
    proof_account: &mut ProofAccount,
) -> ProgramResult {
    let request = queue_account.proof_queue.first()?;

    // Verify proof
    execute_with_vkey!(request, finalize_proof_with_vkey, proof_account)?;

    // Check for nullifier_hash & insert
    let nullifier_hash = request.get_proof_data().nullifier_hash;
    storage_account.can_insert_nullifier_hash(nullifier_hash)?;
    storage_account.insert_nullifier_hash(nullifier_hash)?;

    // Check for commitments in storage account and queue
    for commitment in request.get_commitments() {
        storage_account.can_insert_commitment(commitment)?;
        if queue_account.commitment_queue.contains(commitment) {
            return Err(ElusivError::CommitmentAlreadyUsed.into());
        }
    }

    // Remove request from queue
    queue_account.proof_queue.dequeue()?;

    // Enqueue commitments
    for commitment in request.get_commitments() {
        queue_account.commitment_queue.enqueue(commitment)?;
    }

    // Send request -> add send finalization request to queue
    if let Send { proof_data, recipient, .. } = request {
        queue_account.send_queue.enqueue(
            SendFinalizationRequest {
                amount: proof_data.amount,
                recipient
            }
        )?;
    }

    Ok(())
}

fn finalize_proof_with_vkey<VKey: VerificationKey>(
    proof_account: &mut ProofAccount,
    _request: ProofRequest,
) -> ProgramResult {
    let iteration = proof_account.get_iteration() as usize;

    // Verify proof (+ check that computation is finished)
    if !verify_proof::<VKey>(proof_account, iteration)? {
        return Err(ElusivError::InvalidProof.into());
    }

    // Reset proof account (set is_finished to true)
    proof_account.set_is_finished(true);

    Ok(())
}