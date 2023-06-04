use super::program_account::PDAAccountData;
use super::storage::MT_HEIGHT;
use crate::bytes::*;
use crate::error::ElusivError;
use crate::macros::{elusiv_account, guard, two_pow};
use crate::map::ElusivSet;
use crate::types::{OrdU256, JOIN_SPLIT_MAX_N_ARITY, U256};
use elusiv_types::{ChildAccount, ParentAccount};
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

/// The count of nullifiers is the count of leaves in the MT
const NULLIFIERS_COUNT: usize = two_pow!(MT_HEIGHT);

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
///
/// # Note
///
/// We use [`NullifierMap`]s to store the nullifiers.
#[elusiv_account(parent_account: { child_account_count: ACCOUNTS_COUNT, child_account: NullifierChildAccount }, eager_type: true)]
pub struct NullifierAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,
    pubkeys: [ElusivOption<Pubkey>; ACCOUNTS_COUNT],

    pub root: U256, // this value is only valid, after the active tree has been closed
    pub nullifier_hash_count: u32,

    pub max_values: [ElusivOption<U256>; ACCOUNTS_COUNT],

    moved_values_count: u8,
    moved_values: [U256; JOIN_SPLIT_MAX_N_ARITY],
    moved_values_target: [u8; JOIN_SPLIT_MAX_N_ARITY],
}

/// Tree account after archiving (only a single collapsed N-SMT root)
#[elusiv_account]
pub struct ArchivedNullifierAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,

    commitment_root: U256,
    nullifier_root: U256,
}

impl<'a, 'b, 'c> NullifierAccount<'a, 'b, 'c> {
    pub fn can_insert_nullifier_hash(&self, nullifier_hash: U256) -> Result<bool, ProgramError> {
        let count = self.get_nullifier_hash_count();
        guard!(
            (count as usize) < NULLIFIERS_COUNT,
            ElusivError::CouldNotInsertNullifier
        );

        let account_index = self.find_child_account_index(&nullifier_hash);
        let nullifier_hash = OrdU256(nullifier_hash);

        let moved_values = self.get_all_moved_values();
        if moved_values
            .iter()
            .any(|(value, _)| *value == nullifier_hash)
        {
            return Ok(false);
        }

        let contains = self.execute_on_child_account_mut(account_index, |data| {
            let mut map = NullifierMap::new(data);
            map.contains(&nullifier_hash).is_some()
        })?;

        Ok(!contains)
    }

    pub fn try_insert_nullifier_hash(&mut self, nullifier_hash: U256) -> ProgramResult {
        let count = self.get_nullifier_hash_count();
        guard!(
            (count as usize) < NULLIFIERS_COUNT,
            ElusivError::CouldNotInsertNullifier
        );

        let account_index = self.find_child_account_index(&nullifier_hash);
        let mut nullifier_hash = OrdU256(nullifier_hash);

        // `moved_values` contains all nullifier-hashes that need to be moved to other maps due to previous insertions
        let mut moved_values = self.get_all_moved_values();
        let mut moved_values_modified = false;
        guard!(
            !moved_values
                .iter()
                .any(|(value, _)| *value == nullifier_hash),
            ElusivError::CouldNotInsertNullifier
        );

        // If for the target account the nullifier-hash is smaller than a moved value, we swap both (in order to always only insert minimum values)
        if let Some(i) = moved_values.iter().position(|(value, target)| {
            *target as usize == account_index && nullifier_hash < *value
        }) {
            std::mem::swap(&mut moved_values[i].0, &mut nullifier_hash);
            moved_values_modified = true;
        }

        // Insert the nullifier-hash into the correct map account
        let (insertion, max) = self.execute_on_child_account_mut(account_index, |data| {
            let mut map = NullifierMap::new(data);
            let res = map
                .try_insert_default(nullifier_hash)
                .map_err(|_| ElusivError::CouldNotInsertNullifier);

            (res, map.max())
        })?;

        if let Some((moved_value, _)) = insertion? {
            // The ousted max value becomes a 'moved value' that will be inserted in another map
            let target = account_index as u8 + 1;
            moved_values.push((moved_value, target));
            moved_values_modified = true;
        };

        // Inc `nullifier_hash_count` and update the maximum value for the modified map account
        self.set_nullifier_hash_count(&count.checked_add(1).unwrap());
        self.set_max_values(account_index, &ElusivOption::Some(max.0));

        if moved_values_modified {
            Self::sort_all_moved_values(&mut moved_values);
            self.set_all_moved_values(&moved_values);
        }

        Ok(())
    }

