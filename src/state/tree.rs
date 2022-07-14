use crate::macros::{elusiv_account, two_pow, guard};
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use crate::types::{U256, U256Limbed2};
use crate::bytes::*;
use crate::error::ElusivError::NullifierAlreadyExists;
use super::program_account::{SizedAccount, PDAAccountData, MultiAccountAccountData, MultiAccountAccount, SUB_ACCOUNT_ADDITIONAL_SIZE};
use borsh::{BorshDeserialize, BorshSerialize};

/// The count of nullifiers is the count of leaves in the MT
const NULLIFIERS_COUNT: usize = two_pow!(super::MT_HEIGHT);

/// We store nullifiers with the `NullifierMap` data structure for efficient searching and later N-SMT construction
pub type NullifierMap = ElusivBTreeMap<U256Limbed2, (), NULLIFIERS_PER_ACCOUNT>;

const NULLIFIERS_PER_ACCOUNT: usize = two_pow!(18);
const ACCOUNT_SIZE: usize = NullifierMap::SIZE + SUB_ACCOUNT_ADDITIONAL_SIZE;
const ACCOUNTS_COUNT: usize = u64_as_usize_safe(div_ceiling(NULLIFIERS_COUNT as u64, NULLIFIERS_PER_ACCOUNT as u64));
const_assert_eq!(ACCOUNTS_COUNT, 4);

/// NullifierAccount is a big-array storing `NULLIFIERS_COUNT` nullifiers over multiple accounts
/// - we use `NullifierMap`s to store the nullifiers
#[elusiv_account(pda_seed = b"tree", multi_account = (U256; ACCOUNTS_COUNT; ACCOUNT_SIZE))]
pub struct NullifierAccount {
    pda_data: PDAAccountData,
    multi_account_data: MultiAccountAccountData<ACCOUNTS_COUNT>,

    root: U256, // this value is only valid, after the active tree has been closed
    nullifiers_count: u64,
}

/// Tree account after archiving (only a single collapsed N-SMT root)
#[elusiv_account(pda_seed = b"archived_tree")]
pub struct ArchivedTreeAccount {
    pda_data: PDAAccountData,

    commitment_root: U256,
    nullifier_root: U256,
}

impl<'a, 'b, 'c> NullifierAccount<'a, 'b, 'c> {
    /// Returns the index of the sub-account/NullifierMap that will store the next nullifier
    /// - returns `None` if there is no room for a nullifier
    fn nullifier_map_index(&self) -> Option<usize> {
        let count = u64_as_usize_safe(self.get_nullifiers_count());
        if count >= NULLIFIERS_COUNT { return None }
        Some(count / NULLIFIERS_PER_ACCOUNT)
    }

    pub fn can_insert_nullifier_hash(&self, nullifier: U256) -> Result<bool, ProgramError> {
        if let Some(nmap_index) = self.nullifier_map_index() {
            let repr = U256Limbed2::from(nullifier);
            for i in 0..=nmap_index {
                match self.execute_on_sub_account::<_, _, ProgramError>(i, |data| {
                    let map = NullifierMap::try_from_slice(data)?;
                    if map.contains_key(&repr) { return Ok(Some(false)) }
                    Ok(None)
                })? {
                    Some(v) => return Ok(v),
                    None => {}
                }
            }
            return Ok(true)
        }
        Ok(false)
    }

