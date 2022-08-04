use crate::macros::{elusiv_account, two_pow};
use crate::types::U256;
use crate::bytes::*;
use super::program_account::*;
use borsh::{BorshDeserialize, BorshSerialize};

/// Height of the active MT
/// - we define the height using the amount of leaves
/// - a tree of height n has 2ˆn leaves
pub const MT_HEIGHT: u32 = 20;

/// Count of all nodes in the MT
pub const MT_SIZE: usize = mt_size(MT_HEIGHT);

pub const fn mt_size(height: u32) -> usize {
    two_pow!(height + 1) - 1
}

/// Count of all commitments (leaves) in the MT
pub const MT_COMMITMENT_COUNT: usize = two_pow!(MT_HEIGHT);

/// Index of the first commitment in the MT
pub const MT_COMMITMENT_START: usize = two_pow!(MT_HEIGHT) - 1;

/// Since before submitting a proof request the current root can change, we store the previous ones
pub const HISTORY_ARRAY_COUNT: usize = 100;

const VALUES_PER_ACCOUNT: usize = 83_887;
const ACCOUNT_SIZE: usize = SUB_ACCOUNT_ADDITIONAL_SIZE + VALUES_PER_ACCOUNT * U256::SIZE;

const ACCOUNTS_COUNT: usize = u64_as_usize_safe(div_ceiling((MT_SIZE * U256::SIZE) as u64, ACCOUNT_SIZE as u64));
const_assert_eq!(ACCOUNTS_COUNT, 25);

// The `StorageAccount` contains the active MT that stores new commitments
// - the MT is stored as an array with the first element being the root and the second and third elements the layer below the root
// - in order to manage a growing number of commitments, once the MT is full it get's reset (and the root is stored elsewhere)
#[elusiv_account(pda_seed = b"storage", multi_account = (ACCOUNTS_COUNT; ACCOUNT_SIZE))]
pub struct StorageAccount {
    pda_data: PDAAccountData,
    multi_account_data: MultiAccountAccountData<ACCOUNTS_COUNT>,

    // Points to the next commitment in the active MT
    next_commitment_ptr: u32,

    // The amount of already finished (closed) MTs
    trees_count: u32,

    // The amount of archived MTs
    archived_count: u32,

    // Stores the last HISTORY_ARRAY_COUNT roots of the active tree
    active_mt_root_history: [U256; HISTORY_ARRAY_COUNT],
    mt_roots_count: u32, // required since we batch insert commitments
}

impl<'a, 'b, 't> StorageAccount<'a, 'b, 't> {
    pub fn reset(&mut self) {
        self.set_next_commitment_ptr(&0);
        self.set_mt_roots_count(&0);

        for i in 0..self.active_mt_root_history.len() {
            self.active_mt_root_history[i] = 0;
        }
    }

    pub fn is_full(&self) -> bool {
        let ptr = self.get_next_commitment_ptr() as usize;
        ptr >= MT_COMMITMENT_COUNT
    }

    fn account_and_local_index(&self, index: usize) -> (usize, usize) {
        let account_index = index / VALUES_PER_ACCOUNT;
        (account_index, index % VALUES_PER_ACCOUNT)
    }

    /// `level`: `0` is the root level, `MT_HEIGHT` the commitment level
    pub fn get_node(&self, index: usize, level: usize) -> U256 {
        assert!(level <= MT_HEIGHT as usize);

        let ptr = self.get_next_commitment_ptr() as usize;

        // Accessing a node, that is non-existent (yet) -> we use the default value 
        if use_default_value(index, level, ptr) {
            EMPTY_TREE[MT_HEIGHT as usize - level]
        } else {
            let (account_index, local_index) = self.account_and_local_index(mt_array_index(index, level));
            self.try_execute_on_sub_account(account_index, |data| {
                U256::try_from_slice(
                    &data[local_index * U256::SIZE..(local_index + 1) * U256::SIZE]
                )
            }).unwrap()
        }
    }