    pub fn move_nullifier_hashes_to_next_account(&mut self) -> ProgramResult {
        let moved_values = self.get_all_moved_values();
        guard!(
            !moved_values.is_empty(),
            ElusivError::CouldNotInsertNullifier
        );

        // Finds the smallest target to insert all corresponding moved values into
        let target = moved_values
            .iter()
            .fold(u8::MAX, |min, (_, t)| std::cmp::min(min, *t));

        let (values, mut moved_values): (_, Vec<_>) =
            moved_values.into_iter().partition(|(_, t)| *t == target);

        // Insert all values (as mins), large to small into the map
        let (max_values, max) = self.execute_on_child_account_mut(target as usize, |data| {
            let mut map = NullifierMap::new(data);
            let mut max_values = Vec::new();
            for (v, _) in values {
                let res = map
                    .try_insert_default(v)
                    .map_err(|_| ElusivError::CouldNotInsertNullifier)?;

                if let Some((moved_value, _)) = res {
                    max_values.push(moved_value);
                }
            }

            Ok::<(_, _), ElusivError>((max_values, map.max()))
        })??;

        // Update the maximum value for the modified map account
        self.set_max_values(target as usize, &ElusivOption::Some(max.0));

        if !max_values.is_empty() {
            // The ousted max values become 'moved values' which will be inserted in another map
            let target = target.checked_add(1).unwrap();
            moved_values.extend(max_values.into_iter().map(|v| (v, target)));
            Self::sort_all_moved_values(&mut moved_values);
        }

        self.set_all_moved_values(&moved_values);

        Ok(())
    }

    fn get_all_moved_values(&self) -> Vec<(OrdU256, u8)> {
        let count = self.get_moved_values_count() as usize;
        (0..count)
            .map(|i| {
                (
                    OrdU256(self.get_moved_values(i)),
                    self.get_moved_values_target(i),
                )
            })
            .collect()
    }

    fn set_all_moved_values(&mut self, moved_values: &[(OrdU256, u8)]) {
        assert!(moved_values.len() <= JOIN_SPLIT_MAX_N_ARITY);

        self.set_moved_values_count(&(moved_values.len().try_into().unwrap()));
        for (i, (OrdU256(value), target)) in moved_values.iter().enumerate() {
            self.set_moved_values(i, value);
            self.set_moved_values_target(i, target);
        }
    }

    pub fn is_moved_nullifier_empty(&self) -> bool {
        self.get_moved_values_count() == 0
    }

    /// Sorts the provided values from large to small
    fn sort_all_moved_values(moved_values: &mut [(OrdU256, u8)]) {
        moved_values.sort_by(|(a, _), (b, _)| b.cmp(a));
    }

    pub fn find_child_account_index(&self, nullifier_hash: &U256) -> usize {
        let full_accounts_count = self.get_nullifier_hash_count() as usize / NULLIFIERS_PER_ACCOUNT;
        for i in 0..full_accounts_count {
            if OrdU256(*nullifier_hash) <= OrdU256(self.get_max_values(i).option().unwrap()) {
                return i;
            }
        }

        full_accounts_count
    }

    #[cfg(feature = "elusiv-client")]
    pub fn number_of_movement_instructions(&self, nullifier_hashes: &[U256]) -> usize {
        let count = self.get_nullifier_hash_count() as usize;

        let mut buckets = vec![false; ACCOUNTS_COUNT];
        for (i, nullifier_hash) in nullifier_hashes.iter().enumerate() {
            let full_accounts_count = (count + i) / NULLIFIERS_PER_ACCOUNT;
            let account_index = self.find_child_account_index(nullifier_hash);
            if account_index < full_accounts_count {
                #[allow(clippy::needless_range_loop)]
                for i in account_index..Self::COUNT {
                    if i < full_accounts_count || i == account_index {
                        buckets[i] = true;
                    }
                }
            }
        }

        buckets
            .iter()
            .fold(0, |acc, x| if *x { acc + 1 } else { acc })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fields::{u256_from_str, u64_to_u256, u64_to_u256_skip_mr},
        macros::parent_account,
    };

    #[test]
    fn test_can_insert_nullifier_hash() {
        parent_account!(mut nullifier_account, NullifierAccount);

        let a = [0; 32];
        assert!(nullifier_account.can_insert_nullifier_hash(a).unwrap());
        nullifier_account.try_insert_nullifier_hash(a).unwrap();
        assert!(!nullifier_account.can_insert_nullifier_hash(a).unwrap());

        let b = [1; 32];
        nullifier_account.try_insert_nullifier_hash(b).unwrap();
        assert!(!nullifier_account.can_insert_nullifier_hash(b).unwrap());

        let c = [2; 32];
        assert!(nullifier_account.can_insert_nullifier_hash(c).unwrap());
    }

