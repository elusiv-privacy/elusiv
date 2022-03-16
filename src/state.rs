use super::storage_account::*;

use super::merkle::*;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use super::error::ElusivError::{
    InvalidStorageAccount,
    InvalidStorageAccountSize,
    NullifierAlreadyUsed,
    NoRoomForNullifier,
    CommitmentAlreadyUsed,
    NoRoomForCommitment,
};
use super::fields::scalar::*;

pub use super::poseidon::DepositHashingAccount;
pub use super::groth16::ProofVerificationAccount;

pub const TREE_HEIGHT: usize = 12;
pub const TREE_SIZE: usize = ((2 as usize).pow(TREE_HEIGHT as u32 + 1) - 1) * 32;

pub const TREE_LEAF_START: usize = (2 as usize).pow(TREE_HEIGHT as u32) - 1;
pub const TREE_LEAF_COUNT: usize = (2 as usize).pow(TREE_HEIGHT as u32);

const NULLIFIERS_COUNT: usize = 1 << (TREE_HEIGHT);
const NULLIFIERS_SIZE: usize = NULLIFIERS_COUNT * 32;

const HISTORY_ARRAY_COUNT: usize = 10;
const HISTORY_ARRAY_SIZE: usize = HISTORY_ARRAY_COUNT * 32;

solana_program::declare_id!("B9Z5eCSFKhWkLiYmikqxUTKoJPZpi1Mk7gdHdYxt2Vaa");

pub struct ProgramAccount<'a> {
    /// Merkle tree or arity 2
    /// - 2^{TREE_HEIGHT + 1} - 1 32 byte words
    pub merkle_tree: &'a mut [u8],

    /// Nullifier hashes
    /// - TREE_HEIGHT 32 byte words
    nullifier_hashes: &'a mut [u8],

    /// Root history
    /// - HISTORY_ARRAY_COUNT 32 byte words
    pub root_history: &'a mut [u8],

    /// Next Leaf Pointer
    /// - (u32 represented as 4 bytes)
    /// - multiply by 32 to point at the exact leaf
    /// - range: [0; 2^{TREE_HEIGHT}]
    next_leaf_pointer: &'a mut [u8],

    /// Next Nullifier Pointer
    /// - (u32 represented as 4 bytes)
    /// - multiply by 32 to point at the exact nullifier
    /// - range: [0; NULLIFIERS_COUNT]
    next_nullifier_pointer: &'a mut [u8],
}

impl<'a> ProgramAccount<'a> {
    pub const TOTAL_SIZE: usize = TREE_SIZE + NULLIFIERS_SIZE + HISTORY_ARRAY_SIZE + 4 + 4;

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

        let (merkle_tree, data) = data.split_at_mut(TREE_SIZE);
        let (nullifier_hashes, data) = data.split_at_mut(NULLIFIERS_SIZE);
        let (root_history, data) = data.split_at_mut(HISTORY_ARRAY_SIZE);
        let (next_leaf_pointer, data) = data.split_at_mut(4);
        let (next_nullifier_pointer, _) = data.split_at_mut(4);

        Ok(
            ProgramAccount::init(
                merkle_tree,
                nullifier_hashes,
                root_history,
                next_leaf_pointer,
                next_nullifier_pointer,
            )
        )
    }
}

impl<'a> ProgramAccount<'a> {
    fn init(merkle_tree: &'a mut [u8], nullifier_hashes:&'a mut [u8], root_history:&'a mut [u8], next_leaf_pointer:&'a mut [u8], next_nullifier_pointer: &'a mut [u8]) -> Self {
        ProgramAccount {
            merkle_tree,
            nullifier_hashes,
            root_history,
            next_leaf_pointer,
            next_nullifier_pointer,
        }
    }

