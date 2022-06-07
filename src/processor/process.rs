use ark_bn254::Fr;
use ark_ff::Zero;
use solana_program::{entrypoint::ProgramResult, account_info::AccountInfo};
use crate::commitment::poseidon_hash::TOTAL_POSEIDON_ROUNDS;
use crate::macros::guard;
use crate::state::MT_HEIGHT;
use crate::state::{
    NullifierAccount,
    StorageAccount,
    program_account::{MultiInstanceAccount, ProgramAccount, MultiAccountProgramAccount},
};
use crate::state::queue::{
    RingQueue,
    Queue,
    QueueManagementAccount,
    ProofRequest,FinalizeSendRequest,
    MergeProofQueue,MergeProofQueueAccount,
    MigrateProofQueue,MigrateProofQueueAccount,
    FinalizeSendQueue,FinalizeSendQueueAccount,
    CommitmentQueue,CommitmentQueueAccount,
    BaseCommitmentQueue,BaseCommitmentQueueAccount,
};
use crate::error::ElusivError::{
    InvalidAccount,
    InvalidInstructionData,
    ComputationIsNotYetFinished,
    ComputationIsAlreadyFinished,
    InvalidProof,
    CannotFinalizeBinaryProof,
    InvalidFeePayer,
    NoRoomForCommitment,
};
use crate::proof::{
    VerificationAccount,
    verifier::verify_partial,
    vkey::{
        SendBinaryVKey,
        MergeBinaryVKey,
        MigrateUnaryVKey,
    },
};
use crate::commitment::{
    BaseCommitmentHashingAccount,
    CommitmentHashingAccount,
    poseidon_hash::{binary_poseidon_hash_partial},
    BaseCommitmentHashComputation,
    CommitmentHashComputation,
};
use crate::types::ProofKind;
use super::utils::send_from_pool;
use crate::fields::{u256_to_fr, fr_to_u256_le};
use elusiv_computation::{PartialComputation, PartialComputationInstruction};

/// Dequeues a proof request and places it into a `VerificationAccount`
macro_rules! init_proof {
    ($queue_account_ty: ty, $queue_ty: ty, $queue: ident, $verification_account: ident, $kind: ident, $vkey: ty) => {
        {
            let mut queue_data = &mut $queue.data.borrow_mut()[..];
            let mut queue = <$queue_account_ty>::new(&mut queue_data)?;
            let mut queue = <$queue_ty>::new(&mut queue);
            let request = queue.dequeue_first()?.request;
            $verification_account.reset::<$vkey>(ProofRequest::$kind { request })
        }
    };
}

/// Ensures that a `PartialComputation` is finished
macro_rules! partial_computation_is_finished {
    ($computation: ty, $account: ident) => {
        guard!(
            $account.get_instruction() as usize == <$computation>::INSTRUCTIONS.len(),
            ComputationIsNotYetFinished
        );
    };
}

pub fn verify_proof(
    verification_account: &mut VerificationAccount,
) -> ProgramResult {
    Ok(()) 
}

pub fn init_proof(
    queue: &AccountInfo,
    verification_account: &mut VerificationAccount,

    kind: ProofKind,
    verification_account_index: u64,
) -> ProgramResult {
    guard!(verification_account.is_valid(verification_account_index), InvalidAccount);
    guard!(!verification_account.get_is_active(), ComputationIsNotYetFinished);

    match kind {
        ProofKind::Send => {
            init_proof!(MergeProofQueueAccount, MergeProofQueue, queue, verification_account, Merge, SendBinaryVKey)
        },
        ProofKind::Merge => {
            init_proof!(MergeProofQueueAccount, MergeProofQueue, queue, verification_account, Merge, MergeBinaryVKey)
        },
        ProofKind::Migrate => {
            init_proof!(MigrateProofQueueAccount, MigrateProofQueue, queue, verification_account, Migrate, MigrateUnaryVKey)
        }
    }.or(Err(InvalidInstructionData.into()))
}

