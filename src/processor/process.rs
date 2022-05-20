use solana_program::{entrypoint::ProgramResult, account_info::AccountInfo};
use crate::macros::guard;
use crate::state::NullifierAccount;
use crate::state::queue::{
    RingQueue,
    ProofRequest,FinalizeSendRequest,
    SendProofQueue,SendProofQueueAccount,
    MergeProofQueue,MergeProofQueueAccount,
    MigrateProofQueue,MigrateProofQueueAccount,
    FinalizeSendQueue,FinalizeSendQueueAccount,
    CommitmentQueue,CommitmentQueueAccount,
};
use crate::error::ElusivError::{InvalidAccount, ComputationIsNotYetFinished, InvalidProof, CannotFinalizeUnaryProof, CannotFinalizeBinaryProof, InvalidFeePayer};
use crate::proof::{
    MAX_VERIFICATION_ACCOUNTS_COUNT,
    VerificationAccount, VerificationAccountWrapper,
    verifier::verify_partial,
    vkey::{
        SendVerificationKey,
        MergeVerificationKey,
        MigrateVerificationKey
    },
};
use super::utils::send_from_pool;

/// Dequeues a proof request and places it into a `VerificationAccount`
macro_rules! init_proof {
    ($fn_name: ident, $queue_ty: ty, $queue_account_ty: ty) => {
        pub fn $fn_name<'a>(
            queue: &mut $queue_account_ty,
            verification_account: &mut VerificationAccount,
            verification_account_index: u64,
        ) -> ProgramResult {
            guard!(verification_account_index < MAX_VERIFICATION_ACCOUNTS_COUNT, InvalidAccount);
            guard!(!verification_account.get_is_active(), ComputationIsNotYetFinished);
        
            let mut queue = <$queue_ty>::new(queue);
            let request = queue.dequeue_first()?;
        
            Ok(())
        }
    };
}

init_proof!(init_send_proof, SendProofQueue, SendProofQueueAccount);
init_proof!(init_merge_proof, MergeProofQueue, MergeProofQueueAccount);
init_proof!(init_migrate_proof, MigrateProofQueue, MigrateProofQueueAccount);

/// Partial proof verification computation
pub fn compute_proof<'a>(
    verification_account: &'a mut VerificationAccount<'a>,
    verification_account_index: u64,
) -> ProgramResult {
    guard!(verification_account_index < MAX_VERIFICATION_ACCOUNTS_COUNT, InvalidAccount);
    guard!(verification_account.get_is_active(), ComputationIsNotYetFinished);
    let mut wrapper = VerificationAccountWrapper::new(verification_account);

    let round = wrapper.account.get_round();

    match match verification_account.get_request() {
        ProofRequest::Send { .. } => verify_partial::<SendVerificationKey>(round as usize, &mut wrapper),
        ProofRequest::Merge { .. } => verify_partial::<MergeVerificationKey>(round as usize, &mut wrapper),
        ProofRequest::Migrate { .. } => verify_partial::<MigrateVerificationKey>(round as usize, &mut wrapper),
    } {
        Ok(result) => match result {
            Some(final_result) => { // After last round we receive the verification result
                if final_result {
                    verification_account.set_is_verified(true);
                } else {
                    verification_account.set_is_active(false);
                }
            },
            None => {}
        },
        Err(e) => { // An error can only happen with flawed inputs -> cancel verification
            verification_account.set_is_active(false);
            return Err(e);
        }
    }

    wrapper.account.set_round(round + 1);
    Ok(())
}

