use crate::fields::fr_to_u256_le;
use crate::fields::u256_to_fr;
use crate::macros::{elusiv_account, two_pow};
use crate::types::U256;
use crate::bytes::*;
use super::program_account::*;
use borsh::{BorshDeserialize, BorshSerialize};
use ark_bn254::Fr;
use ark_ff::{BigInteger256};

/// Height of the active Merkle Tree
pub const MT_HEIGHT: u32 = 20;

/// Count of all nodes in the merkle-tree
pub const MT_SIZE: usize = two_pow!(MT_HEIGHT + 1) - 1;

/// Count of all commitments (leafes) in the merkle-tree
pub const MT_COMMITMENT_COUNT: usize = two_pow!(MT_HEIGHT);

/// Index of the first commitment in the Merkle Tree
pub const MT_COMMITMENT_START: usize = two_pow!(MT_HEIGHT) - 1;

/// Since before submitting a proof request the current root can change, we store the previous ones
const HISTORY_ARRAY_COUNT: usize = 100;

pub const STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT: usize = big_array_accounts_count(MT_SIZE, U256::SIZE);
const_assert_eq!(STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT, 7);

/// `EMPTY_TREE[0]` is the empty commitment, all values above are the hashes
pub const EMPTY_TREE: [Fr; MT_HEIGHT as usize + 1] = [
    Fr::new(BigInteger256::new([3162363550698150530, 9486080942857866267, 15374008727889305678, 621823773387469172])),
    Fr::new(BigInteger256::new([16524436797946508368, 5663459082060437627, 4786453218948112063, 1780966499111310984])),
    Fr::new(BigInteger256::new([18120511978754371573, 206215742733117120, 9749843034257160914, 3297229344299070194])),
    Fr::new(BigInteger256::new([3374525873845327982, 8842951786222469347, 14628822357859034252, 2329128191974466333])),
    Fr::new(BigInteger256::new([16314769355260462117, 327129018270864341, 11408471150400527423, 2297888125986573548])),
    Fr::new(BigInteger256::new([17311028596387249094, 10845365150759738931, 17445412932997959742, 48033258554112463])),
    Fr::new(BigInteger256::new([5817658827105621581, 11059894328710668944, 9229044364542587538, 2062834750464334044])),
    Fr::new(BigInteger256::new([16529257223091242829, 15078715808286425477, 14973738173337342358, 1311755140402558240])),
    Fr::new(BigInteger256::new([13017598453810020150, 5751567870812522253, 14128703922746499357, 1640897934100003677])),
    Fr::new(BigInteger256::new([11638450640257199038, 7595865483997155469, 13070293587080331272, 1693899333659075201])),
    Fr::new(BigInteger256::new([7568814144423326948, 7756902877083136468, 2643112947961160356, 2381594894185361981])),
    Fr::new(BigInteger256::new([5490509226410727857, 11094896903217897359, 12299135231088992513, 424172748035087377])),
    Fr::new(BigInteger256::new([15545935430856831981, 6995399324906788259, 7975302095956667197, 278329136157021172])),
    Fr::new(BigInteger256::new([3756085570044973516, 9536957579284113805, 2409437860359384358, 1444172494927843072])),
    Fr::new(BigInteger256::new([13328230243323890297, 11128679064958937924, 9357000335258360483, 2104290745598804102])),
    Fr::new(BigInteger256::new([15436988243663142679, 3814774368829636446, 13365591614313630306, 1576186299059485553])),
    Fr::new(BigInteger256::new([2920096157871139096, 13060020165704901188, 12943162699953729247, 2234479897611199809])),
    Fr::new(BigInteger256::new([4698931903603764972, 15338172196201026259, 845461366581210430, 2490130590646071464])),
    Fr::new(BigInteger256::new([1468893897005136691, 4638128118315565334, 13680790228231065622, 1299508272559288457])),
    Fr::new(BigInteger256::new([5691611937644566852, 17620820813181493991, 11578659137028816515, 2914824457222516369])),
    Fr::new(BigInteger256::new([9148453604387573975, 1305394973545476317, 5622541637850933077, 679291737183423090])),
];

const ZERO: Fr = Fr::new(BigInteger256::new([0, 0, 0, 0]));

// The `StorageAccount` contains the active Merkle Tree that stores new commitments
// - the MT is stored as an array with the first element being the root and the second and third elements the layer below the root
// - in order to manage a growing number of commitments, once the MT is full it get's reset (and the root is stored elsewhere)
#[elusiv_account(pda_seed = b"storage", multi_account = (
    STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT;
    max_account_size(U256::SIZE)
))]
pub struct StorageAccount {
    bump_seed: u8,
    version: u8,
    initialized: bool,