    pub fn insert_nullifier_hash(&mut self, nullifier: U256) -> ProgramResult {
        guard!(self.can_insert_nullifier_hash(nullifier)?, NullifierAlreadyExists);
        let account_index = match self.nullifier_map_index() {
            Some(i) => i,
            None => return Err(NullifierAlreadyExists.into())
        };

        let count = self.get_nullifiers_count();
        self.set_nullifiers_count(&(count + 1));

        self.execute_on_sub_account::<_, _, ProgramError>(account_index, |data| {
            let mut map = NullifierMap::try_from_slice(data)?;
            map.try_insert(U256Limbed2::from(nullifier), ())?;

            let new_data = map.try_to_vec().unwrap();
            data[..new_data.len()].copy_from_slice(&new_data[..]);

            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use solana_program::pubkey::Pubkey;
    use super::super::program_account::MultiAccountProgramAccount;

    fn turn_into_sub_account(data: &[u8]) -> Vec<u8> {
        let mut v = data.to_vec();
        v.insert(0, 1);
        v
    }

    #[allow(unused_macros)]
    macro_rules! nullifier_account {
        (acc $accounts: ident) => {
            nullifier_account!(acc $accounts, [
                vec![0; NullifierAccount::ACCOUNT_SIZE],
                vec![0; NullifierAccount::ACCOUNT_SIZE],
                vec![0; NullifierAccount::ACCOUNT_SIZE],
                vec![0; NullifierAccount::ACCOUNT_SIZE],
            ])
        };
        (acc $accounts: ident, $data: expr) => {
            let mut pks = Vec::new();
            for _ in 0..NullifierAccount::COUNT { pks.push(Pubkey::new_unique()); }

            crate::macros::account!(a0, pks[0], $data[0].clone());
            crate::macros::account!(a1, pks[1], $data[1].clone());
            crate::macros::account!(a2, pks[2], $data[2].clone());
            crate::macros::account!(a3, pks[3], $data[3].clone());

            let mut $accounts = std::collections::HashMap::new();
            
            $accounts.insert(0, &a0);
            $accounts.insert(1, &a1);
            $accounts.insert(2, &a2);
            $accounts.insert(3, &a3);
        };

        ($id: ident) => {
            nullifier_account!(acc accounts);
            let mut data = vec![0; NullifierAccount::SIZE];
            let $id = NullifierAccount::new(&mut data, accounts).unwrap();
        };
        (mut $id: ident, $data: expr) => {
            nullifier_account!(acc accounts, [
                turn_into_sub_account(&$data[0].try_to_vec().unwrap()),
                turn_into_sub_account(&$data[1].try_to_vec().unwrap()),
                turn_into_sub_account(&$data[2].try_to_vec().unwrap()),
                turn_into_sub_account(&$data[3].try_to_vec().unwrap()),
            ]);
            let mut data = vec![0; NullifierAccount::SIZE];
            let mut $id = NullifierAccount::new(&mut data, accounts).unwrap();
        };
        (mut $id: ident) => {
            nullifier_account!(acc accounts);
            let mut data = vec![0; NullifierAccount::SIZE];
            let mut $id = NullifierAccount::new(&mut data, accounts).unwrap();
        };
    }

    #[test]
    fn test_nullifier_map_index() {
        nullifier_account!(mut nullifier_account);
        assert_eq!(nullifier_account.nullifier_map_index().unwrap(), 0);

        nullifier_account.set_nullifiers_count(&(NULLIFIERS_PER_ACCOUNT as u64));
        assert_eq!(nullifier_account.nullifier_map_index().unwrap(), 1);

        nullifier_account.set_nullifiers_count(&(2 * NULLIFIERS_PER_ACCOUNT as u64));
        assert_eq!(nullifier_account.nullifier_map_index().unwrap(), 2);

        nullifier_account.set_nullifiers_count(&(3 * NULLIFIERS_PER_ACCOUNT as u64));
        assert_eq!(nullifier_account.nullifier_map_index().unwrap(), 3);

        nullifier_account.set_nullifiers_count(&(NULLIFIERS_COUNT as u64));
        assert_matches!(nullifier_account.nullifier_map_index(), None);
    }

    #[test]
    fn test_can_insert_nullifier_hash() {
        nullifier_account!(nullifier_account);
        assert!(nullifier_account.can_insert_nullifier_hash([0; 32]).unwrap());

        let mut map = NullifierMap::new();
        map.try_insert([0; 32].into(), ()).unwrap();

        let mut map1 = NullifierMap::new();
        map1.try_insert([1; 32].into(), ()).unwrap();
        map1.try_insert([2; 32].into(), ()).unwrap();

        nullifier_account!(mut nullifier_account, [
            map.clone(),
            map1.clone(),
            NullifierMap::new(),
            NullifierMap::new(),
        ]);
        nullifier_account.set_nullifiers_count(&(2 * NULLIFIERS_PER_ACCOUNT as u64));
        assert!(!nullifier_account.can_insert_nullifier_hash([0; 32]).unwrap());
        assert!(!nullifier_account.can_insert_nullifier_hash([1; 32]).unwrap());
        assert!(!nullifier_account.can_insert_nullifier_hash([2; 32]).unwrap());
        assert!(nullifier_account.can_insert_nullifier_hash([3; 32]).unwrap());
    }

    #[test]
    fn test_insert_nullifier() {
        nullifier_account!(mut nullifier_account);
        nullifier_account.insert_nullifier_hash([0; 32]).unwrap();
        assert_eq!(nullifier_account.get_nullifiers_count(), 1);
    }
}