/// Finalizes proofs of arity two
/// - `original_fee_payer` is the fee payer that payed the computation fee upfront
/// - for Send: enqueue a `FinalizeSendRequest`, enqueue commitment, save nullifier-hashes
/// - for Merge: enqueue commitment, save nullifier-hashes
pub fn finalize_proof_binary<'a>(
    original_fee_payer: &AccountInfo<'a>,
    pool: &AccountInfo<'a>,
    verification_account: &'a mut VerificationAccount,
    commitment_hash_queue: &mut CommitmentQueueAccount,
    finalize_send_queue: &mut FinalizeSendQueueAccount,
    nullifier_account0: &NullifierAccount,
    nullifier_account1: &NullifierAccount,
    verification_account_index: u64,
    tree_indices: [u64; 2], // indices of the two trees into which the nullifiers will be inserted
) -> ProgramResult {
    guard!(verification_account_index < MAX_VERIFICATION_ACCOUNTS_COUNT, InvalidAccount);
    guard!(verification_account.get_is_active(), ComputationIsNotYetFinished);
    guard!(verification_account.get_is_verified(), InvalidProof);

    let commitment_queue = CommitmentQueue::new(commitment_hash_queue);

    match verification_account.get_request() {
        ProofRequest::Send { request } => {
            // Check for correct trees and insert nullifiers
            guard!(tree_indices[0] == request.proof_data.tree_indices[0], InvalidAccount);
            guard!(tree_indices[1] == request.proof_data.tree_indices[1], InvalidAccount);
            nullifier_account0.insert_nullifier(request.public_inputs.join_split.nullifier_hashes[0]);
            nullifier_account1.insert_nullifier(request.public_inputs.join_split.nullifier_hashes[1]);

            // Enqueue send request, commitment
            let queue = FinalizeSendQueue::new(finalize_send_queue);
            queue.enqueue(FinalizeSendRequest {
                amount: request.public_inputs.amount,
                recipient: request.public_inputs.recipient,
            })?;
            commitment_queue.enqueue(request.public_inputs.join_split.commitment)?;

            // Repay fee_payer
            guard!(original_fee_payer.key.to_bytes() == request.fee_payer, InvalidFeePayer);
            send_from_pool(pool, original_fee_payer, 0)?;
        },
        ProofRequest::Merge { request } => {
            // Check for correct trees and insert nullifiers
            guard!(tree_indices[0] == request.proof_data.tree_indices[0], InvalidAccount);
            guard!(tree_indices[1] == request.proof_data.tree_indices[1], InvalidAccount);
            nullifier_account0.insert_nullifier(request.public_inputs.join_split.nullifier_hashes[0]);
            nullifier_account1.insert_nullifier(request.public_inputs.join_split.nullifier_hashes[1]);

            // Enqueue commitment
            commitment_queue.enqueue(request.public_inputs.join_split.commitment)?;

            // Repay fee_payer
            guard!(original_fee_payer.key.to_bytes() == request.fee_payer, InvalidFeePayer);
            send_from_pool(pool, original_fee_payer, 0)?;
        },
        _ => return Err(CannotFinalizeUnaryProof.into()),
    }

    Ok(())
}

// Finalizes proofs of arity one
// - for Migrate: enqueue commitment, save nullifier-hash
pub fn finalize_proof_unary<'a>(
    original_fee_payer: &AccountInfo<'a>,
    pool: &AccountInfo<'a>,
    verification_account: &'a mut VerificationAccount,
    commitment_hash_queue: &mut CommitmentQueueAccount,
    nullifier_account: &NullifierAccount,
    verification_account_index: u64,
    tree_index: u64,
) -> ProgramResult {
    guard!(verification_account_index < MAX_VERIFICATION_ACCOUNTS_COUNT, InvalidAccount);
    guard!(verification_account.get_is_active(), ComputationIsNotYetFinished);
    guard!(verification_account.get_is_verified(), InvalidProof);

    let commitment_queue = CommitmentQueue::new(commitment_hash_queue);

    match verification_account.get_request() {
        ProofRequest::Migrate { request } => {
            // Check for correct tree and insert nullifier
            guard!(tree_index == request.proof_data.tree_indices[0], InvalidAccount);
            nullifier_account.insert_nullifier(request.public_inputs.join_split.nullifier_hashes[0]);

            // Enqueue commitment
            commitment_queue.enqueue(request.public_inputs.join_split.commitment)?;

            // Repay fee_payer
            guard!(original_fee_payer.key.to_bytes() == request.fee_payer, InvalidFeePayer);
            send_from_pool(pool, original_fee_payer, 0)?;
        },
        _ => return Err(CannotFinalizeBinaryProof.into()),
    }

    Ok(())
}