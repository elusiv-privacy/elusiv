use crate::macros::{ ElusivAccount, remove_original_implementation, guard };
use solana_program::entrypoint::ProgramResult;
use crate::types::U256;
use crate::bytes::not_contains;
use crate::error::ElusivError::{
    NullifierAlreadyExists,
    NoRoomForNullifier,
    InvalidMerkleRoot,
};

// The nullifiers count is 2 times the amount of commitments (since bind stores two commitments)
const NULLIFIERS_COUNT: usize = 1 << (super::TREE_HEIGHT + 1);

#[derive(ElusivAccount)]
#[remove_original_implementation]
struct NullifierAccount {
    key: U256,

    nullifiers: [U256; NULLIFIERS_COUNT],
    // The root is only set AFTER the nullifier account is archived!
    root: U256,
    next_nullifier: u64,
}

// Nullifiers and root
impl<'a> NullifierAccount<'a> {
    pub fn can_insert_nullifier(&self, nullifier: U256) -> ProgramResult {
        // Room for next nullifier
        guard!(
            self.get_next_nullifier() < NULLIFIERS_COUNT as u64,
            NullifierAlreadyExists
        );

        // Check that nullifier does not already exist
        guard!(
            not_contains(nullifier, &self.nullifiers),
            NoRoomForNullifier
        );

        Ok(())
    }

    pub fn insert_nullifier(&mut self, nullifier: U256) -> ProgramResult {
        let ptr = self.get_next_nullifier();

        // Room for next nullifier
        guard!(
            ptr < NULLIFIERS_COUNT as u64,
            NullifierAlreadyExists
        );

        // Save nullifier
        self.set_nullifiers(ptr as usize, &nullifier);

        // Increment pointer
        self.set_next_nullifier(ptr + 1);

        Ok(())
    }

    pub fn is_root_valid(&self, root: U256) -> ProgramResult {
        guard!(
            root == self.root,
            InvalidMerkleRoot
        );

        Ok(())
    }
}