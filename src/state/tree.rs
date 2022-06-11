use std::collections::BTreeMap;
use crate::macros::{elusiv_account};
use solana_program::{entrypoint::ProgramResult, program_error::ProgramError::InvalidAccountData};
use crate::types::{U256, U256Limbed2};
use crate::bytes::*;
use crate::error::ElusivError::{NullifierAlreadyExists};
use super::program_account::{SizedAccount, MAX_PERMITTED_DATA_LENGTH, get_multi_accounts_count, MultiAccountAccount};
use borsh::{BorshDeserialize, BorshSerialize};

/// The count of nullifiers is the count of leafes in the MT
const NULLIFIERS_COUNT: usize = 2usize.pow(super::MT_HEIGHT as u32);

/// We store nullifiers with the `NullifierMap` data structure for seaching and later N-SMT construction
pub type NullifierMap = BTreeMap<U256Limbed2, ()>;

const NULLIFIER_MAP_STATIC_SIZE: usize = 4; // 4 bytes to store the u32 tree map size
const NULLIFIER_MAP_ELEMENT_SIZE: usize = U256::SIZE + 1;
const MAX_NULLIFIERS_PER_ACCOUNT: usize = (MAX_PERMITTED_DATA_LENGTH as usize - NULLIFIER_MAP_STATIC_SIZE) / NULLIFIER_MAP_ELEMENT_SIZE;

const NULLIFIER_ACCOUNT_SUB_ACCOUNTS_COUNT: usize = get_multi_accounts_count(MAX_NULLIFIERS_PER_ACCOUNT, NULLIFIERS_COUNT);
const_assert_eq!(NULLIFIER_ACCOUNT_SUB_ACCOUNTS_COUNT, 4);

const NULLIFIER_ACCOUNT_INTERMEDIARY_ACCOUNT_SIZE: usize = NULLIFIER_MAP_STATIC_SIZE + MAX_NULLIFIERS_PER_ACCOUNT * NULLIFIER_MAP_ELEMENT_SIZE;
//const NULLIFIER_ACCOUNT_LAST_ACCOUNT_SIZE: usize = NULLIFIER_MAP_STATIC_SIZE + MAX_NULLIFIERS_PER_ACCOUNT * NULLIFIER_MAP_ELEMENT_SIZE;

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
    /// Returns the index of the sub-account/NullifierMap that will store the next nullifier
    /// - returns `None` if there is no room for a nullifier
    fn nullifier_map_index(&self) -> Option<usize> {
        let count = self.get_nullifiers_count() as usize;
        if count >= NULLIFIERS_COUNT { return None }
        Some(count / MAX_NULLIFIERS_PER_ACCOUNT)
    }

    pub fn can_insert_nullifier_hash(&self, nullifier: U256) -> bool {
        if let Some(nmap_index) = self.nullifier_map_index() {
            let repr = U256Limbed2::from(nullifier);
            for i in 0..nmap_index {
                let account = self.get_account(i);
                let mut data = &account.data.borrow()[..];
                let map = NullifierMap::deserialize(&mut data).unwrap();

                if map.contains_key(&repr) { return true }
            }
            return true
        }
        false
    }

    pub fn insert_nullifier_hash(&mut self, nullifier: U256) -> ProgramResult {
        let account_index = match self.nullifier_map_index() {
            Some(i) => i,
            None => return Err(NullifierAlreadyExists.into())
        };

        // TODO: check if can be inserted?
        let count = self.get_nullifiers_count();
        self.set_nullifiers_count(&(count + 1));

        let account = self.get_account(account_index);
        let mut data = &account.data.borrow()[..];
        let mut map = NullifierMap::deserialize(&mut data).unwrap();
        map.insert(U256Limbed2::from(nullifier), ());
        let new_data = map.try_to_vec().unwrap();

        account.serialize_data(&new_data).or(Err(InvalidAccountData))
    }
}

#[cfg(test)]
mod tests {
    /*use super::*;
    use crate::macros::account;
    use solana_program::{account_info::AccountInfo, pubkey::Pubkey};
    use crate::state::{program_account::{MultiAccountAccount, MultiAccountProgramAccount}};
    use std::collections::BTreeMap;*/

    #[allow(unused_macros)]
    macro_rules! nullifier_account {
        ($id: ident) => {
            let mut pks = Vec::new();
            for _ in 0..NullifierAccount::COUNT { pks.push(Pubkey::new_unique()); }

            account!(a0, pks[0], vec![0; NULLIFIER_ACCOUNT_INTERMEDIARY_ACCOUNT_SIZE]);
            account!(a1, pks[1], vec![0; NULLIFIER_ACCOUNT_INTERMEDIARY_ACCOUNT_SIZE]);
            account!(a2, pks[2], vec![0; NULLIFIER_ACCOUNT_INTERMEDIARY_ACCOUNT_SIZE]);
            account!(a3, pks[3], vec![0; NULLIFIER_ACCOUNT_LAST_ACCOUNT_SIZE]);

            let accounts = [a0, a1, a2, a3]; 
            let mut data = vec![0; NullifierAccount::SIZE];
            let $id = NullifierAccount::new(&mut data, &accounts[..]).unwrap();
        };
    }

    #[test]
    fn test_nullifier_map_index() {
        //nullifier_account!(nullifier_account);
    }

    #[test]
    fn test_insert_nullifier() {

    }

    #[test]
    fn test_insert_duplicate_nullifier() {

    }
}