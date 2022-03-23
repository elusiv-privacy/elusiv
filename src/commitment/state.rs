use elusiv_account::*;
use super::super::types::U256;
use super::super::state::TREE_HEIGHT;

solana_program::declare_id!("CJ4PyZKqLUCf4XMZbsbX9WMCuFLNR37PejKtLjVbxXHN");
#[derive(ElusivAccount)]
#[remove_original_implementation]
struct CommitmentAccount {
    leaf_index: u64,

    opening: [U256; TREE_HEIGHT],
    hashing_state_storage: [U256; 3],
    finished_hashes_storage: [U256; TREE_HEIGHT + 1],

    current_hash_iteration: u64,
    current_hash_tree_position: u64,
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