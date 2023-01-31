use super::program_account::PDAAccountData;
use crate::bytes::*;
use crate::error::ElusivError::CouldNotInsertNullifier;
use crate::macros::{elusiv_account, guard, two_pow};
use crate::map::ElusivSet;
use crate::types::{OrdU256, U256};
use elusiv_types::{ChildAccount, ParentAccount};
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

/// The count of nullifiers is the count of leaves in the MT
const NULLIFIERS_COUNT: usize = two_pow!(super::MT_HEIGHT);

/// We store nullifiers with the `NullifierMap` data structure for efficient searching and later N-SMT construction
pub type NullifierMap<'a> = ElusivSet<'a, OrdU256, NULLIFIERS_PER_ACCOUNT>;

pub const NULLIFIERS_PER_ACCOUNT: usize = two_pow!(16);
const ACCOUNTS_COUNT: usize = div_ceiling_usize(NULLIFIERS_COUNT, NULLIFIERS_PER_ACCOUNT);
#[cfg(test)]
const_assert_eq!(ACCOUNTS_COUNT, 16);

pub struct NullifierChildAccount;

impl ChildAccount for NullifierChildAccount {
    const INNER_SIZE: usize = NullifierMap::SIZE;
}

/// Account storing [`NULLIFIERS_COUNT`] nullifiers over multiple accounts
/// - we use [`NullifierMap`]s to store the nullifiers
#[elusiv_account(parent_account: { child_account_count: ACCOUNTS_COUNT, child_account: NullifierChildAccount }, eager_type: true)]
pub struct NullifierAccount {
    pda_data: PDAAccountData,
    pubkeys: [ElusivOption<Pubkey>; ACCOUNTS_COUNT],

    pub root: U256, // this value is only valid, after the active tree has been closed
    pub nullifier_hash_count: u32,
}

/// Tree account after archiving (only a single collapsed N-SMT root)
#[elusiv_account]
pub struct ArchivedTreeAccount {
    pda_data: PDAAccountData,

    commitment_root: U256,
    nullifier_root: U256,
}

impl<'a, 'b, 'c> NullifierAccount<'a, 'b, 'c> {
    pub fn can_insert_nullifier_hash(&self, nullifier_hash: U256) -> Result<bool, ProgramError> {
        let count = self.get_nullifier_hash_count();
        guard!(
            count < usize_as_u32_safe(NULLIFIERS_COUNT),
            CouldNotInsertNullifier
        );
        let nmap_index = count as usize / NULLIFIERS_PER_ACCOUNT;
        let nullifier_hash = OrdU256(nullifier_hash);

        for i in 0..=nmap_index {
            let contains = self.execute_on_child_account_mut(i, |data| {
                let mut map = NullifierMap::new(data);
                map.contains(&nullifier_hash).is_some()
            })?;

            if contains {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn try_insert_nullifier_hash(&mut self, nullifier_hash: U256) -> ProgramResult {
        let count = self.get_nullifier_hash_count();
        guard!(
            count < usize_as_u32_safe(NULLIFIERS_COUNT),
            CouldNotInsertNullifier
        );
        self.set_nullifier_hash_count(&(count + 1));
        let nullifier_hash = OrdU256(nullifier_hash);

        let mut account_index = 0;
        let mut value = Some(nullifier_hash);
        while let Some(nullifier_hash) = value {
            let insertion = self.execute_on_child_account_mut(account_index, |data| {
                NullifierMap::new(data).try_insert_default(nullifier_hash)
            })?;

            match insertion {
                Ok(Some((k, ()))) => value = Some(k),
                Ok(None) => return Ok(()),
                Err(_) => return Err(CouldNotInsertNullifier.into()),
            };

            account_index += 1;
            if account_index >= Self::COUNT {
                return Err(CouldNotInsertNullifier.into());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fields::{u256_from_str, u64_to_u256_skip_mr},
        macros::parent_account,
    };
    use assert_matches::assert_matches;

    #[test]
    fn test_can_insert_nullifier_hash() {
        parent_account!(mut nullifier_account, NullifierAccount);
        assert!(nullifier_account
            .can_insert_nullifier_hash([0; 32])
            .unwrap());

        nullifier_account
            .try_insert_nullifier_hash([0; 32])
            .unwrap();
        assert!(!nullifier_account
            .can_insert_nullifier_hash([0; 32])
            .unwrap());

        nullifier_account
            .try_insert_nullifier_hash([1; 32])
            .unwrap();
        assert!(!nullifier_account
            .can_insert_nullifier_hash([1; 32])
            .unwrap());

        assert!(nullifier_account
            .can_insert_nullifier_hash([2; 32])
            .unwrap());
    }

    #[test]
    fn test_try_insert_nullifier_hash() {
        parent_account!(mut nullifier_account, NullifierAccount);

        // Successfull insertion
        nullifier_account
            .try_insert_nullifier_hash(u256_from_str("123"))
            .unwrap();
        assert_eq!(nullifier_account.get_nullifier_hash_count(), 1);
        assert!(!nullifier_account
            .can_insert_nullifier_hash(u256_from_str("123"))
            .unwrap());

        // Full
        nullifier_account.set_nullifier_hash_count(&(NULLIFIERS_COUNT as u32 - 1));
        nullifier_account
            .try_insert_nullifier_hash(u256_from_str("0"))
            .unwrap();
        assert_matches!(
            nullifier_account.try_insert_nullifier_hash(u256_from_str("1")),
            Err(_)
        );
    }

    #[test]
    fn test_full_insertions() {
        parent_account!(mut nullifier_account, NullifierAccount);
        let count = NULLIFIERS_PER_ACCOUNT as u64 * 2 + 1;

        for i in (0..count).rev() {
            nullifier_account
                .try_insert_nullifier_hash(u64_to_u256_skip_mr(i))
                .unwrap();
        }

        for i in 0..count {
            assert!(!nullifier_account
                .can_insert_nullifier_hash(u64_to_u256_skip_mr(i))
                .unwrap());
        }
    }

    #[test]
    #[ignore]
    fn test_full_insertions_max() {
        parent_account!(mut nullifier_account, NullifierAccount);
        let count = NULLIFIERS_COUNT as u64;

        for i in (0..count).rev() {
            match nullifier_account.try_insert_nullifier_hash(u64_to_u256_skip_mr(i)) {
                Ok(_) => {}
                Err(_) => panic!("{}", i),
            }
        }

        //for i in 0..count {
        //assert!(!nullifier_account.can_insert_nullifier_hash(u64_to_u256_skip_mr(i)).unwrap());
        //}

        assert_matches!(
            nullifier_account.try_insert_nullifier_hash(u64_to_u256_skip_mr(count)),
            Err(_)
        );
    }
}
