use elusiv_account::*;
use solana_program::entrypoint::ProgramResult;
use super::types::U256;
use super::bytes::contains;
use super::error::ElusivError;

pub const TREE_HEIGHT: usize = 16;
pub const TREE_SIZE: usize = 1 << (TREE_HEIGHT + 1);
pub const TREE_LEAF_START: usize = (1 << TREE_HEIGHT) - 1;
pub const TREE_LEAF_COUNT: usize = 1 << TREE_HEIGHT;
const NULLIFIERS_COUNT: usize = 1 << (TREE_HEIGHT);
const HISTORY_ARRAY_COUNT: usize = 10;

solana_program::declare_id!("CYFkyPAmHjayCwhRS6LpQjY2E7atNeLS3b8FE1HTYQY4");

#[derive(ElusivAccount)]
#[remove_original_implementation]
struct StorageAccount {
    next_leaf: u64,
    next_nullifier: u64,

    merkle_tree: [U256; TREE_SIZE],
    nullifier_hashes: [U256; NULLIFIERS_COUNT],
    root_history: [U256; HISTORY_ARRAY_COUNT],
}

impl<'a> StorageAccount<'a> {
    pub fn can_insert_nullifier_hash(&self, nullifier_hash: U256) -> ProgramResult {
        if self.get_next_nullifier() >= NULLIFIERS_COUNT as u64 {
            return Err(ElusivError::NullifierAlreadyUsed.into());
        }

        if contains(nullifier_hash, &self.nullifier_hashes) {
            return Err(ElusivError::NoRoomForNullifier.into());
        }

        Ok(())
    }

    pub fn can_insert_commitment(&self, commitment: U256) -> ProgramResult {
        if self.get_next_leaf() >= TREE_LEAF_COUNT as u64 {
            return Err(ElusivError::NoRoomForCommitment.into());
        }

        let tree_leaves = &self.merkle_tree[TREE_LEAF_START..(TREE_LEAF_START + self.get_next_leaf() as usize)];
        if contains(commitment, tree_leaves) {
            return Err(ElusivError::CommitmentAlreadyUsed.into());
        }

        Ok(())
    }

    pub fn is_root_valid(&self, root: U256) -> ProgramResult {
        // Checks for root equality with tree root
        if contains(root, &self.merkle_tree[..32]) {
            return Ok(());
        }

        // Checks for root in root history
        if contains(root, self.root_history) {
            return Ok(());
        }

        Err(ElusivError::InvalidMerkleRoot.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correct_size() {
        let mut data = vec![0; StorageAccount::TOTAL_SIZE];
        StorageAccount::from_data(&mut data).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_invalid_size() {
        let mut data = vec![0; StorageAccount::TOTAL_SIZE - 1];
        StorageAccount::from_data(&mut data).unwrap();
    }
}