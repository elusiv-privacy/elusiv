use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    system_program,
    program_error::ProgramError,
};
use crate::types::{JoinSplitPublicInputs, JoinSplitProofData};
use crate::error::ElusivError::{InvalidAccount, InvalidMerkleRoot, InvalidPublicInputs, InvalidAmount};
use crate::macros::guard;
use crate::state::{NullifierAccount, StorageAccount};
//use crate::state::queue::{CommitmentQueueAccount, RingQueue};

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
    let uses_multiple_trees = N > 1 && proof_data.tree_indices[0] != proof_data.tree_indices[1];

    // Check that roots are the same if they represent the same tree
    guard!(!uses_multiple_trees || public_inputs.roots[0] == public_inputs.roots[1], InvalidMerkleRoot);

    // Check that roots are valid by matching the nullifier account
    //storage_account.is_root_valid(nullifier_accounts[0], proof_data.roots[0])?;
    //storage_account.is_root_valid(nullifier_accounts[1], proof_data.roots[1])?;

    // Check that nullifier hashes are different for the same tree
    /*if N > 1 {
        guard!(
            !uses_multiple_trees || proof_data.nullifiers[0] != proof_data.nullifiers[1],
            InvalidPublicInputs
        );
    }

    // Check that commitment is new and is not in queue
    // TODO: reevaluate if it makes sense to verify the commitment uniqueness here (seems uneccesary)
    storage_account.can_insert_commitment(public_inputs.commitment)?;
    //commitment_queue_account.contains(public_inputs.commitment);

    // Check if nullifiers are new
    nullifier_accounts[0].can_insert_nullifier(proof_data.nullifiers[0])?;
    nullifier_accounts[1].can_insert_nullifier(proof_data.nullifiers[1])?;*/

    // Check that roots are correct
    panic!();

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