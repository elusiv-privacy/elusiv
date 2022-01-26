use super::merkle::*;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use super::error::ElusivError::{
    InvalidStorageAccountSize,
    NullifierAlreadyUsed,
    NoRoomForNullifier,
    CommitmentAlreadyUsed,
    NoRoomForCommitment,
};
use super::poseidon::*;

pub const TREE_HEIGHT: usize = 12;
pub const TREE_SIZE: usize = ((2 as usize).pow(TREE_HEIGHT as u32 + 1) - 1) * 32;

pub const TREE_LEAF_START: usize = (2 as usize).pow(TREE_HEIGHT as u32) - 1;
pub const TREE_LEAF_COUNT: usize = (2 as usize).pow(TREE_HEIGHT as u32);

const NULLIFIERS_COUNT: usize = 1 << (TREE_HEIGHT);
const NULLIFIERS_SIZE: usize = NULLIFIERS_COUNT * 32;

const HISTORY_ARRAY_COUNT: usize = 10;
const HISTORY_ARRAY_SIZE: usize = HISTORY_ARRAY_COUNT * 32;

pub const TOTAL_SIZE: usize = TREE_SIZE + NULLIFIERS_SIZE + HISTORY_ARRAY_SIZE + 4 + 4 + (3 + TREE_HEIGHT + 1) * 32 + 8 + 2 + 2;

//solana_program::declare_id!("746Em3pvd2Rd2L3BRZ31RJ5qukorCiAw4kpudFkxgyBy");

pub struct StorageAccount<'a> {
    /// Merkle tree or arity 2
    /// - 2^{TREE_HEIGHT + 1} - 1 32 byte words
    pub merkle_tree: &'a mut [u8],

    /// Nullifier hashes
    /// - TREE_HEIGHT 32 byte words
    nullifier_hashes: &'a mut [u8],

    /// Root history
    /// - HISTORY_ARRAY_COUNT 32 byte words
    root_history: &'a mut [u8],

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

impl<'a> StorageAccount<'a> {
    pub fn from(buffer: &'a mut [u8]) -> Result<Self, ProgramError> {
        if buffer.len() != TOTAL_SIZE { return Err(InvalidStorageAccountSize.into()); }

        let (merkle_tree, rest) = buffer.split_at_mut(TREE_SIZE);
        let (nullifier_hashes, rest) = rest.split_at_mut(NULLIFIERS_SIZE);
        let (root_history, rest) = rest.split_at_mut(HISTORY_ARRAY_SIZE);
        let (next_leaf_pointer, rest) = rest.split_at_mut(4);
        let (next_nullifier_pointer, rest) = rest.split_at_mut(4);
        let (hashing_state_storage, rest) = rest.split_at_mut(3 * 32);
        let (finished_hashes_storage, rest) = rest.split_at_mut((TREE_HEIGHT + 1) * 32);
        let (committed_amount, rest) = rest.split_at_mut(8);
        let (current_hash_iteration, rest) = rest.split_at_mut(2);
        let (current_hash_tree_position, _) = rest.split_at_mut(2);

        Ok(StorageAccount {
            merkle_tree,
            nullifier_hashes,
            root_history,
            next_leaf_pointer,
            next_nullifier_pointer,
            hashing_state_storage,
            finished_hashes_storage,
            committed_amount,
            current_hash_iteration,
            current_hash_tree_position,
        })
    }

