use crate::macros::{elusiv_account, two_pow, guard};
use crate::map::{ElusivSet, ElusivMapError};
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use crate::types::U256;
use crate::bytes::*;
use crate::error::ElusivError::{CouldNotInsertNullifier};
use super::program_account::{SizedAccount, PDAAccountData, MultiAccountAccountData, MultiAccountAccount, SUB_ACCOUNT_ADDITIONAL_SIZE};
use borsh::{BorshDeserialize, BorshSerialize};

/// The count of nullifiers is the count of leaves in the MT
const NULLIFIERS_COUNT: usize = two_pow!(super::MT_HEIGHT);

/// We store nullifiers with the `NullifierMap` data structure for efficient searching and later N-SMT construction
pub type NullifierMap<'a> = ElusivSet<'a, U256, NULLIFIERS_PER_ACCOUNT>;

const NULLIFIERS_PER_ACCOUNT: usize = two_pow!(16);
const ACCOUNT_SIZE: usize = NullifierMap::SIZE + SUB_ACCOUNT_ADDITIONAL_SIZE;
const ACCOUNTS_COUNT: usize = u64_as_usize_safe(div_ceiling(NULLIFIERS_COUNT as u64, NULLIFIERS_PER_ACCOUNT as u64));
const_assert_eq!(ACCOUNTS_COUNT, 16);

/// NullifierAccount is a big-array storing `NULLIFIERS_COUNT` nullifiers over multiple accounts
/// - we use `NullifierMap`s to store the nullifiers
#[elusiv_account(pda_seed = b"nullifier", multi_account = (ACCOUNTS_COUNT; ACCOUNT_SIZE))]
pub struct NullifierAccount {
    pda_data: PDAAccountData,
    multi_account_data: MultiAccountAccountData<ACCOUNTS_COUNT>,

    root: U256, // this value is only valid, after the active tree has been closed
    nullifier_hash_count: u64,
}

/// Tree account after archiving (only a single collapsed N-SMT root)
#[elusiv_account(pda_seed = b"archived")]
pub struct ArchivedTreeAccount {
    pda_data: PDAAccountData,

    commitment_root: U256,
    nullifier_root: U256,
}

impl<'a, 'b, 'c> NullifierAccount<'a, 'b, 'c> {
    /// Returns the index of the sub-account/NullifierMap that will store the next nullifier_hash
    /// - returns `None` if there is no room for a nullifier_hash
    fn nullifier_map_index(&self) -> Option<usize> {
        let count = u64_as_usize_safe(self.get_nullifier_hash_count());
        if count >= NULLIFIERS_COUNT { return None }
        Some(count / NULLIFIERS_PER_ACCOUNT)
    }

    pub fn can_insert_nullifier_hash(&self, nullifier_hash: U256) -> Result<bool, ProgramError> {
        let count = self.get_nullifier_hash_count();
        guard!(count < NULLIFIERS_COUNT as u64, CouldNotInsertNullifier);

        if let Some(nmap_index) = self.nullifier_map_index() {
            let repr = nullifier_hash;
            for i in 0..=nmap_index {
                let contains = self.try_execute_on_sub_account::<_, _, ProgramError>(i, |data| {
                    let mut map = NullifierMap::new(data);
                    let result = map.contains(&repr).is_some();
                    Ok(result)
                })?;

                if contains { return Ok(false) }
            }
            return Ok(true)
        }
        Ok(false)
    }

    pub fn try_insert_nullifier_hash(&mut self, nullifier_hash: U256) -> ProgramResult {
        let count = self.get_nullifier_hash_count();
        guard!(count < NULLIFIERS_COUNT as u64, CouldNotInsertNullifier);
        self.set_nullifier_hash_count(&(count + 1));

        let mut account_index = 0;
        let mut value = Some(nullifier_hash);
        while value.is_some() {
            let insertion = self.try_execute_on_sub_account::<_, Option<(U256, ())>, ElusivMapError<()>>(account_index, |data| {
                NullifierMap::new(data).try_insert_default(nullifier_hash)
            });

            value = match insertion {
                Ok(None) => None,
                Ok(Some((k, _))) => Some(k),
                Err(_) => return Err(CouldNotInsertNullifier.into()),
            };

            account_index += 1;
            if account_index >= Self::COUNT {
                return Err(CouldNotInsertNullifier.into())
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use super::super::program_account::MultiAccountProgramAccount;
    use crate::{fields::u256_from_str, macros::nullifier_account};

    #[test]
    fn test_nullifier_map_index() {
        nullifier_account!(mut nullifier_account);
        for i in 1..=NullifierAccount::COUNT {
            assert_eq!(nullifier_account.nullifier_map_index().unwrap(), i - 1);
            nullifier_account.set_nullifier_hash_count(&((i * NULLIFIERS_PER_ACCOUNT) as u64));
        }
        nullifier_account.set_nullifier_hash_count(&(NULLIFIERS_COUNT as u64 + 1));
        assert!(nullifier_account.nullifier_map_index().is_none());
    }

    #[test]
    fn test_can_insert_nullifier_hash() {
        nullifier_account!(mut nullifier_account);
        assert!(nullifier_account.can_insert_nullifier_hash([0; 32]).unwrap());

        nullifier_account.try_insert_nullifier_hash([0; 32]).unwrap();
        assert!(!nullifier_account.can_insert_nullifier_hash([0; 32]).unwrap());

        nullifier_account.try_insert_nullifier_hash([1; 32]).unwrap();
        assert!(!nullifier_account.can_insert_nullifier_hash([1; 32]).unwrap());

        assert!(nullifier_account.can_insert_nullifier_hash([2; 32]).unwrap());
    }

    #[test]
    fn test_try_insert_nullifier_hash() {
        // Note: true fuctional test only with integration tests

        nullifier_account!(mut nullifier_account);

        // Successfull insertion
        nullifier_account.try_insert_nullifier_hash(u256_from_str("123")).unwrap();
        assert_eq!(nullifier_account.get_nullifier_hash_count(), 1);
        assert!(!nullifier_account.can_insert_nullifier_hash(u256_from_str("123")).unwrap());

        // Full
        nullifier_account.set_nullifier_hash_count(&(NULLIFIERS_COUNT as u64 - 1));
        nullifier_account.try_insert_nullifier_hash(u256_from_str("0")).unwrap();
        assert_matches!(nullifier_account.try_insert_nullifier_hash(u256_from_str("1")), Err(_));
    }
}