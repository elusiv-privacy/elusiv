use solana_program::entrypoint::ProgramResult;
use crate::state::TREE_HEIGHT;
use crate::macros::guard;
use crate::{
    commitment::CommitmentAccount,
    queue::state::QueueAccount,
    state::StorageAccount,
    error::ElusivError::{
        CommitmentComputationIsNotYetFinished,
        CommitmentComputationIsAlreadyFinished,
        CommitmentAlreadyExists,
        HashingIsAlreadyComplete,
        DidNotFinishHashing,
    },
    types::U256_ZERO
};
use crate::commitment::Poseidon2;
use crate::fields::scalar::{ from_bytes_le_mont, to_bytes_le_mont };

/// Store first commitment from queue in commitment account
pub fn init_commitment(
    storage_account: &StorageAccount,
    queue_account: &mut QueueAccount,
    commitment_account: &mut CommitmentAccount,
) -> ProgramResult {
    // Dequeue commitment
    let commitment = queue_account.commitment_queue.dequeue_first()?;

    // Check if commitment account is in reset state
    guard!(
        !commitment_account.get_is_active(),
        CommitmentComputationIsNotYetFinished
    );

    // Check if commitment is new
    storage_account.can_insert_commitment(commitment)?;
    guard!(
        !queue_account.commitment_queue.contains(commitment),
        CommitmentAlreadyExists
    );

    // Reset commitment account
    commitment_account.reset(storage_account, commitment)?;

    Ok(())
}

/// Compute hashes for commitment
pub fn compute_commitment(
    commitment_account: &mut CommitmentAccount,
) -> ProgramResult {
    // Check that commitment account is active
    guard!(
        commitment_account.get_is_active(),
        CommitmentComputationIsAlreadyFinished
    );

    let mut iteration = commitment_account.get_current_hash_iteration() as usize;
    let mut tree_position = commitment_account.get_current_hash_tree_position() as usize;
    let mut state = [
        commitment_account.get_hashing_state(0),
        commitment_account.get_hashing_state(1),
        commitment_account.get_hashing_state(2),
    ];

    // Check that computation is complete
    guard!(
        tree_position < crate::state::TREE_HEIGHT && iteration == crate::commitment::ITERATIONS,
        HashingIsAlreadyComplete
    );

    // Move to next tree level
    if iteration as usize == crate::commitment::ITERATIONS {
        // Save complted hash
        let hash = state[0];
        commitment_account.set_finished_hashes(tree_position as usize, &hash);

        // Reset values
        let index = commitment_account.get_leaf_index() >> (tree_position as usize);
        let neighbour = commitment_account.get_opening(tree_position);
        let last_hash_is_left = (index & 1) == 0;
        tree_position += 1;
        iteration = 0;

        // Set new inputs
        state[0] = U256_ZERO;
        state[1] = if last_hash_is_left { hash } else { neighbour };
        state[2] = if last_hash_is_left { neighbour } else { hash };

        // Finished
        if tree_position as usize == crate::state::TREE_HEIGHT + 1 { return Ok(()) }
    }

    // Partial hashing
    let hash = Poseidon2::new().partial_hash(
        iteration as usize,
        from_bytes_le_mont(&state[0]),
        from_bytes_le_mont(&state[1]),
        from_bytes_le_mont(&state[2]),
    );

    // Save values
    commitment_account.set_hashing_state(0, &to_bytes_le_mont(hash[0]));
    commitment_account.set_hashing_state(1, &to_bytes_le_mont(hash[1]));
    commitment_account.set_hashing_state(2, &to_bytes_le_mont(hash[2]));

    commitment_account.set_current_hash_iteration(iteration as u64 + 1);
    commitment_account.set_current_hash_tree_position(tree_position as u64);

    Ok(())
}

/// Store commitment and hashes in storage account
pub fn finalize_commitment(
    storage_account: &mut StorageAccount,
    commitment_account: &mut CommitmentAccount,
) -> ProgramResult {
    let iteration = commitment_account.get_current_hash_iteration() as usize;
    let tree_position = commitment_account.get_current_hash_tree_position() as usize;

    // Check that computation is complete
    guard!(
        iteration == crate::commitment::ITERATIONS && tree_position == crate::state::TREE_HEIGHT,
        DidNotFinishHashing
    );

    // Add commitment and hashes into storage account
    let mut values = [U256_ZERO; TREE_HEIGHT + 1];
    for i in 0..=TREE_HEIGHT {
        values[i] = commitment_account.get_finished_hashes(i);
    }
    storage_account.insert_commitment(values)?;

    // Deactivate commitment account
    commitment_account.set_is_active(false);

    Ok(())
}