use crate::macros::elusiv_account;
use crate::types::U256;
use crate::bytes::*;
use super::program_account::*;
use borsh::{BorshDeserialize, BorshSerialize};

/// Height of the active Merkle Tree
pub const MT_HEIGHT: usize = 20;

/// Count of all nodes in the merkle-tree
pub const MT_SIZE: usize = 2usize.pow(MT_HEIGHT as u32 + 1);

/// Index of the first commitment in the Merkle Tree
pub const MT_COMMITMENT_START: usize = 2usize.pow(MT_HEIGHT as u32) - 1;

/// Since before submitting a proof request the current root can change, we store the previous ones
const HISTORY_ARRAY_COUNT: usize = 100;

pub const STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT: usize = big_array_accounts_count(MT_SIZE, U256::SIZE);

// The `StorageAccount` contains the active Merkle Tree that stores new commitments
// - the MT is stored as an array with the first element being the root and the second and third elements the layer below the root
// - in order to manage a growing number of commitments, once the MT is full it get's reset (and the root is stored elsewhere)
#[elusiv_account(pda_seed = b"storage", multi_account = (
    STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT;
    max_account_size(U256::SIZE)
))]
pub struct StorageAccount {
    bump_seed: u8,
    initialized: bool,
    pubkeys: [U256; STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT],

    // Points to the next commitment in the active MT
    next_commitment_ptr: u64,

    // The amount of already finished MTs
    trees_count: u64,

    // The amount of archived MTs
    archived_count: u64,

    // Stores the last HISTORY_ARRAY_COUNT roots of the active tree
    active_mt_root_history: [U256; HISTORY_ARRAY_COUNT],
}

impl<'a, 'b, 't> BigArrayAccount<'t> for StorageAccount<'a, 'b, 't> {
    type T = U256;
}

impl<'a, 'b, 't> StorageAccount<'a, 'b, 't> {
    pub fn reset(&mut self) {
        self.set_next_commitment_ptr(&0);

        for i in 0..self.active_mt_root_history.len() {
            self.active_mt_root_history[i] = 0;
        }
    }

    /// Inserts commitment and the above hashes
    pub fn insert_commitment(&mut self, values: [U256; MT_HEIGHT + 1]) {
        let ptr = self.get_next_commitment_ptr() as usize;
        self.set_next_commitment_ptr(&(ptr as u64 + 1));

        // Save last root
        self.set_active_mt_root_history(ptr % HISTORY_ARRAY_COUNT, &self.get_root());

        // Insert values into the tree
        for (i, &value) in values.iter().enumerate() {
            let layer = MT_HEIGHT - i;
            let index = ptr >> i;
            self.set(mt_array_index(layer, index), value);
        }
    }

    pub fn get_root(&self) -> U256 {
        self.get(0)
    }

    /// A root is valid if it's the current root or inside of the active_mt_root_history array
    pub fn is_root_valid(&self, root: U256) -> bool {
        //root == self.get_root() || contains(root, self.active_mt_root_history)
        panic!()
    }

    pub fn get_mt_opening(&self, index: usize) -> [U256; MT_HEIGHT] {
        let mut opening = [[0; 32]; MT_HEIGHT];
        let mut index = index;

        for i in 0..MT_HEIGHT {
            let layer = MT_HEIGHT - i;
            let n_index = if index % 2 == 0 { index + 1 } else { index - 1};
            opening[i] = self.get_mt_node(layer, n_index);
            index = index >> 1;
        }

        opening
    }

    pub fn get_mt_node(&self, layer: usize, index: usize) -> U256 {
        self.get(mt_array_index(layer, index))
    }
}

pub fn mt_array_index(layer: usize, index: usize) -> usize {
    2usize.pow(layer as u32) - 1 + index
}