    pub fn set_node(&mut self, value: &U256, index: usize, level: usize) {
        assert!(level <= MT_HEIGHT as usize);
        assert!(index < two_pow!(usize_as_u32_safe(level)));

        let (account_index, local_index) = self.account_and_local_index(mt_array_index(index, level));
        self.try_execute_on_sub_account(account_index, |data| {
            U256::override_slice(
                value,
                &mut data[local_index * U256::SIZE..(local_index + 1) * U256::SIZE]
            )
        }).unwrap();
    }

    pub fn get_root(&self) -> U256 {
        self.get_node(0, 0)
    }

    /// A root is valid if it's the current root or inside of the active_mt_root_history array
    pub fn is_root_valid(&self, root: U256) -> bool {
        let max_history_roots = std::cmp::min(self.get_mt_roots_count() as usize, HISTORY_ARRAY_COUNT);
        root == self.get_root() || (max_history_roots > 0 && contains(root, &self.active_mt_root_history[..max_history_roots * 32]))
    }

    #[allow(clippy::needless_range_loop)]
    pub fn get_mt_opening(&self, index: usize) -> [U256; MT_HEIGHT as usize] {
        let mut opening = [[0; 32]; MT_HEIGHT as usize];
        let mut index = index;

        for i in 0..MT_HEIGHT as usize {
            let level = MT_HEIGHT as usize - i;
            let n_index = if index % 2 == 0 { index + 1 } else { index - 1};
            opening[i] = self.get_node(n_index, level);
            index >>= 1;
        }

        opening
    }
}

pub fn mt_array_index(index: usize, level: usize) -> usize {
    two_pow!(usize_as_u32_safe(level)) - 1 + index
}

fn use_default_value(index: usize, level: usize, next_leaf_ptr: usize) -> bool {
    let level_inv = MT_HEIGHT as usize - level;
    next_leaf_ptr == 0 || index > (next_leaf_ptr - 1) >> level_inv
}