/// Partial proof verification computation
pub fn compute_proof(
    verification_account: &mut VerificationAccount,
    verification_account_index: u64,
) -> ProgramResult {
    /*guard!(verification_account.is_valid(verification_account_index), InvalidAccount);
    guard!(verification_account.get_is_active(), ComputationIsNotYetFinished);

    let request = verification_account.get_request();
    let round = verification_account.get_round();

    match match request {
        ProofRequest::Send { .. } => verify_partial::<SendVerificationKey>(round as usize, verification_account),
        ProofRequest::Merge { .. } => verify_partial::<MergeVerificationKey>(round as usize, verification_account),
        ProofRequest::Migrate { .. } => verify_partial::<MigrateVerificationKey>(round as usize, verification_account),
    } {
        Ok(result) => match result {
            Some(final_result) => { // After last round we receive the verification result
                if final_result {
                    verification_account.set_is_verified(&true);
                } else {
                    verification_account.set_is_active(&false);
                }
            },
            None => {}
        },
        Err(e) => { // An error can only happen with flawed inputs -> cancel verification
            verification_account.set_is_active(&false);
            return Err(e.into());
        }
    }

    // Serialize rams
    verification_account.serialize_rams();

    verification_account.set_round(&(round + 1));
*/
    Ok(())
}

/// Finalizes proofs of arity two
/// - `original_fee_payer` is the fee payer that payed the computation fee upfront
/// - for Send: enqueue a `FinalizeSendRequest`, enqueue commitment, save nullifier-hashes
/// - for Merge: enqueue commitment, save nullifier-hashes
/// - for Migrate: enqueue commitment, update NSMT-root
pub fn finalize_proof<'a>(
    original_fee_payer: &AccountInfo<'a>,
    pool: &AccountInfo<'a>,
    verification_account: &mut VerificationAccount,
    commitment_hash_queue: &mut CommitmentQueueAccount,
    finalize_send_queue: &mut FinalizeSendQueueAccount,
    nullifier_account0: &mut NullifierAccount,
    nullifier_account1: &mut NullifierAccount,

    verification_account_index: u64,
    tree_indices: [u64; 2],
) -> ProgramResult {
    guard!(verification_account.is_valid(verification_account_index), InvalidAccount);
    guard!(verification_account.get_is_active(), ComputationIsNotYetFinished);
    guard!(verification_account.get_is_verified(), InvalidProof);

    let mut commitment_queue = CommitmentQueue::new(commitment_hash_queue);

    match verification_account.get_request() {
        ProofRequest::Send { request } => {
            // Check for correct trees and insert nullifiers
            guard!(tree_indices[0] == request.proof_data.tree_indices[0], InvalidAccount);
            guard!(tree_indices[1] == request.proof_data.tree_indices[1], InvalidAccount);
            nullifier_account0.insert_nullifier_hash(request.public_inputs.join_split.nullifier_hashes[0])?;
            nullifier_account1.insert_nullifier_hash(request.public_inputs.join_split.nullifier_hashes[1])?;

            // Enqueue send request, commitment
            let mut queue = FinalizeSendQueue::new(finalize_send_queue);
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
            nullifier_account0.insert_nullifier_hash(request.public_inputs.join_split.nullifier_hashes[0])?;
            nullifier_account1.insert_nullifier_hash(request.public_inputs.join_split.nullifier_hashes[1])?;

            // Enqueue commitment
            commitment_queue.enqueue(request.public_inputs.join_split.commitment)?;

            // Repay fee_payer
            guard!(original_fee_payer.key.to_bytes() == request.fee_payer, InvalidFeePayer);
            send_from_pool(pool, original_fee_payer, 0)?;
        },

        ProofRequest::Migrate { request } => {
            // Check for correct tree and insert nullifier
            guard!(tree_indices[0] == request.proof_data.tree_indices[0], InvalidAccount);
            nullifier_account0.insert_nullifier_hash(request.public_inputs.join_split.nullifier_hashes[0])?;

            // Enqueue commitment
            commitment_queue.enqueue(request.public_inputs.join_split.commitment)?;

            // Repay fee_payer
            guard!(original_fee_payer.key.to_bytes() == request.fee_payer, InvalidFeePayer);
            send_from_pool(pool, original_fee_payer, 0)?;

            panic!("NSTM not implemented")
        }
    }

    Ok(())
}

