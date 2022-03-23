use elusiv_account::*;
use super::types::U256;

pub const TREE_HEIGHT: usize = 16;
pub const TREE_SIZE: usize = 1 << (TREE_HEIGHT + 1);
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