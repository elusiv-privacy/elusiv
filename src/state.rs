use limbed_merkle::tree::LimbedMerkleTree;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use super::error::ElusivError::{
    InvalidStorageAccountSize,
    CouldNotCreateMerkleTree,
    NullifierAlreadyUsed,
    NoRoomForNullifier,
    CommitmentAlreadyUsed,
    NoRoomForCommitment,
};
use poseidon::scalar;
use poseidon::poseidon::Poseidon;
use poseidon::scalar::Scalar;
use poseidon::scalar::ScalarLimbs;

const TREE_HEIGHT: usize = 12;
const TREE_SIZE: usize = ((2 as usize).pow(TREE_HEIGHT as u32 + 1) - 1) * 32; //((1 << (TREE_HEIGHT + 1)) - 1) * 32;
const TREE_LEAF_START: usize = (2 as usize).pow(TREE_HEIGHT as u32) - 1;
const TREE_LEAF_COUNT: usize = (2 as usize).pow(TREE_HEIGHT as u32);

const NULLIFIERS_COUNT: usize = 1 << (TREE_HEIGHT);
const NULLIFIERS_SIZE: usize = NULLIFIERS_COUNT * 32;

const HISTORY_ARRAY_COUNT: usize = 10;
const HISTORY_ARRAY_SIZE: usize = HISTORY_ARRAY_COUNT ;

pub const TOTAL_SIZE: usize = TREE_SIZE + NULLIFIERS_SIZE + HISTORY_ARRAY_SIZE + 4 + 4;

pub struct StorageAccount<'a> {
    /// Merkle tree or arity 2
    /// - (elements: 32 u8 limbs)
    merkle_tree: &'a mut [u8],

    /// Nullifier hashes
    /// - (elements: 32 u8 limbs)
    nullifier_hashes: &'a mut [u8],

    /// Root history
    /// - (elements: 32 u8 limbs)
    root_history: &'a mut [u8],

    /// Next Leaf Pointer
    /// - (u32 represented as 4 bytes)
    /// - multiply by 32 to point at the exact leaf
    /// - range: [0; 2^{TREE_HEIGHT})
    next_leaf_pointer: &'a mut [u8],

    /// Next Nullifier Pointer
    /// - (u32 represented as 4 bytes)
    /// - multiply by 32 to point at the exact nullifier
    /// - range: [0; NULLIFIERS_COUNT)
    next_nullifier_pointer: &'a mut [u8],
}

impl<'a> StorageAccount<'a> {
    pub fn from(buffer: &'a mut [u8]) -> Result<Self, ProgramError> {
        if buffer.len() != TOTAL_SIZE { return Err(InvalidStorageAccountSize.into()); }

        let (merkle_tree, rest) = buffer.split_at_mut(TREE_SIZE);
        let (nullifier_hashes, rest) = rest.split_at_mut(NULLIFIERS_SIZE);
        let (root_history, rest) = rest.split_at_mut(HISTORY_ARRAY_SIZE);
        let (next_leaf_pointer, rest) = rest.split_at_mut(4);
        let (next_nullifier_pointer, _) = rest.split_at_mut(4);

        Ok(StorageAccount {
            merkle_tree,
            nullifier_hashes,
            root_history,
            next_leaf_pointer,
            next_nullifier_pointer
        })
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
        let bytes = scalar::limbs_to_bytes(&nullifier_hash);
        Self::set(self.nullifier_hashes, pointer, 4, &bytes)?;

        // Increment pointer
        pointer += 1;
        Self::set(&mut self.next_nullifier_pointer, 0, 4, &pointer.to_le_bytes())?;

        Ok(())
    }

    /// Inserts the commitment into the tree if there is room for it
    /// 
    /// ### Arguments
    /// 
    /// * `commitment` - commitment hash as 4 u64 limbs
    pub fn try_add_commitment(&mut self, commitment: ScalarLimbs) -> ProgramResult {
        // Checks if there is room
        let mut pointer = bytes_to_u32(&self.next_leaf_pointer) as usize;
        if pointer >= TREE_LEAF_COUNT { return Err(NoRoomForCommitment.into()); }

        // Checks if the commitment already exists
        let commitment_slice = &self.merkle_tree[TREE_LEAF_START..(TREE_LEAF_START + TREE_LEAF_COUNT)];
        if contains_limbs(commitment, &commitment_slice) { return Err(CommitmentAlreadyUsed.into()); }

        // Insert commitment into merkle tree
        let commitment = scalar::from_limbs(&commitment);
        let limbs_to_value = |limbs: &[u8]| { scalar::from_bytes_le(limbs) };
        let value_to_limbs = |value: Scalar| { scalar::to_bytes_le(value) };
        let poseidon = Poseidon::new();
        let hash = |left: Scalar, right: Scalar| { poseidon.hash(vec![left, right]).unwrap() };
        let mut tree = LimbedMerkleTree::new(
            TREE_HEIGHT,    // height
            &mut self.merkle_tree,   // store
            5,   //limbing_power
            hash, // hash
            limbs_to_value, // limbs_to_value
            value_to_limbs  // value_to_limbs
        ).or(Err(ProgramError::from(CouldNotCreateMerkleTree)))?;
        tree.set_leaf(pointer, commitment);

        // Increment pointer
        pointer += 1;
        Self::set(self.next_leaf_pointer, 0, 4, &pointer.to_le_bytes())?;

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

    /// Overrides the bytes in the slice
    /// 
    /// ### Arguments
    /// 
    /// * `slice` - buffer to write in
    /// * `from` - start index to write from
    /// * `bytecount` - amount of bytes to write
    /// * `bytes` - values to set
    fn set(slice: &mut [u8], from: usize, bytecount: usize, bytes: &[u8]) -> ProgramResult {
        for i in 0..bytecount {
            slice[from + i] = bytes[i];
        }

        Ok(())
    }
}

/// Checks whether a word represented by 4 u64 limbs is contained inside a byte array
/// 
/// ### Arguments
/// 
/// * `limbs` - 4 u64 limbs
/// * `buffer` - bytes to search in
fn contains_limbs(limbs: ScalarLimbs, buffer: &[u8]) -> bool {
    let bytes: [u8; 32] = scalar::limbs_to_bytes(&limbs);
    let length = buffer.len() >> 5;
    for i in 0..length {
        let index = i << 5;
        if buffer[index] == bytes[0] {
            for j in 1..4 {
                if buffer[index + 1] != bytes[j] { continue; }
                return true;
            }
        }
    }
    false
}

fn bytes_to_u32(bytes: &[u8]) -> u32 {
    let a: [u8; 4] = [bytes[0], bytes[1], bytes[2], bytes[3]];
    u32::from_le_bytes(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creation() {
        // Valid size

        // Invalid size
    }

    #[test]
    fn test_contains_limbs() {
        let limbs: [u64; 4] = [18446744073709551615, 18446744073709551615, 18446744073709551615, 18446744073709551615];
        let lb = scalar::limbs_to_bytes(&limbs);
        let mut bytes: [u8; 32 * 10] = [0; 32 * 10];
        for i in 0..32 {
            bytes[7 * 32 + i] = lb[i];
        } 

        assert_eq!(true, contains_limbs(limbs, &bytes))
    }
}