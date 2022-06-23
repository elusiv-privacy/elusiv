use std::collections::BTreeMap;
use crate::macros::{elusiv_account, two_pow};
use solana_program::{entrypoint::ProgramResult, program_error::ProgramError::InvalidAccountData};
use crate::types::{U256, U256Limbed2};
use crate::bytes::*;
use crate::error::ElusivError::{NullifierAlreadyExists};
use super::program_account::{SizedAccount, MAX_PERMITTED_DATA_LENGTH, get_multi_accounts_count, MultiAccountAccount, HeterogenMultiAccountAccount};
use borsh::{BorshDeserialize, BorshSerialize};

/// The count of nullifiers is the count of leaves in the MT
const NULLIFIERS_COUNT: usize = two_pow!(super::MT_HEIGHT);

/// We store nullifiers with the `NullifierMap` data structure for searching and later N-SMT construction
pub type NullifierMap = BTreeMap<U256Limbed2, ()>;

const NULLIFIER_MAP_STATIC_SIZE: usize = 4; // 4 bytes to store the u32 tree map size
const MAX_NULLIFIERS_PER_ACCOUNT: usize = (MAX_PERMITTED_DATA_LENGTH as usize - NULLIFIER_MAP_STATIC_SIZE) / U256::SIZE;

pub const NULLIFIER_ACCOUNT_SUB_ACCOUNTS_COUNT: usize = get_multi_accounts_count(MAX_NULLIFIERS_PER_ACCOUNT, NULLIFIERS_COUNT);
const_assert_eq!(NULLIFIER_ACCOUNT_SUB_ACCOUNTS_COUNT, 4);

const NULLIFIER_ACCOUNT_INTERMEDIARY_ACCOUNT_SIZE: usize = NULLIFIER_MAP_STATIC_SIZE + MAX_NULLIFIERS_PER_ACCOUNT * U256::SIZE;

/// NullifierAccount is a big-array storing `NULLIFIERS_COUNT` nullifiers over multiple accounts
/// - we use `NullifierMap`s to store the nullifiers
#[elusiv_account(pda_seed = b"tree", multi_account = (
    U256;
    NULLIFIER_ACCOUNT_SUB_ACCOUNTS_COUNT;
    NULLIFIER_ACCOUNT_INTERMEDIARY_ACCOUNT_SIZE;
))]
pub struct NullifierAccount {
    bump_seed: u8,
    version: u8,
    initialized: bool,

    pubkeys: [U256; NULLIFIER_ACCOUNT_SUB_ACCOUNTS_COUNT],

    root: U256, // this value is only valid, after the active tree has been closed
    nullifiers_count: u64,
}

impl<'a, 'b, 'c> HeterogenMultiAccountAccount<'c> for NullifierAccount<'a, 'b, 'c> {
    const LAST_ACCOUNT_SIZE: usize = NULLIFIER_MAP_STATIC_SIZE + MAX_NULLIFIERS_PER_ACCOUNT * U256::SIZE;
}

/// Tree account after archiving (only a single collapsed N-SMT root)
#[elusiv_account(pda_seed = b"archived_tree")]
pub struct ArchivedTreeAccount {
    bump_seed: u8,
    version: u8,
    initialized: bool,

    commitment_root: U256,
    nullifier_root: U256,
}

impl<'a, 'b, 'c> NullifierAccount<'a, 'b, 'c> {
    /// Returns the index of the sub-account/NullifierMap that will store the next nullifier
    /// - returns `None` if there is no room for a nullifier
    fn nullifier_map_index(&self) -> Option<usize> {
        let count = u64_as_usize_safe(self.get_nullifiers_count());
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
    use borsh::BorshSerialize;
    use crate::types::U256Limbed2;
    use super::NullifierMap;
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
        let mut map = NullifierMap::new();
        map.insert(U256Limbed2([0; 2]), ());
        map.insert(U256Limbed2([1; 2]), ());
        println!("{:?}", map.try_to_vec().unwrap().len());
    }

    #[test]
    fn test_insert_nullifier() {
        panic!()
    }

    #[test]
    fn test_insert_duplicate_nullifier() {
        panic!()
    }
}