    #[test]
    fn test_can_insert_nullifier_hash_moved_values() {
        parent_account!(mut nullifier_account, NullifierAccount);

        let a = [0; 32];
        nullifier_account.set_all_moved_values(&[(OrdU256(a), 0)]);
        assert!(!nullifier_account.can_insert_nullifier_hash(a).unwrap());

        for i in 0..NULLIFIERS_PER_ACCOUNT as u64 {
            nullifier_account
                .try_insert_nullifier_hash(u64_to_u256_skip_mr(i + 1))
                .unwrap();
        }

        let b = [1; 32];
        nullifier_account.set_all_moved_values(&[(OrdU256(b), 1)]);
        assert!(nullifier_account.can_insert_nullifier_hash(a).unwrap());
        assert!(!nullifier_account.can_insert_nullifier_hash(b).unwrap());
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

        assert_eq!(
            nullifier_account.try_insert_nullifier_hash(u256_from_str("1")),
            Err(ElusivError::CouldNotInsertNullifier.into())
        );
    }

    #[test]
    fn test_try_insert_nullifier_hash_moved_values() {
        parent_account!(mut nullifier_account, NullifierAccount);

        // Value cannot be inserted since it's contained in the moved values
        nullifier_account.set_all_moved_values(&[(OrdU256(u256_from_str("2")), 0)]);
        assert_eq!(
            nullifier_account.try_insert_nullifier_hash(u256_from_str("2")),
            Err(ElusivError::CouldNotInsertNullifier.into())
        );

        // Value now can be inserted
        nullifier_account.set_all_moved_values(&[]);
        nullifier_account
            .try_insert_nullifier_hash(u256_from_str("2"))
            .unwrap();

        for i in 0..NULLIFIERS_PER_ACCOUNT as u64 {
            nullifier_account
                .try_insert_nullifier_hash(u64_to_u256_skip_mr(i))
                .unwrap();
        }

        // Moved value now linked to second child account
        nullifier_account.set_all_moved_values(&[(OrdU256(u256_from_str("3")), 1)]);
        assert_eq!(
            nullifier_account.try_insert_nullifier_hash(u256_from_str("3")),
            Err(ElusivError::CouldNotInsertNullifier.into())
        );

        nullifier_account.set_all_moved_values(&[]);
        nullifier_account
            .try_insert_nullifier_hash(u256_from_str("3"))
            .unwrap();
    }

    #[test]
    fn test_full_insertions() {
        parent_account!(mut nullifier_account, NullifierAccount);
        let count = NULLIFIERS_PER_ACCOUNT as u64;

        for i in (0..count).rev() {
            nullifier_account
                .try_insert_nullifier_hash(u64_to_u256(i))
                .unwrap();
        }

        for i in 0..count {
            assert!(!nullifier_account
                .can_insert_nullifier_hash(u64_to_u256(i))
                .unwrap());
        }
    }