/// `EMPTY_TREE[0]` is the empty commitment, all values above are the hashes (`EMPTY_TREE[MT_HEIGHT]` is the root)
/// - all values are in mr-form
pub const EMPTY_TREE: [U256; MT_HEIGHT as usize + 1] = [
    [130, 154, 1, 250, 228, 248, 226, 43, 27, 76, 165, 173, 91, 84, 165, 131, 78, 224, 152, 167, 123, 115, 91, 213, 116, 49, 167, 101, 109, 41, 161, 8],
    [80, 180, 254, 174, 183, 151, 82, 229, 123, 24, 44, 98, 7, 166, 152, 78, 191, 94, 109, 201, 215, 229, 108, 66, 136, 150, 102, 80, 152, 67, 183, 24],
    [245, 111, 221, 89, 163, 253, 120, 251, 192, 102, 179, 28, 32, 160, 220, 2, 210, 250, 182, 48, 149, 102, 78, 135, 242, 178, 240, 129, 158, 28, 194, 45],
    [110, 88, 234, 59, 103, 185, 212, 46, 227, 64, 178, 47, 204, 121, 184, 122, 140, 228, 122, 122, 109, 4, 4, 203, 29, 99, 252, 22, 192, 185, 82, 32],
    [37, 132, 186, 12, 74, 180, 105, 226, 213, 211, 193, 225, 27, 50, 138, 4, 63, 92, 234, 13, 17, 8, 83, 158, 236, 140, 4, 107, 19, 189, 227, 31],
    [198, 123, 74, 104, 202, 32, 61, 240, 51, 94, 111, 182, 36, 122, 130, 150, 62, 80, 89, 255, 161, 142, 26, 242, 207, 185, 133, 129, 254, 165, 170, 0],
    [77, 214, 11, 70, 225, 121, 188, 80, 144, 34, 40, 76, 75, 163, 124, 153, 146, 178, 225, 180, 243, 38, 20, 128, 220, 24, 194, 179, 70, 169, 160, 28],
    [77, 199, 105, 95, 222, 183, 99, 229, 133, 193, 250, 29, 35, 92, 66, 209, 150, 145, 122, 205, 136, 103, 205, 207, 32, 181, 252, 167, 89, 74, 52, 18],
    [54, 63, 5, 212, 210, 204, 167, 180, 13, 135, 84, 97, 129, 172, 209, 79, 29, 33, 249, 83, 92, 61, 19, 196, 93, 251, 179, 42, 250, 163, 197, 22],
    [190, 171, 114, 180, 49, 21, 132, 161, 141, 16, 77, 191, 105, 239, 105, 105, 8, 64, 253, 159, 196, 2, 99, 181, 129, 34, 5, 36, 120, 240, 129, 23],
    [228, 244, 77, 241, 92, 212, 9, 105, 212, 241, 190, 161, 17, 14, 166, 107, 164, 226, 117, 236, 56, 57, 174, 36, 61, 114, 205, 34, 240, 31, 13, 33],
    [177, 89, 55, 44, 13, 53, 50, 76, 143, 95, 226, 63, 243, 253, 248, 153, 1, 33, 141, 61, 84, 78, 175, 170, 17, 92, 8, 242, 221, 246, 226, 5],
    [237, 115, 97, 145, 232, 65, 190, 215, 163, 149, 19, 111, 159, 166, 20, 97, 61, 235, 236, 85, 0, 247, 173, 110, 244, 211, 71, 235, 223, 210, 220, 3],
    [204, 81, 128, 228, 236, 75, 32, 52, 141, 233, 50, 175, 99, 20, 90, 132, 38, 45, 25, 223, 247, 10, 112, 33, 0, 79, 144, 138, 59, 187, 10, 20],
    [121, 74, 240, 81, 202, 98, 247, 184, 68, 43, 52, 182, 165, 2, 113, 154, 163, 26, 73, 186, 58, 190, 218, 129, 134, 0, 143, 187, 72, 241, 51, 29],
    [23, 63, 108, 217, 4, 51, 59, 214, 94, 255, 90, 176, 19, 205, 240, 52, 98, 138, 26, 96, 194, 30, 124, 185, 113, 59, 71, 135, 22, 189, 223, 21],
    [24, 105, 228, 247, 251, 67, 134, 40, 68, 98, 71, 80, 37, 131, 62, 181, 223, 238, 211, 88, 230, 89, 159, 179, 65, 229, 221, 202, 160, 119, 2, 31],
    [236, 110, 13, 154, 171, 245, 53, 65, 211, 210, 85, 234, 88, 34, 220, 212, 62, 173, 99, 42, 162, 174, 187, 11, 168, 240, 202, 51, 148, 184, 142, 34],
    [51, 251, 186, 127, 55, 143, 98, 20, 22, 65, 243, 34, 243, 240, 93, 64, 22, 16, 89, 136, 58, 238, 219, 189, 137, 240, 147, 136, 227, 199, 8, 18],
    [68, 89, 173, 222, 230, 170, 252, 78, 231, 34, 126, 164, 43, 187, 137, 244, 131, 254, 238, 133, 35, 169, 175, 160, 145, 110, 94, 131, 102, 137, 115, 40],
    [215, 208, 169, 37, 21, 214, 245, 126, 221, 48, 194, 233, 207, 177, 29, 18, 85, 167, 242, 130, 212, 71, 7, 78, 114, 10, 173, 101, 60, 84, 109, 9],
];