    // Hashing
    pub fn get_hashing_state(&self) -> [Scalar; 3] {
        [
            from_bytes_le(&self.hashing_state_storage[..32]),
            from_bytes_le(&self.hashing_state_storage[32..64]),
            from_bytes_le(&self.hashing_state_storage[64..]),
        ]
    }
    pub fn set_hashing_state(&mut self, state: [Scalar; 3]) {
        let mut bytes: Vec<u8> = to_bytes_le(state[0]);
        bytes.append(&mut to_bytes_le(state[1]));
        bytes.append(&mut to_bytes_le(state[2]));

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
        for (i, &byte) in to_bytes_le(value).iter().enumerate() {
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
        let bytes = limbs_to_bytes(&nullifier_hash);
        Self::set(self.nullifier_hashes, pointer, 4, &bytes)?;

        // Increment pointer
        pointer += 1;
        Self::set(&mut self.next_nullifier_pointer, 0, 4, &pointer.to_le_bytes())?;

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
    pub fn add_commitment(&mut self) -> ProgramResult {
        let pointer = self.leaf_pointer() as usize;
        let values = self.get_finished_hashes_storage();

        // Additional commitment security check
        let commitment = values[0];
        self.can_insert_commitment(bytes_to_limbs(&commitment))?;

        // Save last root
        let root = &self.merkle_tree[..32];
        Self::set(self.root_history, (pointer % HISTORY_ARRAY_COUNT) * 32, 32, root)?;

        // Insert values into the tree
        /*for value in values {
            msg!(&format!("{}", from_bytes_le(&value)));
        }*/
        insert_hashes(&mut self.merkle_tree, values, pointer);

        // Increment pointer
        self.increment_leaf_pointer(1)?;

        Ok(())
    }

    pub fn increment_leaf_pointer(&mut self, count: usize) -> ProgramResult  {
        let mut pointer = self.leaf_pointer() as usize;
        pointer += count;
        Self::set(self.next_leaf_pointer, 0, 4, &pointer.to_le_bytes())?;
        Ok(())
    }

    /// Check whether the provided root is valid
    /// 
    /// ### Arguments
    /// 
    /// * `root` - merkle root provided as 4 u64 limbs
    pub fn is_root_valid(&mut self, root: ScalarLimbs) -> bool {
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

    pub fn leaf_pointer(&self) -> u32 {
        bytes_to_u32(&self.next_leaf_pointer)
    }

    pub fn nullifier_pointer(&self) -> u32 {
        bytes_to_u32(&self.next_nullifier_pointer)
    }
}

/// Checks whether a word represented by 4 u64 limbs is contained inside a byte array
/// 
/// ### Arguments
/// 
/// * `limbs` - 4 u64 limbs
/// * `buffer` - bytes to search in
fn contains_limbs(limbs: ScalarLimbs, buffer: &[u8]) -> bool {
    let bytes: [u8; 32] = limbs_to_bytes(&limbs);
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

fn bytes_to_u64(bytes: &[u8]) -> u64 {
    let a: [u8; 8] = [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]];
    u64::from_le_bytes(a)
}

fn bytes_to_u32(bytes: &[u8]) -> u32 {
    let a: [u8; 4] = [bytes[0], bytes[1], bytes[2], bytes[3]];
    u32::from_le_bytes(a)
}

fn bytes_to_u16(bytes: &[u8]) -> u16 {
    let a: [u8; 2] = [bytes[0], bytes[1]];
    u16::from_le_bytes(a)
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
        let lb = limbs_to_bytes(&limbs);
        let mut bytes: [u8; 32 * 10] = [0; 32 * 10];
        for i in 0..32 {
            bytes[7 * 32 + i] = lb[i];
        } 

        assert_eq!(true, contains_limbs(limbs, &bytes))
    }

    #[test]
    fn test_init_correct_size() {
        let mut data: Vec<u8> = vec![0; TOTAL_SIZE];
        StorageAccount::from(&mut data).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_init_wrong_size() {
        let mut data: Vec<u8> = vec![0; TOTAL_SIZE - 1];
        StorageAccount::from(&mut data).unwrap();
    }

    #[test]
    fn test_get_finished_hashes() {
        let mut data: Vec<u8> = vec![0; TOTAL_SIZE];
        let start_offset = TREE_SIZE + NULLIFIERS_SIZE + HISTORY_ARRAY_SIZE + 4 + 4 + 3 * 32;
        for i in 0..=TREE_HEIGHT {
            let scalar = from_str_10(&format!("{}", i + 1));
            let bytes = to_bytes_le(scalar);
            for (j, &byte) in bytes.iter().enumerate() {
                data[start_offset + i * 32 + j] = byte;
            }
        }
        let storage = StorageAccount::from(&mut data).unwrap();

        let hashes = storage.get_finished_hashes_storage();
        for i in 0..=TREE_HEIGHT {
            let scalar = from_str_10(&format!("{}", i + 1));
            assert_eq!(
                from_bytes_le(&hashes[i]),
                scalar
            )
        }
    }
}