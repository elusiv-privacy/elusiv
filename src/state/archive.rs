use crate::macros::{ ElusivAccount, remove_original_implementation, guard };
use solana_program::entrypoint::ProgramResult;
use crate::types::U256;
use crate::bytes::{find, contains};
use crate::state::StorageAccount;
use crate::error::ElusivError::{
    UnableToArchiveNullifierAccount,
    InvalidNullifierAccount,
};

const NULLIFIER_ACCOUNTS_COUNT: usize = 312500;

#[derive(ElusivAccount)]
#[remove_original_implementation]
struct ArchiveAccount {
    nullifier_accounts: [U256; NULLIFIER_ACCOUNTS_COUNT],
    roots: [U256; NULLIFIER_ACCOUNTS_COUNT],
    next_account: u64,
}

impl<'a> ArchiveAccount<'a> {
    crate::macros::pubkey!("CYFkyPAmHjayCwhRS6LpQjY2E7atNeLS3b8FE1HTYQY4");

    pub fn archive_nullifier_account(&mut self, account: U256, root: U256) -> ProgramResult {
        let ptr = self.get_next_account();

        guard!(
            ptr < NULLIFIER_ACCOUNTS_COUNT as u64,
            UnableToArchiveNullifierAccount
        );

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
        match storage_account.get_nullifier_account() {
            Some(nullifier_account) => {
                if nullifier_account == account {
                    return Ok(())
                }
            },
            None => {}
        }

        // Archived
        guard!(
            contains(account, self.nullifier_accounts),
            InvalidNullifierAccount
        );

        Ok(())
    }
}