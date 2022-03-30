use crate::macros::{ ElusivAccount, remove_original_implementation };
use solana_program::entrypoint::ProgramResult;
use crate::types::U256;
use crate::bytes::contains;
use crate::error::ElusivError;

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
        if self.get_next_nullifier() >= NULLIFIERS_COUNT as u64 { return Err(ElusivError::NullifierAlreadyUsed.into()); }

        // Check that nullifier does not already exist
        if contains(nullifier, &self.nullifiers) {
            return Err(ElusivError::NoRoomForNullifier.into());
        }

        Ok(())
    }

    pub fn insert_nullifier(&mut self, nullifier: U256) -> ProgramResult {
        let ptr = self.get_next_nullifier();
        if ptr >= NULLIFIERS_COUNT as u64 { return Err(ElusivError::NullifierAlreadyUsed.into()); }

        // Save nullifier
        self.set_nullifiers(ptr as usize, &nullifier);

        // Increment pointer
        self.set_next_nullifier(ptr + 1);

        Ok(())
    }

    pub fn is_root_valid(&self, root: U256) -> ProgramResult {
        if root == self.root {
            return Ok(());
        }

        Err(ElusivError::InvalidMerkleRoot.into())
    }
}