#[cfg(feature = "test-elusiv")]
pub fn empty_root_raw() -> crate::types::RawU256 {
    use crate::fields::{fr_to_u256_le_repr, scalar_skip_mr, u256_to_big_uint};
    crate::types::RawU256::new(
        fr_to_u256_le_repr(&scalar_skip_mr(u256_to_big_uint(&EMPTY_TREE[MT_HEIGHT as usize])))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{macros::storage_account, commitment::{poseidon_hash::full_poseidon2_hash}, fields::{u256_to_fr_skip_mr, u256_from_str}};
    use ark_bn254::Fr;
    use std::str::FromStr;

    #[test]
    fn test_mt_array_index() {
        assert_eq!(0, mt_array_index(0, 0));

        assert_eq!(1, mt_array_index(0, 1));
        assert_eq!(2, mt_array_index(1, 1));

        assert_eq!(3, mt_array_index(0, 2));
        assert_eq!(4, mt_array_index(1, 2));
        assert_eq!(5, mt_array_index(2, 2));
        assert_eq!(6, mt_array_index(3, 2));
    }

    #[test]
    fn test_empty_root_raw() {
        assert_eq!(empty_root_raw().reduce(), EMPTY_TREE[MT_HEIGHT as usize]);
    }

    #[test]
    fn test_set_node() {
        storage_account!(mut storage_account);
        storage_account.set_next_commitment_ptr(&(MT_COMMITMENT_COUNT as u32));

        for level in 0..=MT_HEIGHT {
            let last = two_pow!(level) - 1;

            // First node
            storage_account.set_node(&[1; 32], 0, level as usize);
            assert_eq!(storage_account.get_node(0, level as usize), [1; 32]);

            // Last node
            storage_account.set_node(&[2; 32], last, level as usize);
            assert_eq!(storage_account.get_node(last, level as usize), [2; 32]);
        }
    }

    #[test]
    #[should_panic]
    fn test_set_node_invalid_level() {
        storage_account!(mut storage_account);
        storage_account.set_node(&[1; 32], 0, MT_HEIGHT as usize + 1);
    }

    #[test]
    #[should_panic]
    fn test_set_node_invalid_level_index() {
        storage_account!(mut storage_account);
        storage_account.set_node(&[1; 32], 4, 2);
    }

    #[test]
    fn test_use_default_value() {
        assert!(!use_default_value(0, MT_HEIGHT as usize, 1));
        assert!(use_default_value(1, MT_HEIGHT as usize, 1));

        for level in 0..=MT_HEIGHT as usize {
            // Empty tree
            assert!(use_default_value(0, level, 0));

            // Commitments
            assert!(!use_default_value(0, level, 1));
            assert!(!use_default_value(0, level, 2));
        }
    }

    #[test]
    fn test_get_node() {
        storage_account!(mut storage_account);

        // No commitments -> default values
        assert_eq!(
            storage_account.get_node(0, 0),
            u256_from_str("11702828337982203149177882813338547876343922920234831094975924378932809409969")
        );
        assert_eq!(
            storage_account.get_node(0, MT_HEIGHT as usize),
            u256_from_str("14744269619966411208579211824598458697587494354926760081771325075741142829156")
        );
        for level in 0..=MT_HEIGHT {
            assert_eq!(
                storage_account.get_node(0, level as usize),
                EMPTY_TREE[(MT_HEIGHT - level) as usize],
            );
        }

        for i in 0..4 {
            storage_account.set_next_commitment_ptr(&(i as u32 + 1));

            for level in 0..=MT_HEIGHT as usize {
                assert_eq!(
                    storage_account.get_node(i >> (MT_HEIGHT as usize - level), level),
                    u256_from_str("0")
                );

                // Default values right of commitment
                let offset = (i + 1) >> (MT_HEIGHT as usize - level);
                if offset > (i + 1) {
                    assert_eq!(
                        storage_account.get_node(offset, level),
                        EMPTY_TREE[MT_HEIGHT as usize - level],
                    );
                }
            }
        }
    }

    #[test]
    fn test_get_root() {
        storage_account!(mut storage_account);
        storage_account.set_node(&[1; 32], 0, 0);
        storage_account.set_next_commitment_ptr(&1);

        assert_eq!(
            storage_account.get_root(),
            [1; 32]
        );
    }

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn test_hash_two_commitments_together() {
        let a = Fr::from_str("8806693615866680221624359022326040351320802923100496896469027799555969415608").unwrap();
        let b = Fr::from_str("10325823052538184185762853738620713863393182243594528391700012489616960720113").unwrap();
        let mut hash = full_poseidon2_hash(a, b);
        for i in 1..MT_HEIGHT as usize {
            hash = full_poseidon2_hash(hash, u256_to_fr_skip_mr(&EMPTY_TREE[i]));
        }
        assert_eq!(hash, Fr::from_str("2405070960812791252603303680410822171263982421393937538616415344325138142909").unwrap());
    }

    #[test]
    fn test_is_root_valid() {
        storage_account!(storage_account);
        assert!(storage_account.is_root_valid(EMPTY_TREE[MT_HEIGHT as usize]));
        assert!(!storage_account.is_root_valid([0; 32]));
    }
}