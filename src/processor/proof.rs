use solana_program::entrypoint::ProgramResult;
use crate::macros::guard;
use crate::proof::{
    vkey::*,
    partial_prepare_inputs,
    partial_miller_loop,
    partial_final_exponentiation,
    verify_proof,
    VerificationKey,
};
use crate::queue::proof_request::*;
use crate::error::ElusivError:: {
    ProofComputationIsNotYetFinished,
    ProofComputationIsAlreadyFinished,
    InvalidNullifierAccount,
    CommitmentAlreadyUsed,
};
use crate::queue::send_finalization_request::SendFinalizationRequest;
use crate::queue::state::*;
use crate::queue::proof_request::ProofRequestKind::*;
use crate::proof::state::*;
use crate::state::StorageAccount;
use crate::state::NullifierAccount;

macro_rules! execute_with_vkey {
    ($request: expr, $fun: ident, $proof_account: expr) => {
        match $request.kind {
            Store { .. } => { $fun::<StoreVerificationKey>($proof_account, $request) },
            Bind { .. } => { $fun::<SendVerificationKey>($proof_account, $request) },
            Send { .. } => { $fun::<BindVerificationKey>($proof_account, $request) },
        }
    };
}

/// Reset proof account
pub fn init_proof(
    queue_account: &mut QueueAccount,
    proof_account: &mut ProofAccount,
) -> ProgramResult {
    // Check that proof account can be reset
    guard!(
        !proof_account.get_is_active(),
        ProofComputationIsNotYetFinished
    );

    // Dequeue request
    let request = queue_account.proof_queue.dequeue_first()?;

    execute_with_vkey!(request, reset_with_request, proof_account)
}

/// Verify proof
pub fn compute_proof(
    proof_account: &mut ProofAccount,
) -> ProgramResult {
    // Check that proof account is active
    guard!(
        proof_account.get_is_active(),
        ProofComputationIsAlreadyFinished
    );

    let request = ProofRequest::deserialize(proof_account.request);

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
    _request: ProofRequest,
) -> ProgramResult {
    let iteration = proof_account.get_iteration() as usize;

    // Check that verification is not yet complete
    guard!(
        iteration < VKey::FULL_ITERATIONS,
        ProofComputationIsAlreadyFinished
    );

    {
        // Prepare inputs
        match_partial_computation!(VKey::PREPARE_INPUTS, partial_prepare_inputs::<VKey>, iteration, proof_account);

        // Miller loop
        match_partial_computation!(VKey::MILLER_LOOP, partial_miller_loop::<VKey>, iteration, proof_account);

        // Final exponentiation
        match_partial_computation!(VKey::FINAL_EXPONENTIATION, partial_final_exponentiation, iteration, proof_account);
    }

    // Increment iteration
    proof_account.set_iteration(iteration as u64 + 1);

    // Save final result if finished
    {
        // Skip if computation is not finished
        if crate::proof::is_computation_finished::<VKey>(proof_account) {
            return Ok(())
        }

        // Verify proof (+ check that computation is finished)
        if verify_proof::<VKey>(proof_account)? {
            proof_account.set_is_verified(true);
        } else {
            // If the proof is invalid, we allow the account to be reset
            proof_account.set_is_active(false);
            proof_account.set_is_verified(false);
        }
    }
    
    Ok(())
}

/// Check if proof is valid, store nullifier, enqueue commitments or send finalization request
pub fn finalize_proof(
    storage_account: &mut StorageAccount,
    nullifier_account: &mut NullifierAccount,
    queue_account: &mut QueueAccount,
    proof_account: &mut ProofAccount,
) -> ProgramResult {
    let request = ProofRequest::deserialize(proof_account.request);

    // Check that nullifier account is correct
    guard!(
        nullifier_account.get_key() == request.nullifier_account,
        InvalidNullifierAccount
    );

    // Check that computation is finished
    execute_with_vkey!(request, is_computation_finished, proof_account)?;

    // Reset proof account
    proof_account.set_is_active(false);

    // Check if proof is verified
    if proof_account.get_is_verified() {    // Valid proof
        // Check nullifier & insert
        let nullifier = request.proof_data.nullifier;
        nullifier_account.can_insert_nullifier(nullifier)?;
        nullifier_account.insert_nullifier(nullifier)?;

        // Check for commitments in storage account and queue
        for commitment in request.get_commitments() {
            storage_account.can_insert_commitment(commitment)?;
            guard!(
                !queue_account.commitment_queue.contains(commitment),
                CommitmentAlreadyUsed
            );
        }

        // Enqueue commitments
        for commitment in request.get_commitments() {
            queue_account.commitment_queue.enqueue(commitment)?;
        }

        // Send request -> add send finalization request to queue
        if let Send { recipient } = request.kind {
            queue_account.send_queue.enqueue(
                SendFinalizationRequest {
                    amount: request.proof_data.amount,
                    recipient
                }
            )?;
        }
    }

    Ok(())
}

fn is_computation_finished<VKey: VerificationKey>(
    proof_account: &mut ProofAccount,
    _request: ProofRequest,
) -> ProgramResult {
    guard!(
        crate::proof::is_computation_finished::<VKey>(proof_account),
        ProofComputationIsNotYetFinished
    );

    Ok(())
}