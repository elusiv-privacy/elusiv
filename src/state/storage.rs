use crate::macros::{ elusiv_account, two_pow };
use solana_program::entrypoint::ProgramResult;
use crate::types::U256;
use crate::bytes::*;
use crate::error::ElusivError::{ CommitmentAlreadyExists, NoRoomForCommitment };
use crate::macros::guard;
use super::program_account::*;

pub const MT_HEIGHT: usize = 20;
pub const MT_SIZE: usize = two_pow!(MT_HEIGHT + 1);
pub const MT_COMMITMENT_START: usize = two_pow!(MT_HEIGHT) - 1;
const HISTORY_ARRAY_COUNT: usize = 10;

#[elusiv_account(pda_seed = b"storage", big_array = [U256; MT_SIZE])]
struct StorageAccount {
    /// Points to the next commitment in the active MT
    next_commitment_ptr: u64,
    /// Stores the last HISTORY_ARRAY_COUNT roots of the active tree
    active_mt_root_history: [U256; HISTORY_ARRAY_COUNT],
    /// The amount of already finished MTs
    trees_count: u64,
    // The amount of archived MTs
    archived_count: u64,
}

impl<'a> StorageAccount<'a> {
    pub fn reset(&mut self) {
        self.set_next_commitment_ptr(0);

        for i in 0..self.active_mt_root_history.len() {
            self.active_mt_root_history[i] = 0;
        }
    }

    pub fn can_insert_commitment(&self, commitment: U256) -> ProgramResult {
        guard!(self.get_next_commitment_ptr() < two_pow!(MT_HEIGHT), NoRoomForCommitment);
        guard!(
            not_contains(commitment, self.get_mut_array_slice(MT_COMMITMENT_START, MT_SIZE - 1)),
            CommitmentAlreadyExists
        );

        Ok(())
    }

    /// Inserts commitment and the above hashes
    pub fn insert_commitment(&mut self, values: [U256; MT_HEIGHT + 1]) {
        let ptr = self.get_next_commitment_ptr() as usize;
        self.set_next_commitment_ptr(ptr as u64 + 1);

        // Save last root
        self.set_active_mt_root_history(ptr % HISTORY_ARRAY_COUNT, self.get_root());

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

    pub fn is_root_valid(&self, root: U256) -> bool {
        root == self.get_root() || contains(root, self.active_mt_root_history)
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
    two_pow!(layer) - 1 + index
}