use std::collections::BTreeMap;

use crate::macros::{elusiv_account, guard};
use solana_program::entrypoint::ProgramResult;
use crate::types::U256;
use crate::bytes::*;
use crate::error::ElusivError::{NullifierAlreadyExists};
use super::program_account::{SizedAccount, MAX_PERMITTED_DATA_LENGTH, get_multi_accounts_count};
use borsh::{BorshDeserialize, BorshSerialize};

/// The count of nullifiers is the count of leafes in the MT
const NULLIFIERS_COUNT: usize = 2usize.pow(super::MT_HEIGHT as u32);

/// We store nullifiers with the `NullifierMap` data structure for seaching and later N-SMT construction
pub type NullifierMap = BTreeMap<U256, bool>;
const NULLIFIER_MAP_STATIC_SIZE: usize = 4; // 4 bytes to store the u32 tree map size
const NULLIFIER_MAP_ELEMENT_SIZE: usize = U256::SIZE + 1;
const MAX_NULLIFIERS_PER_ACCOUNT: usize = (MAX_PERMITTED_DATA_LENGTH as usize - NULLIFIER_MAP_STATIC_SIZE) / NULLIFIER_MAP_ELEMENT_SIZE;

const NULLIFIER_ACCOUNT_SUB_ACCOUNTS_COUNT: usize = get_multi_accounts_count(MAX_NULLIFIERS_PER_ACCOUNT, NULLIFIERS_COUNT);
const_assert_eq!(NULLIFIER_ACCOUNT_SUB_ACCOUNTS_COUNT, 4);

const NULLIFIER_ACCOUNT_INTERMEDIARY_ACCOUNT_SIZE: usize = NULLIFIER_MAP_STATIC_SIZE + MAX_NULLIFIERS_PER_ACCOUNT * NULLIFIER_MAP_ELEMENT_SIZE;

/// NullifierAccount is a big-array storing `NULLIFIERS_COUNT` nullifiers over multiple accounts
/// - we use `NullifierMap`s to store the nullifiers
#[elusiv_account(pda_seed = b"tree", multi_account = (
    NULLIFIER_ACCOUNT_SUB_ACCOUNTS_COUNT;
    NULLIFIER_ACCOUNT_INTERMEDIARY_ACCOUNT_SIZE
))]
pub struct NullifierAccount {
    bump_seed: u8,
    initialized: bool,

    pubkeys: [U256; NULLIFIER_ACCOUNT_SUB_ACCOUNTS_COUNT],
    finished_setup: bool,

    root: U256, // this value is only valid, after the active tree has been reset
    nullifiers_count: u64,
}

/// Tree account after archivation (no big array anymore)
#[elusiv_account(pda_seed = b"archived_tree")]
pub struct ArchivedTreeAccount {
    bump_seed: u8,
    initialized: bool,

    commitment_root: U256,
    nullifier_root: U256,
}

impl<'a, 'b, 'c> NullifierAccount<'a, 'b, 'c> {
    pub fn can_insert_nullifier_hash(&self, _nullifier: U256) -> bool {
        //self.get_nullifiers_count() < NULLIFIERS_COUNT as u64// && not_contains(nullifier, self.get_full_array()), NoRoomForNullifier);
        panic!("TODO: add BTree Map");
    }

    pub fn insert_nullifier_hash(&mut self, _nullifier: U256) -> ProgramResult {
        let count = self.get_nullifiers_count();
        guard!(count < NULLIFIERS_COUNT as u64, NullifierAlreadyExists);

        //self.set(ptr as usize, nullifier);
        self.set_nullifiers_count(&(count + 1));

        panic!("TODO: add BTree Map and insert nullifier");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_insert_nullifier() {

    }

    #[test]
    fn test_insert_duplicate_nullifier() {

    }
}