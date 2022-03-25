use elusiv_account::*;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
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

// Nullifier hashes and commitment
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

    pub fn insert_nullifier_hash(&mut self, nullifier_hash: U256) -> ProgramResult {
        let index = self.get_next_nullifier();
        self.set_nullifier_hashes(index as usize, &nullifier_hash);

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

    /// Inserts commitment and the above hashes
    pub fn insert_commitment(&mut self, values: [U256; TREE_HEIGHT + 1]) -> ProgramResult {
        let leaf_index = self.get_next_leaf() as usize;

        // Additional commitment security check
        let commitment = values[0];
        self.can_insert_commitment(commitment)?;

        // Save last root
        let root = self.get_tree_node(0, 0)?;
        self.set_root_history(leaf_index % HISTORY_ARRAY_COUNT, &root);

        // Insert values into the tree
        for (i, &value) in values.iter().enumerate() {
            let layer = TREE_HEIGHT - i;
            let index = leaf_index >> i;
            self.set_tree_node(layer, index, value)?;
        }

        // Increment pointer
        self.set_next_leaf(leaf_index as u64 + 1);

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

macro_rules! assert_valid_tree_access {
    ($layer: expr, $index: expr) => {
        if $layer > TREE_HEIGHT || $index > size_of_tree_layer($layer) {
            return Err(ElusivError::InvalidMerkleTreeAccess.into());
        }
    };
}

// Merkle tree
// - `layer` 0 is the root
impl<'a> StorageAccount<'a> {
    #[allow(unused_comparisons)]
    pub fn get_tree_opening(&self, index: usize) -> Result<[U256; TREE_HEIGHT], ProgramError> {
        assert_valid_tree_access!(0, index);

        let mut opening = [[0; 32]; TREE_HEIGHT];
        let mut index = index;

        for i in 0..TREE_HEIGHT {
            let layer = TREE_HEIGHT - i;
            let n_index = if index % 2 == 0 { index + 1 } else { index - 1};
            opening[i] = self.get_tree_node(layer, n_index)?;
            index = index >> 1;
        }

        Ok(opening)
    }

    pub fn get_tree_node(&self, layer: usize, index: usize) -> Result<U256, ProgramError> {
        assert_valid_tree_access!(layer, index);

        Ok(self.get_merkle_tree(tree_array_index(layer, index)))
    }

    pub fn set_tree_node(&mut self, layer: usize, index: usize, value: U256) -> Result<(), ProgramError> {
        assert_valid_tree_access!(layer, index);

        self.set_merkle_tree(tree_array_index(layer, index), &value);

        Ok(())
    }
}

pub fn tree_array_index(layer: usize, index: usize) -> usize {
    (1 << layer) - 1 + index
}

fn size_of_tree_layer(layer: usize) -> usize {
    1 << layer
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