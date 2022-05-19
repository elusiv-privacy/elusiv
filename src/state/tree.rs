use crate::macros::{elusiv_account, guard};
use solana_program::entrypoint::ProgramResult;
use crate::types::U256;
use crate::bytes::*;
use crate::error::ElusivError::{NullifierAlreadyExists};

/// The count of nullifiers is the count of leafes in the MT
const NULLIFIERS_COUNT: usize = 2usize.pow(super::MT_HEIGHT as u32);

const NULLIFIER_ACCOUNT_SUB_ACCOUNTS_COUNT: usize = 1;

/// NullifierAccount is a  big-array storing `NULLIFIERS_COUNT` nullifiers over multiple PDA accounts
/// - we use BTreeMaps to store the nullifiers
#[elusiv_account(pda_seed = b"tree", multi_account = NULLIFIER_ACCOUNT_SUB_ACCOUNTS_COUNT)]
pub struct NullifierAccount {
    nullifiers_count: u64,
}

/// Tree account after archivation (no big array anymore)
#[elusiv_account(pda_seed = b"archived_tree")]
pub struct ArchivedTreeAccount {
    commitment_root: U256,
    nullifier_root: U256,
}

impl<'a, 'b> NullifierAccount<'a, 'b> {
    pub fn can_insert_nullifier(&self, nullifier: U256) -> ProgramResult {
        guard!(self.get_nullifiers_count() < NULLIFIERS_COUNT as u64, NullifierAlreadyExists);
        //guard!(not_contains(nullifier, self.get_full_array()), NoRoomForNullifier);

        Ok(())
    }

    pub fn insert_nullifier(&mut self, nullifier: U256) -> ProgramResult {
        let count = self.get_nullifiers_count();
        guard!(count < NULLIFIERS_COUNT as u64, NullifierAlreadyExists);

        //self.set(ptr as usize, nullifier);
        self.set_nullifiers_count(count + 1);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_insert_nullifier() {
        panic!()
    }

    #[test]
    fn test_insert_duplicate_nullifier() {
        panic!()
    }
}