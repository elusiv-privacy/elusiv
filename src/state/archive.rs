use elusiv_account::{ ElusivAccount, remove_original_implementation };
use solana_program::entrypoint::ProgramResult;
use crate::types::U256;
use crate::bytes::{find, contains};
use crate::state::StorageAccount;

const NULLIFIER_ACCOUNTS_COUNT: usize = 312500;

#[derive(ElusivAccount)]
#[remove_original_implementation]
struct ArchiveAccount {
    nullifier_accounts: [U256; NULLIFIER_ACCOUNTS_COUNT],
    roots: [U256; NULLIFIER_ACCOUNTS_COUNT],
    next_account: u64,
}

impl<'a> ArchiveAccount<'a> {
    elusiv_account::pubkey!("CYFkyPAmHjayCwhRS6LpQjY2E7atNeLS3b8FE1HTYQY4");

    pub fn archive_nullifier_account(&mut self, account: U256, root: U256) -> ProgramResult {
        let ptr = self.get_next_account();

        if ptr >= NULLIFIER_ACCOUNTS_COUNT as u64 {
            return Err(crate::error::ElusivError::UnableToArchiveNullifierAccount.into());
        }

        self.set_nullifier_accounts(ptr as usize, &account);
        self.set_roots(ptr as usize, &root);

        self.set_next_account(ptr + 1);

        Ok(())
    }

    pub fn find_account_with_root(&self, root: U256) -> Option<(usize, U256)> {
        let result = find(root, self.roots);

        match result {
            Some(index) => Some((index, self.get_nullifier_accounts(index))),
            None => None
        }
    }

    pub fn is_nullifier_account_valid(
        &self,
        storage_account: &StorageAccount,
        account: U256
    ) -> ProgramResult {
        // Active nullifier account
        if account == storage_account.get_nullifier_account().expect("No active nullifier account") {
            return Ok(())
        }

        // Archived
        if contains(account, self.nullifier_accounts) {
            return Ok(())
        }

        return Err(crate::error::ElusivError::InvalidNullifierAccount.into())
    }
}