/// Dequeues a base commitment hashing request and places it in the `BaseCommitmentHashingAccount`
/// - this request will result in a single hash computation
/// - computation: `commitment = h(base_commitment, amount)` (https://github.com/elusiv-privacy/circuits/blob/master/circuits/commitment.circom)
pub fn init_base_commitment_hash(
    fee_payer: &AccountInfo,
    q_manager: &QueueManagementAccount,
    queue: &mut BaseCommitmentQueueAccount,
    hashing_account: &mut BaseCommitmentHashingAccount,

    base_commitment_hash_account_index: u64,
) -> ProgramResult {
    // TODO: queue is implemented wrong, we need to split `is_being_processed` elements somehow

    guard!(q_manager.get_finished_setup(), InvalidAccount);
    guard!(hashing_account.is_valid(base_commitment_hash_account_index), InvalidAccount);
    guard!(!hashing_account.get_is_active(), ComputationIsNotYetFinished);

    let mut queue = BaseCommitmentQueue::new(queue);
    let request = queue.process_first()?;
    hashing_account.reset(request, fee_payer.key.to_bytes())
}

pub fn compute_base_commitment_hash(
    hashing_account: &mut BaseCommitmentHashingAccount,

    base_commitment_hash_account_index: u64,
    _nonce: u64,
) -> ProgramResult {
    guard!(hashing_account.is_valid(base_commitment_hash_account_index), InvalidAccount);
    guard!(hashing_account.get_is_active(), ComputationIsNotYetFinished);

    let instruction = hashing_account.get_instruction();
    let start_round = BaseCommitmentHashComputation::INSTRUCTIONS[instruction as usize].start_round;
    let rounds = BaseCommitmentHashComputation::INSTRUCTIONS[instruction as usize].rounds;

    // Read state
    let mut state = [
        u256_to_fr(&hashing_account.get_state(0)),
        u256_to_fr(&hashing_account.get_state(1)),
        u256_to_fr(&hashing_account.get_state(2)),
    ];

    // Hash computation
    for round in start_round..start_round + rounds {
        guard!(round < BaseCommitmentHashComputation::TOTAL_ROUNDS, ComputationIsAlreadyFinished);
        binary_poseidon_hash_partial(round, &mut state);
    }

    // Update state
    hashing_account.set_state(0, &fr_to_u256_le(&state[0]));
    hashing_account.set_state(1, &fr_to_u256_le(&state[1]));
    hashing_account.set_state(2, &fr_to_u256_le(&state[2]));

    hashing_account.set_instruction(&(instruction + 1));

    Ok(())
}

pub fn finalize_base_commitment_hash(
    q_manager: &QueueManagementAccount,
    base_commitment_hash_queue: &mut BaseCommitmentQueueAccount,
    commitment_hash_queue: &mut CommitmentQueueAccount,
    hashing_account: &mut BaseCommitmentHashingAccount,

    base_commitment_hash_account_index: u64,
) -> ProgramResult {
    guard!(q_manager.get_finished_setup(), InvalidAccount);
    guard!(hashing_account.is_valid(base_commitment_hash_account_index), InvalidAccount);
    guard!(hashing_account.get_is_active(), ComputationIsNotYetFinished);
    partial_computation_is_finished!(BaseCommitmentHashComputation, hashing_account);

    let result = hashing_account.get_state(0);

    // Check that first queue-element is the finished one, then dequeue it
    let mut base_commitment_queue = BaseCommitmentQueue::new(base_commitment_hash_queue);
    let first = base_commitment_queue.dequeue_first()?;
    guard!(first.request.commitment == result, ComputationIsNotYetFinished);

    // If the client sent a flawed commitment value, we will not insert the commitment
    let mut commitment_queue = CommitmentQueue::new(commitment_hash_queue);
    commitment_queue.enqueue(result)?;

    hashing_account.set_is_active(&false);

    Ok(())
}

