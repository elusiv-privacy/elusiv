use super::super::storage_account::*;

use solana_program::program_error::ProgramError;
use solana_program::entrypoint::ProgramResult;
use super::super::error::ElusivError::{ InvalidStorageAccount, InvalidStorageAccountSize };
use super::super::fields::scalar::*;
use super::super::state::TREE_HEIGHT;

solana_program::declare_id!("CJ4PyZKqLUCf4XMZbsbX9WMCuFLNR37PejKtLjVbxXHN");

pub struct DepositHashingAccount<'a> {
    /// Next leaf index
    /// - 4 bytes
    leaf_index: &'a mut [u8],

    /// Hashing neighbours
    /// - TREE_HEIGHT 32 byte elements
    opening: &'a mut [u8],

    /// Hash working storage of current deposit
    /// - (element-size: 32 bytes)
    /// - containts 3 elements
    hashing_state_storage: &'a mut [u8],

    /// Finished tree nodes of current deposit
    /// - (element-size: 32 bytes)
    /// - containts TREE_HEIGHT + 1 elements (every layer of the tree)
    finished_hashes_storage: &'a mut [u8],

    /// Amount of current deposit
    /// - (u64 represented as 8 bytes)
    committed_amount: &'a mut [u8],

    /// Hash iteraction of current deposit
    /// - (u16 represented as 2 bytes)
    current_hash_iteration: &'a mut [u8],

    /// Hashing process tree position of current deposit
    /// - (u16 represented as 2 bytes)
    current_hash_tree_position: &'a mut [u8],
}

impl<'a> DepositHashingAccount<'a> {
    pub const TOTAL_SIZE: usize = 4 + (TREE_HEIGHT + 3 + TREE_HEIGHT + 1) * 32 + 8 + 2 + 2;

    pub fn new(
        account_info: &solana_program::account_info::AccountInfo,
        data: &'a mut [u8],
        program_id: &solana_program::pubkey::Pubkey,
    ) -> Result<Self, ProgramError> {
        if account_info.owner != program_id { return Err(InvalidStorageAccount.into()); }
        if !account_info.is_writable { return Err(InvalidStorageAccount.into()); }
        //if *account_info.key != id() { return Err(InvalidStorageAccount.into()); }
        
        Self::from_data(data)
    }

    pub fn from_data(data: &'a mut [u8]) -> Result<Self, ProgramError> {
        if data.len() != Self::TOTAL_SIZE { return Err(InvalidStorageAccountSize.into()); }

        let (leaf_index, data) = data.split_at_mut(4);
        let (opening, data) = data.split_at_mut(TREE_HEIGHT * 32);
        let (hashing_state_storage, data) = data.split_at_mut(3 * 32);
        let (finished_hashes_storage, data) = data.split_at_mut((TREE_HEIGHT + 1) * 32);
        let (committed_amount, data) = data.split_at_mut(8);
        let (current_hash_iteration, data) = data.split_at_mut(2);
        let (current_hash_tree_position, _) = data.split_at_mut(2);

        Ok(
            DepositHashingAccount {
                leaf_index,
                opening,
                hashing_state_storage,
                finished_hashes_storage,
                committed_amount,
                current_hash_iteration,
                current_hash_tree_position,
            }
        )
    }
}

impl<'a> DepositHashingAccount<'a> {
    pub fn get_leaf_index(&self) -> usize {
        bytes_to_u32(&self.leaf_index) as usize
    }
    pub fn set_leaf_index(&mut self, index: u32) {
        let bytes = u32::to_le_bytes(index);
        self.leaf_index[0] = bytes[0];
        self.leaf_index[1] = bytes[1];
        self.leaf_index[2] = bytes[2];
        self.leaf_index[3] = bytes[3];
    }

    pub fn get_neighbour(&self, index: usize) -> Scalar {
        from_bytes_le_mont(&self.opening[index * 32..index * 32 + 32])
    }
    pub fn set_opening(&mut self, opening: &[u8]) -> ProgramResult {
        set(&mut self.opening, 0, TREE_HEIGHT * 32, opening)
    }

    pub fn get_hashing_state(&self) -> [Scalar; 3] {
        [
            from_bytes_le_mont(&self.hashing_state_storage[..32]),
            from_bytes_le_mont(&self.hashing_state_storage[32..64]),
            from_bytes_le_mont(&self.hashing_state_storage[64..]),
        ]
    }
    pub fn set_hashing_state(&mut self, state: [Scalar; 3]) {
        let mut bytes: Vec<u8> = to_bytes_le_mont(state[0]);
        bytes.append(&mut to_bytes_le_mont(state[1]));
        bytes.append(&mut to_bytes_le_mont(state[2]));

        for (i, &byte) in bytes.iter().enumerate() {
            self.hashing_state_storage[i] = byte;
        }
    }

    pub fn get_finished_hashes_storage(&self) -> [[u8; 32]; TREE_HEIGHT + 1] {
        let mut a = [[0; 32]; TREE_HEIGHT + 1];
        for i in 0..a.len() {
            let slice = &self.finished_hashes_storage[i * 32..(i + 1) * 32];
            for (j, &byte) in slice.iter().enumerate() {
                a[i][j] = byte;
            }
        }
        a
    }
    pub fn set_finished_hash(&mut self, position: usize, value: Scalar) {
        for (i, &byte) in to_bytes_le_mont(value).iter().enumerate() {
            self.finished_hashes_storage[position * 32 + i] = byte;
        }
    }

    pub fn get_committed_amount(&self) -> u64 { bytes_to_u64(self.committed_amount) }
    pub fn set_committed_amount(&mut self, amount: u64) {
        let bytes = amount.to_le_bytes();
        self.committed_amount[0] = bytes[0];
        self.committed_amount[1] = bytes[1];
        self.committed_amount[2] = bytes[2];
        self.committed_amount[3] = bytes[3];
        self.committed_amount[4] = bytes[4];
        self.committed_amount[5] = bytes[5];
        self.committed_amount[6] = bytes[6];
        self.committed_amount[7] = bytes[7];
    }

    pub fn get_current_hash_iteration(&self) -> u16 { bytes_to_u16(self.current_hash_iteration) }
    pub fn set_current_hash_iteration(&mut self, round: u16) {
        let bytes = round.to_le_bytes();
        self.current_hash_iteration[0] = bytes[0];
        self.current_hash_iteration[1] = bytes[1];
    }

    pub fn get_current_hash_tree_position(&self) -> u16 { bytes_to_u16(self.current_hash_tree_position) }
    pub fn set_current_hash_tree_position(&mut self, position: u16) {
        let bytes = position.to_le_bytes();
        self.current_hash_tree_position[0] = bytes[0];
        self.current_hash_tree_position[1] = bytes[1];
    }
}

#[cfg(test)]
mod tests {
    type StorageAccount<'a> = super::DepositHashingAccount<'a>;

    #[test]
    fn test_correct_size() {
        let mut data = [0; StorageAccount::TOTAL_SIZE];
        StorageAccount::from_data(&mut data).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_invalid_size() {
        let mut data = [0; StorageAccount::TOTAL_SIZE - 1];
        StorageAccount::from_data(&mut data).unwrap();
    }
}