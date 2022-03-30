use crate::macros::{ ElusivAccount, remove_original_implementation };
use solana_program::entrypoint::ProgramResult;
use crate::error::ElusivError;
use crate::types::U256;
use crate::state::{TREE_HEIGHT, StorageAccount};

#[derive(ElusivAccount)]
#[remove_original_implementation]
struct CommitmentAccount {
    // If `false` account can be reset
    is_active: bool,

    leaf_index: u64,

    opening: [U256; TREE_HEIGHT],
    hashing_state: [U256; 3],
    finished_hashes: [U256; TREE_HEIGHT + 1],

    current_hash_iteration: u64,
    current_hash_tree_position: u64,
}

impl<'a> CommitmentAccount<'a> {
    crate::macros::pubkey!("CYFkyPAmHjayCwhRS6LpQjY2E7atNeLS3b8FE1HTYQY4");

    pub fn reset(
        &mut self,
        storage_account: &StorageAccount,
        commitment: U256,
    ) -> ProgramResult {
        // Check if account can be reset
        if self.get_is_active() {
            return Err(ElusivError::ProofAccountCannotBeReset.into());
        }
        self.set_is_active(true);

        // Reset counters
        self.set_current_hash_iteration(super::ITERATIONS as u64);
        self.set_current_hash_tree_position(0);

        // Store hashing partners (aka opening)
        let leaf_index = storage_account.get_next_commitment();
        self.set_leaf_index(leaf_index);
        let opening = storage_account.get_tree_opening(leaf_index as usize)?;
        for (i, partner) in opening.iter().enumerate() {
            self.set_opening(i, partner);
        }

        // Add commitment to hashing state and finished hash store
        self.set_finished_hashes(0, &commitment);

        // Reset hashing state
        self.set_hashing_state(0, &commitment);
        self.set_hashing_state(1, &[0; 32]);
        self.set_hashing_state(2, &[0; 32]);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correct_size() {
        let mut data = [0; CommitmentAccount::TOTAL_SIZE];
        CommitmentAccount::from_data(&mut data).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_invalid_size() {
        let mut data = [0; CommitmentAccount::TOTAL_SIZE - 1];
        CommitmentAccount::from_data(&mut data).unwrap();
    }
}