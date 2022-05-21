use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    system_program,
    program_error::ProgramError,
};
use crate::types::{JoinSplitPublicInputs, JoinSplitProofData};
use crate::error::ElusivError::{InvalidAccount, InvalidMerkleRoot, InvalidPublicInputs, InvalidAmount, NullifierAlreadyExists};
use crate::macros::guard;
use crate::state::{NullifierAccount, StorageAccount};

/// Sends lamports from the sender Sender to the recipient
pub fn send_with_system_program<'a>(
    sender: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    lamports: u64,
) -> ProgramResult {
    // Check that system_program is correct
    guard!(*system_program.key == system_program::ID, InvalidAccount);

    // Transfer funds from sender
    let instruction = solana_program::system_instruction::transfer(
        &sender.key,
        recipient.key,
        lamports 
    );
    let (_, bump_seed) = Pubkey::find_program_address(&[b"elusiv"], &super::super::id());
    solana_program::program::invoke_signed(
        &instruction,
        &[
            sender.clone(),
            recipient.clone(),
            system_program.clone(),
        ],
        &[&[&b"elusiv"[..], &[bump_seed]]],
    )    
}

/// Verifies public inputs and the proof data for proof requests
pub fn check_join_split_public_inputs<const N: usize>(
    public_inputs: &JoinSplitPublicInputs<N>,
    proof_data: &JoinSplitProofData<N>,
    storage_account: &StorageAccount,
    nullifier_accounts: [&NullifierAccount; N],
    //commitment_queue_account: &CommitmentQueueAccount,
) -> ProgramResult {
    assert!(N <= 2);

    let uses_multiple_trees = N > 1 && proof_data.tree_indices[0] != proof_data.tree_indices[1];
    let active_tree_index = storage_account.get_trees_count();

    // Check that roots are the same if they represent the same tree
    guard!(!uses_multiple_trees || public_inputs.roots[0] == public_inputs.roots[1], InvalidMerkleRoot);

    // Check that roots are valid
    for i in 0..N {
        // For the active tree: root can either be the last root or any root from the active_mt_root_history
        if proof_data.tree_indices[i] == active_tree_index {
            guard!(storage_account.is_root_valid(public_inputs.roots[i]), InvalidMerkleRoot);
        } else { // For a non-active tree: root can only be one value
            guard!(public_inputs.roots[i] == nullifier_accounts[i].get_root(), InvalidMerkleRoot);
        }
    }

    // Check that nullifier_hashes for the same tree are different
    guard!(!uses_multiple_trees || public_inputs.nullifier_hashes[0] == public_inputs.nullifier_hashes[1], InvalidPublicInputs);

    // Check that nullifier_hashes can be inserted
    for i in 0..N {
        guard!(nullifier_accounts[i].can_insert_nullifier_hash(public_inputs.nullifier_hashes[i]), NullifierAlreadyExists);
    }

    Ok(())
}

/// Sends from a program owned pool
pub fn send_from_pool<'a>(
    pool: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>,
    amount: u64,
) -> ProgramResult {
    **pool.try_borrow_mut_lamports()? = pool.lamports().checked_sub(amount)
        .ok_or(ProgramError::from(InvalidAmount))?;

    **recipient.try_borrow_mut_lamports()? = recipient.lamports().checked_add(amount)
        .ok_or(ProgramError::from(InvalidAmount))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_public_inputs_commitment_duplicate() {
        panic!()
    }

    #[test]
    fn test_public_inputs_different_roots_same_tree() {
        panic!()
    }

    #[test]
    fn test_public_inputs_different_nullifiers() {
        panic!()
    }

    #[test]
    fn test_public_inputs_valid() {
        panic!()
    }
}