    #[test]
    #[should_panic]
    fn test_full_insertions2() {
        parent_account!(mut nullifier_account, NullifierAccount);
        let count = NULLIFIERS_PER_ACCOUNT as u64 * 2 + 1;

        for i in (0..count).rev() {
            nullifier_account
                .try_insert_nullifier_hash(u64_to_u256(i))
                .unwrap();
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

        assert_eq!(
            nullifier_account.try_insert_nullifier_hash(u64_to_u256_skip_mr(count)),
            Err(ElusivError::CouldNotInsertNullifier.into())
        );
    }

    #[test]
    fn test_find_child_account_index() {
        parent_account!(mut nullifier_account, NullifierAccount);
        let count = NULLIFIERS_PER_ACCOUNT as u64;

        for i in 0..count {
            assert_eq!(
                nullifier_account.find_child_account_index(&u64_to_u256_skip_mr(count)),
                0
            );

            nullifier_account
                .try_insert_nullifier_hash(u64_to_u256_skip_mr(i))
                .unwrap();
        }

        // Equality to the max value
        assert_eq!(
            nullifier_account.find_child_account_index(&u64_to_u256_skip_mr(count - 1)),
            0
        );

        // Larger than max value
        assert_eq!(
            nullifier_account.find_child_account_index(&u64_to_u256_skip_mr(count)),
            1
        );

        // Less than max value
        assert_eq!(nullifier_account.find_child_account_index(&[0; 32]), 0);
    }

    #[test]
    fn test_set_all_moved_values() {
        parent_account!(mut nullifier_account, NullifierAccount);

        let moved_values: Vec<_> = (0..JOIN_SPLIT_MAX_N_ARITY)
            .map(|i| (OrdU256(u64_to_u256(i as u64)), i as u8))
            .collect();
        nullifier_account.set_all_moved_values(&moved_values);
        assert_eq!(nullifier_account.get_all_moved_values(), moved_values);
    }

    #[test]
    #[should_panic]
    fn test_set_all_moved_values_invalid_length() {
        parent_account!(mut nullifier_account, NullifierAccount);

        nullifier_account.set_all_moved_values(
            &(0..JOIN_SPLIT_MAX_N_ARITY + 1)
                .map(|_| (OrdU256([0; 32]), 0))
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn test_sort_all_moved_values() {
        let v = [(OrdU256(u64_to_u256(0)), 1), (OrdU256(u64_to_u256(1)), 0)];
        let mut values = v;
        NullifierAccount::sort_all_moved_values(&mut values);

        assert_eq!(&values[..], &v.into_iter().rev().collect::<Vec<_>>()[..]);
    }

    #[test]
    fn test_number_of_movement_instructions() {
        parent_account!(mut nullifier_account, NullifierAccount);

        assert_eq!(
            nullifier_account.number_of_movement_instructions(&[[0; 32]]),
            0
        );

        for i in 0..NULLIFIERS_PER_ACCOUNT as u64 {
            nullifier_account
                .try_insert_nullifier_hash(u64_to_u256_skip_mr(i))
                .unwrap();
        }

        // Insertion into first map -> 1 movement
        assert_eq!(
            nullifier_account.number_of_movement_instructions(&[[0; 32]]),
            1
        );

        // Two insertions into first map -> 1 movement
        assert_eq!(
            nullifier_account.number_of_movement_instructions(&[[0; 32], [0; 32]]),
            1
        );

        // Insertion into second map -> no movement
        assert_eq!(
            nullifier_account.number_of_movement_instructions(&[u64_to_u256_skip_mr(
                NULLIFIERS_PER_ACCOUNT as u64
            )]),
            0
        );

        for i in 0..NULLIFIERS_PER_ACCOUNT as u64 {
            nullifier_account
                .try_insert_nullifier_hash(u64_to_u256_skip_mr(NULLIFIERS_PER_ACCOUNT as u64 + i))
                .unwrap();
        }

        // Insertion into first map -> 2 movements
        assert_eq!(
            nullifier_account.number_of_movement_instructions(&[[0; 32]]),
            2
        );

        // Insertion into third map -> 0 movements
        assert_eq!(
            nullifier_account.number_of_movement_instructions(&[u64_to_u256_skip_mr(
                NULLIFIERS_PER_ACCOUNT as u64 * 2
            )]),
            0
        );

        // Insertion into first and second map -> 2 movements
        assert_eq!(
            nullifier_account.number_of_movement_instructions(&[
                u64_to_u256_skip_mr(0),
                u64_to_u256_skip_mr(NULLIFIERS_PER_ACCOUNT as u64)
            ]),
            2
        );

        // Insertion into first and third map -> 2 movements
        assert_eq!(
            nullifier_account.number_of_movement_instructions(&[
                u64_to_u256_skip_mr(0),
                u64_to_u256_skip_mr(NULLIFIERS_PER_ACCOUNT as u64 * 2)
            ]),
            2
        );

        for i in 0..NULLIFIERS_PER_ACCOUNT as u64 {
            nullifier_account
                .try_insert_nullifier_hash(u64_to_u256_skip_mr(
                    2 * NULLIFIERS_PER_ACCOUNT as u64 + i,
                ))
                .unwrap();
        }

        // Insertion into first map -> 3 movements
        assert_eq!(
            nullifier_account.number_of_movement_instructions(&[[0; 32]]),
            3
        );

        // Insertion into fourth map -> 0 movements
        assert_eq!(
            nullifier_account.number_of_movement_instructions(&[u64_to_u256_skip_mr(
                NULLIFIERS_PER_ACCOUNT as u64 * 3
            )]),
            0
        );

        // Insertion into first map and third map -> 3 movements
        assert_eq!(
            nullifier_account.number_of_movement_instructions(&[
                u64_to_u256_skip_mr(0),
                u64_to_u256_skip_mr(NULLIFIERS_PER_ACCOUNT as u64 * 3)
            ]),
            3
        );
    }
}
