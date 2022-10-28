pub use elusiv_types::accounts::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macros::account;
    use std::collections::HashMap;
    use borsh::BorshSerialize;
    use elusiv_proc_macros::repeat;
    use elusiv_types::ElusivOption;
    use solana_program::{account_info::AccountInfo, pubkey::Pubkey, program_error::ProgramError};

    struct TestPDAAccount { }

    impl PDAAccount for TestPDAAccount {
        const PROGRAM_ID: Pubkey = crate::PROGRAM_ID;
        const SEED: &'static [u8] = b"ABC";

        #[cfg(feature = "instruction-abi")]
        const IDENT: &'static str = "TestPDAAccount";
    }

    #[test]
    fn test_pda_account() {
        assert_ne!(TestPDAAccount::find(None), TestPDAAccount::find(Some(0)));
        assert_ne!(TestPDAAccount::find(Some(0)), TestPDAAccount::find(Some(1)));
    }

    #[test]
    fn test_sub_account() {
        let mut data = vec![0; 100];
        let mut account = SubAccount::new(&mut data);

        assert!(!account.get_is_in_use());
        account.set_is_in_use(true);
        assert!(account.get_is_in_use());
        account.set_is_in_use(false);
        assert!(!account.get_is_in_use());

        assert_eq!(account.data.len(), 99);
    }

    struct TestMultiAccount<'a, 'b> {
        pub pubkeys: [ElusivOption<Pubkey>; SUB_ACCOUNT_COUNT],
        pub accounts: std::collections::HashMap<usize, &'a AccountInfo<'b>>,
    }

    impl<'a, 'b> PDAAccount for TestMultiAccount<'a, 'b> {
        const PROGRAM_ID: Pubkey = crate::PROGRAM_ID;
        const SEED: &'static [u8] = b"ABC";

        #[cfg(feature = "instruction-abi")]
        const IDENT: &'static str = "TestMultiAccount";
    }

    impl<'a, 'b> MultiAccountAccount<'b> for TestMultiAccount<'a, 'b> {
        const COUNT: usize = SUB_ACCOUNT_COUNT;
        const ACCOUNT_SIZE: usize = 2;

        unsafe fn get_account_unsafe(&self, account_index: usize) -> Result<&AccountInfo<'b>, ProgramError> {
            Ok(self.accounts[&account_index])
        }
    }

    impl<'a, 'b> TestMultiAccount<'a, 'b> {
        fn serialize(&self) -> Vec<u8> {
            let mut v = Vec::new();
            v.extend(vec![0; 3]);
            v.extend(self.pubkeys.try_to_vec().unwrap());
            v
        }
    }

    const SUB_ACCOUNT_COUNT: usize = 3;

    macro_rules! test_multi_account {
        ($accounts: ident, $pubkeys: ident) => {
            let mut accounts = HashMap::new();
            let mut pubkeys = [ElusivOption::None; SUB_ACCOUNT_COUNT];

            repeat!({
                let pk = solana_program::pubkey::Pubkey::new_unique();
                account!(account_index, pk, vec![1, 0]);
                accounts.insert(_index, &account_index);

                pubkeys[_index] = ElusivOption::Some(pk);
            }, 3);

            let $accounts = accounts;
            let $pubkeys = pubkeys;

        };

        ($id: ident) => {
            test_multi_account!(accounts, pubkeys);
            let $id = TestMultiAccount { pubkeys, accounts };
        };
        (mut $id: ident) => {
            test_multi_account!(accounts, pubkeys);
            let mut $id = TestMultiAccount { pubkeys, accounts };
        };
    }

    #[test]
    #[should_panic]
    fn test_get_account_unsafe() {
        test_multi_account!(account);
        unsafe { _ = account.get_account_unsafe(3); }
    }

    #[test]
    fn test_try_execute_on_sub_account() {
        test_multi_account!(account);

        for i in 0..SUB_ACCOUNT_COUNT {
            assert_eq!(
                account.try_execute_on_sub_account::<_, usize, ProgramError>(i, |data| {
                    data[0] = i as u8 + 1;
                    Ok(42)
                }).unwrap(),
                42
            );
        }

        for i in 0..SUB_ACCOUNT_COUNT {
            assert_eq!(account.accounts[&i].data.borrow()[1], i as u8 + 1);
        }
    }

    #[test]
    fn test_execute_on_sub_account() {
        test_multi_account!(account);

        for i in 0..SUB_ACCOUNT_COUNT {
            account.execute_on_sub_account(i, |data| {
                data[0] = i as u8 + 1;
            }).unwrap();
        }

        for i in 0..SUB_ACCOUNT_COUNT {
            assert_eq!(account.accounts[&i].data.borrow()[1], i as u8 + 1);
        }
    }

    fn test_find(
        pubkey_is_setup: [bool; SUB_ACCOUNT_COUNT],
        accounts: Vec<Option<usize>>,
        expected: Vec<usize>,
    ) {
        test_multi_account!(mut account);
        for (i, &is_setup) in pubkey_is_setup.iter().enumerate() {
            if is_setup { continue }
            account.pubkeys[i] = ElusivOption::None;
        }

        let data = account.serialize();
        let pk = solana_program::pubkey::Pubkey::new_unique();
        account!(main_account, pk, data);

        let pk = solana_program::pubkey::Pubkey::new_unique();
        account!(unused_account, pk, vec![1, 0]);

        let account_info_iter = &mut accounts.iter().map(|a| match a {
            Some(i) => account.accounts[i],
            None => &unused_account,
        });
        let len_prev = account_info_iter.len();
        let map = TestMultiAccount::find_sub_accounts::<_, TestMultiAccount, {SUB_ACCOUNT_COUNT}>(
            &main_account,
            &crate::ID,
            false,
            account_info_iter,
        ).unwrap();
        assert_eq!(len_prev, account_info_iter.len() + map.len());

        let mut keys: Vec<usize> = map.iter().map(|(&k, _)| k).collect();
        keys.sort_unstable();
        assert_eq!(keys, expected);
        for key in keys {
            assert_eq!(map[&key].key, account.accounts[&key].key);
        }
    }

    #[test]
    fn test_find_sub_accounts() {
        // All none
        test_find(
            [false, false, false],
            vec![Some(0), Some(1), Some(2)],
            vec![]
        );

        // First account set
        test_find(
            [true, false, false],
            vec![Some(0), Some(1), Some(2)],
            vec![0]
        );

        // Middle account set
        test_find(
            [false, true, false],
            vec![Some(0), Some(1), Some(2)],
            vec![1]
        );

        // Last account set
        test_find(
            [false, false, true],
            vec![Some(0), Some(1), Some(2)],
            vec![2]
        );

        // Different account at start
        test_find(
            [true, true, true],
            vec![None, Some(0), Some(1), Some(2)],
            vec![]
        );

        // Wrong order
        test_find(
            [true, true, true],
            vec![Some(2), Some(1), Some(0)],
            vec![2]
        );

        // Correct order
        test_find(
            [true, true, true],
            vec![Some(0), Some(1), Some(2)],
            vec![0, 1, 2]
        );

        // Accounts at end ignored
        test_find(
            [true, true, true],
            vec![Some(0), Some(1), Some(2), None, None],
            vec![0, 1, 2]
        );
    }
}