    pubkeys: [U256; STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT],

    // Points to the next commitment in the active MT
    next_commitment_ptr: u32,

    // The amount of already finished (closed) MTs
    trees_count: u64,

    // The amount of archived MTs
    archived_count: u64,

    // Stores the last HISTORY_ARRAY_COUNT roots of the active tree
    active_mt_root_history: [U256; HISTORY_ARRAY_COUNT],
}

impl<'a, 'b, 't> BigArrayAccount<'t> for StorageAccount<'a, 'b, 't> {
    type T = U256;
    const VALUES_COUNT: usize = MT_SIZE;
}

impl<'a, 'b, 't> MultiInstancePDAAccount for StorageAccount<'a, 'b, 't> {
    const MAX_INSTANCES: u64 = 1;
}

impl<'a, 'b, 't> StorageAccount<'a, 'b, 't> {
    pub fn reset(&mut self) {
        self.set_next_commitment_ptr(&0);

        for i in 0..self.active_mt_root_history.len() {
            self.active_mt_root_history[i] = 0;
        }
    }

    pub fn is_full(&self) -> bool {
        let ptr = self.get_next_commitment_ptr() as usize;
        ptr >= MT_COMMITMENT_COUNT
    }

    /// Inserts commitment and the above hashes
    pub fn insert_commitment(&mut self, values: &[U256]) {
        assert!(values.len() == MT_HEIGHT as usize + 1);

        let ptr = self.get_next_commitment_ptr();

        // Save last root
        self.set_active_mt_root_history(ptr as usize % HISTORY_ARRAY_COUNT, &self.get_root());

        // Insert values into the tree
        for (i, &value) in values.iter().enumerate() {
            let level = MT_HEIGHT as usize - i;
            let index = ptr >> i;
            self.set_node(&value, index as usize, level);
        }

        self.set_next_commitment_ptr(&(ptr + 1));
    }

    /// `level`: 0 is the root level, `MT_HEIGHT` the commitment level
    pub fn get_node(&self, index: usize, level: usize) -> Fr {
        assert!(level <= MT_HEIGHT as usize);

        let ptr = self.get_next_commitment_ptr() as usize;

        // Accessing a node, that is non-existent (yet) -> we use the default value 
        if use_default_value(index, level, ptr) {
            EMPTY_TREE[MT_HEIGHT as usize - level]
        } else {
            u256_to_fr(&self.get(mt_array_index(index, level)))
        }
    }

    fn set_node(&mut self, value: &U256, index: usize, level: usize) {
        assert!(level <= MT_HEIGHT as usize);

        self.set(mt_array_index(index, level), *value);
    }

    pub fn get_root(&self) -> U256 {
        fr_to_u256_le(&self.get_node(0, 0))
    }

    /// A root is valid if it's the current root or inside of the active_mt_root_history array
    pub fn is_root_valid(&self, root: U256) -> bool {
        // TODO: only check till ptr
        root == self.get_root() || contains(root, self.active_mt_root_history)
    }

    #[allow(clippy::needless_range_loop)]
    pub fn get_mt_opening(&self, index: usize) -> [Fr; MT_HEIGHT as usize] {
        let mut opening = [ZERO; MT_HEIGHT as usize];
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
    next_leaf_ptr == 0 || (index >> level_inv) > ((next_leaf_ptr - 1) >> level_inv)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macros::{account, generate_storage_accounts, generate_storage_accounts_valid_size};
    use solana_program::{account_info::AccountInfo};
    use super::super::program_account::{MultiAccountAccount};
    use std::str::FromStr;

    macro_rules! storage_account {
        ($id: ident) => {
            let mut data = vec![0; StorageAccount::SIZE];
            generate_storage_accounts_valid_size!(accounts);
            let $id = StorageAccount::new(&mut data, accounts).unwrap();
        };
    }

    #[test]
    fn test_storage_account() {
        assert_eq!(STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT, 7);
    }

    #[test]
    fn test_get_default_mt_values() {
        storage_account!(storage_account);

        // Commitment
        assert_eq!(storage_account.get_node(0, MT_HEIGHT as usize), Fr::from_str("14744269619966411208579211824598458697587494354926760081771325075741142829156").unwrap());

        // root
        assert_eq!(storage_account.get_node(0, 0), Fr::from_str("11702828337982203149177882813338547876343922920234831094975924378932809409969").unwrap());

        for level in (0..=MT_HEIGHT as usize).rev() {
            // Empty tree
            assert!(use_default_value(0, level, 0));

            // One commitment
            assert!(!use_default_value(0, level, 1));
        }
    }
}