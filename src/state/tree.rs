use crate::macros::{ elusiv_account, guard, two_pow };
use solana_program::entrypoint::ProgramResult;
use crate::types::U256;
use crate::bytes::*;
use crate::error::ElusivError::{ NullifierAlreadyExists, NoRoomForNullifier };
use super::program_account::*;

const NULLIFIERS_COUNT: usize = two_pow!(super::MT_HEIGHT);

/// Big-array storing NULLIFIERS_COUNT nullifiers over multiple PDA accounts
#[elusiv_account(pda_seed = b"tree", big_array = [U256; NULLIFIERS_COUNT])]
struct NullifierAccount {
    next_nullifier_ptr: u64,
}

/// Tree account after archivation (no big array anymore)
#[elusiv_account(pda_seed = b"archived_tree")]
struct ArchivedTreeAccount {
    commitment_root: U256,
    nullifier_root: U256,
}

impl<'a> NullifierAccount<'a> {
    pub fn can_insert_nullifier(&self, nullifier: U256) -> ProgramResult {
        let ptr = self.get_next_nullifier_ptr();
        guard!(ptr < NULLIFIERS_COUNT as u64, NullifierAlreadyExists);
        guard!(not_contains(nullifier, self.get_full_array()), NoRoomForNullifier);

        Ok(())
    }

    pub fn insert_nullifier(&mut self, nullifier: U256) -> ProgramResult {
        let ptr = self.get_next_nullifier_ptr();
        guard!(ptr < NULLIFIERS_COUNT as u64, NullifierAlreadyExists);

        self.set(ptr as usize, nullifier);
        self.set_next_nullifier_ptr(ptr + 1);

        Ok(())
    }
}