/// Reads a commitment hashing request and places it in the `CommitmentHashingAccount`
pub fn init_commitment_hash(
    fee_payer: &AccountInfo,
    q_manager: &QueueManagementAccount,
    queue: &mut CommitmentQueueAccount,
    hashing_account: &mut CommitmentHashingAccount,
    storage_account: &StorageAccount,
) -> ProgramResult {
    guard!(q_manager.get_finished_setup(), InvalidAccount);
    guard!(!hashing_account.get_is_active(), ComputationIsNotYetFinished);
    guard!(!storage_account.is_full(), NoRoomForCommitment);

    let mut queue = CommitmentQueue::new(queue);
    let request = queue.process_first()?;

    // Get hashing siblings
    let ordering = storage_account.get_next_commitment_ptr();
    let siblings = storage_account.get_mt_opening(ordering as usize);

    // Reset values and get hashing siblings from storage account
    hashing_account.reset(request, ordering, siblings, fee_payer.key.to_bytes())
}

pub fn compute_commitment_hash(
    hashing_account: &mut CommitmentHashingAccount,

    _nonce: u64,
) -> ProgramResult {
    guard!(hashing_account.get_is_active(), ComputationIsNotYetFinished);

    let instruction = hashing_account.get_instruction();
    let start_round = CommitmentHashComputation::INSTRUCTIONS[instruction as usize].start_round;
    let rounds = CommitmentHashComputation::INSTRUCTIONS[instruction as usize].rounds;

    // Read state
    let mut state = [
        u256_to_fr(&hashing_account.get_state(0)),
        u256_to_fr(&hashing_account.get_state(1)),
        u256_to_fr(&hashing_account.get_state(2)),
    ];

    // Hash computation
    for round in start_round..start_round + rounds {
        guard!(round < CommitmentHashComputation::TOTAL_ROUNDS, ComputationIsAlreadyFinished);

        binary_poseidon_hash_partial(round % TOTAL_POSEIDON_ROUNDS, &mut state);

        // A single hash is finished
        if round % TOTAL_POSEIDON_ROUNDS == 64 {
            let hash_num = round / TOTAL_POSEIDON_ROUNDS;
            let ordering = hashing_account.get_ordering();
            let offset = (ordering >> (hash_num + 1)) % 2;

            // Save hash
            let hash = state[0];
            hashing_account.set_finished_hashes(hash_num as usize, &fr_to_u256_le(&hash));

            // Reset state for next hash
            if hash_num < MT_HEIGHT - 1 {
                state[0] = Fr::zero();
                state[1 + offset as usize] = hash;
                state[2 - offset as usize] = hashing_account.get_siblings(hash_num as usize + 1).0;
            }
        }
    }

    // Update state
    hashing_account.set_state(0, &fr_to_u256_le(&state[0]));
    hashing_account.set_state(1, &fr_to_u256_le(&state[1]));
    hashing_account.set_state(2, &fr_to_u256_le(&state[2]));

    hashing_account.set_instruction(&(instruction + 1));

    Ok(())
}

