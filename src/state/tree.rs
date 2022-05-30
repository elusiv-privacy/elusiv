use crate::macros::{elusiv_account, guard};
use solana_program::entrypoint::ProgramResult;
use crate::types::U256;
use crate::bytes::*;
use crate::error::ElusivError::{NullifierAlreadyExists};
use super::program_account::{SizedAccount, MultiAccountAccount};
use borsh::{BorshDeserialize, BorshSerialize};

/// The count of nullifiers is the count of leafes in the MT
const NULLIFIERS_COUNT: usize = 2usize.pow(super::MT_HEIGHT as u32);

const NULLIFIER_ACCOUNT_SUB_ACCOUNTS_COUNT: usize = 1;

/// NullifierAccount is a  big-array storing `NULLIFIERS_COUNT` nullifiers over multiple PDA accounts
/// - we use BTreeMaps to store the nullifiers
#[elusiv_account(pda_seed = b"tree", multi_account = (
    NULLIFIER_ACCOUNT_SUB_ACCOUNTS_COUNT;
    0
))]
pub struct NullifierAccount {
    bump_seed: u8,
    initialized: bool,

    pubkeys: [U256; NULLIFIER_ACCOUNT_SUB_ACCOUNTS_COUNT],
    finished_setup: bool,

    root: U256,
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
    /*#[test]
    fn test_insert_nullifier() {
        panic!()
    }

    #[test]
    fn test_insert_duplicate_nullifier() {
        panic!()
    }*/
}