    /// Checks whether the nullifier hash can be inserted
    /// - is there still place for a new nullifier?
    /// - does it not already exist?
    /// 
    /// ### Arguments
    /// 
    /// * `nullifier_hash` - nullifier hash as 4 u64 limbs
    pub fn can_insert_nullifier_hash(&self, nullifier_hash: ScalarLimbs) -> ProgramResult {
        if bytes_to_u32(&self.next_nullifier_pointer) >= NULLIFIERS_COUNT as u32 { return Err(NullifierAlreadyUsed.into()); }

        // Note: we could make this more efficient by only running this till the last inserted hash, but we want to keep the
        // performance identical, so wen let it run over the full array.
        if contains_limbs(nullifier_hash, &self.nullifier_hashes) { return Err(NoRoomForNullifier.into()); }

        Ok(())
    }

    /// Inserts a nullifier hash if it does not already exist
    /// 
    /// ### Arguments
    /// 
    /// * `nullifier_hash` - nullifier hash as 4 u64 limbs
    pub fn insert_nullifier_hash(&mut self, nullifier_hash: ScalarLimbs) -> ProgramResult {
        // Additional security check
        self.can_insert_nullifier_hash(nullifier_hash)?;

        // Insert
        let mut pointer = bytes_to_u32(&self.next_nullifier_pointer) as usize;
        let bytes = limbs_to_bytes(&nullifier_hash);
        set(self.nullifier_hashes, pointer, 4, &bytes)?;

        // Increment pointer
        pointer += 1;
        set(&mut self.next_nullifier_pointer, 0, 4, &pointer.to_le_bytes())?;

        Ok(())
    }

    pub fn can_insert_commitment(&self, commitment: ScalarLimbs) -> ProgramResult {
        // Checks if there is room
        let pointer = self.leaf_pointer() as usize;
        if pointer >= TREE_LEAF_COUNT { return Err(NoRoomForCommitment.into()); }

        // Checks if the commitment already exists
        let commitment_slice = &self.merkle_tree[TREE_LEAF_START..(TREE_LEAF_START + pointer)];
        if contains_limbs(commitment, &commitment_slice) { return Err(CommitmentAlreadyUsed.into()); }

        Ok(())
    }

    /// Inserts the commitment into the tree
    pub fn add_commitment(&mut self, values: [[u8; 32]; TREE_HEIGHT + 1]) -> ProgramResult {
        let pointer = self.leaf_pointer() as usize;

        // Additional commitment security check
        let commitment = values[0];
        self.can_insert_commitment(bytes_to_limbs(&commitment))?;

        // Save last root
        let root = &self.merkle_tree[..32];
        set(self.root_history, (pointer % HISTORY_ARRAY_COUNT) * 32, 32, root)?;

        // Insert values into the tree
        insert_hashes(&mut self.merkle_tree, values, pointer);

        // Increment pointer
        self.increment_leaf_pointer(1)?;

        Ok(())
    }

    pub fn increment_leaf_pointer(&mut self, count: usize) -> ProgramResult  {
        let mut pointer = self.leaf_pointer() as usize;
        pointer += count;
        set(self.next_leaf_pointer, 0, 4, &pointer.to_le_bytes())?;
        Ok(())
    }

    /// Check whether the provided root is valid
    /// 
    /// ### Arguments
    /// 
    /// * `root` - merkle root provided as 4 u64 limbs
    pub fn is_root_valid(&self, root: ScalarLimbs) -> bool {
        // Checks for root equality with tree root
        if contains_limbs(root, &self.merkle_tree[..32]) { return true; }

        // Checks for root in root history
        contains_limbs(root, self.root_history)
    }

    pub fn leaf_pointer(&self) -> u32 {
        bytes_to_u32(&self.next_leaf_pointer)
    }

    pub fn nullifier_pointer(&self) -> u32 {
        bytes_to_u32(&self.next_nullifier_pointer)
    }
}

#[cfg(test)]
mod tests {
    type StorageAccount<'a> = super::ProgramAccount<'a>;

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