pub fn finalize_commitment_hash(
    q_manager: &QueueManagementAccount,
    queue: &mut CommitmentQueueAccount,
    hashing_account: &mut CommitmentHashingAccount,
    storage_account: &mut StorageAccount,
) -> ProgramResult {
    guard!(q_manager.get_finished_setup(), InvalidAccount);
    guard!(hashing_account.get_is_active(), ComputationIsNotYetFinished);
    partial_computation_is_finished!(CommitmentHashComputation, hashing_account);

    // Dequeue request
    let mut commitment_queue = CommitmentQueue::new(queue);
    let commitment = commitment_queue.dequeue_first()?;

    // Insert commitment and hashes and save last root
    let mut values = vec![commitment.request];
    for i in 0..MT_HEIGHT as usize { values.push(hashing_account.get_finished_hashes(i)); }
    storage_account.insert_commitment(&values);

    hashing_account.set_is_active(&false);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{state::{program_account::SizedAccount, EMPTY_TREE, queue::BaseCommitmentHashRequest}, commitment::poseidon_hash::full_poseidon2_hash, fields::u64_to_scalar};
    use crate::state::MT_HEIGHT;
    use solana_program::native_token::LAMPORTS_PER_SOL;
    use std::str::FromStr;

    #[test]
    fn test_compute_base_commitment_hash() {
        let mut data = vec![0; BaseCommitmentHashingAccount::SIZE];
        let mut hashing_account = BaseCommitmentHashingAccount::new(&mut data).unwrap();

        // Setup hashing account
        let bc = Fr::from_str("8337064132573119120838379738103457054645361649757131991036638108422638197362").unwrap();
        let base_commitment = fr_to_u256_le(&bc);
        let c = Fr::from_str("139214303935475888711984321184227760578793579443975701453971046059378311483").unwrap();
        let commitment = fr_to_u256_le(&c);
        let amount = LAMPORTS_PER_SOL;

        // Reset values and get hashing siblings from storage account
        hashing_account.reset(BaseCommitmentHashRequest {
            base_commitment,
            amount,
            commitment,
        }, [0; 32]).unwrap();

        // Compute hash
        for i in 1..=BaseCommitmentHashComputation::INSTRUCTIONS.len() {
            compute_base_commitment_hash(&mut hashing_account, 0, 0).unwrap();
            assert_eq!(hashing_account.get_instruction(), i as u32);
        }

        let expected = full_poseidon2_hash(bc, u64_to_scalar(amount));

        // Check commitment
        let result = hashing_account.get_state(0);
        assert_eq!(u256_to_fr(&result), expected);
        assert_eq!(result, commitment);
    }

    #[test]
    fn test_compute_commitment_hash() {
        let mut data = vec![0; CommitmentHashingAccount::SIZE];
        let mut hashing_account = CommitmentHashingAccount::new(&mut data).unwrap();

        // Setup hashing account
        let c = Fr::from_str("17943901642092756540532360474161569402553221410028090072917443956036363428842").unwrap();
        let commitment = fr_to_u256_le(&c);
        let ordering = 0;

        // Siblings are the default values of the MT
        let mut siblings = [Fr::zero(); MT_HEIGHT as usize];
        for i in 0..MT_HEIGHT as usize { siblings[i] = EMPTY_TREE[i]; } 

        // Reset values and get hashing siblings from storage account
        hashing_account.reset(commitment, ordering, siblings, [0; 32]).unwrap();

        // Compute hashes
        for i in 1..=CommitmentHashComputation::INSTRUCTIONS.len() {
            compute_commitment_hash(&mut hashing_account, 0).unwrap();
            assert_eq!(hashing_account.get_instruction(), i as u32);
        }

        // Check hashes with hash-function
        let mut hash = c;

        for i in 0..MT_HEIGHT as usize {
            hash = full_poseidon2_hash(hash, EMPTY_TREE[i]);
            assert_eq!(hashing_account.get_siblings(i).0, EMPTY_TREE[i]);
            assert_eq!(hashing_account.get_finished_hashes(i), fr_to_u256_le(&hash));
        }

        // Check root
        assert_eq!(
            hashing_account.get_finished_hashes(MT_HEIGHT as usize - 1),
            fr_to_u256_le(&Fr::from_str("13088350874257591466321551177903363895529460375369348286819794485219676679592").unwrap())
        );
    }
}