pub use elusiv_types::accounts::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macros::{account_info, parent_account};
    use borsh::BorshDeserialize;
    use elusiv_types::{split_child_account_data, BorshSerDeSized, ElusivOption};
    use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

    struct TestPDAAccount;

    impl PDAAccount for TestPDAAccount {
        const PROGRAM_ID: Pubkey = crate::PROGRAM_ID;
        const SEED: &'static [u8] = b"ABC";
        const FIRST_PDA: (Pubkey, u8) = (Pubkey::new_from_array([0; 32]), 123);

        #[cfg(feature = "elusiv-client")]
        const IDENT: &'static str = "TestPDAAccount";
    }

    #[test]
    fn test_pda_account() {
        assert_ne!(TestPDAAccount::find(None), TestPDAAccount::find(Some(0)));
        assert_ne!(TestPDAAccount::find(Some(0)), TestPDAAccount::find(Some(1)));

        assert_eq!(
            TestPDAAccount::find(None),
            (Pubkey::new_from_array([0; 32]), 123)
        );
        //assert_eq!(TestPDAAccount::find(None).0, Pubkey::find_program_address(&[TestPDAAccount::SEED], &crate::PROGRAM_ID).0);
    }

    struct TestChildAccount;

    impl ChildAccount for TestChildAccount {
        const INNER_SIZE: usize = 123;
    }

    #[test]
    fn test_child_account_size() {
        assert_eq!(
            TestChildAccount::SIZE,
            TestChildAccount::INNER_SIZE + ChildAccountConfig::SIZE
        );
    }

    #[test]
    fn test_child_account() {
        let data = vec![0; TestChildAccount::SIZE];
        let (config, inner_data) = split_child_account_data(&data).unwrap();
        let config = ChildAccountConfig::try_from_slice(config).unwrap();

        assert!(!config.is_in_use);
        assert_eq!(inner_data.len(), TestChildAccount::INNER_SIZE);
    }

    const CHILD_ACCOUNT_COUNT: usize = 3;

    struct TestParentAccount<'a, 'b, 't> {
        _data: &'a [u8],
        pub pubkeys: [ElusivOption<Pubkey>; CHILD_ACCOUNT_COUNT],
        pub accounts: Vec<Option<&'b AccountInfo<'t>>>,
    }

    impl<'a, 'b, 't> PDAAccount for TestParentAccount<'a, 'b, 't> {
        const PROGRAM_ID: Pubkey = crate::PROGRAM_ID;
        const SEED: &'static [u8] = b"ABC";
        const FIRST_PDA: (Pubkey, u8) = (
            Pubkey::new_from_array([
                68, 179, 231, 162, 105, 190, 164, 236, 219, 59, 110, 153, 250, 190, 228, 201, 206,
                98, 34, 111, 200, 139, 69, 232, 47, 91, 47, 54, 136, 144, 12, 62,
            ]),
            0,
        );

        #[cfg(feature = "elusiv-client")]
        const IDENT: &'static str = "TestParentAccount";
    }

    impl<'a, 'b, 't> SizedAccount for TestParentAccount<'a, 'b, 't> {
        const SIZE: usize = 0;
    }

    impl<'a, 'b, 't> ProgramAccount<'a> for TestParentAccount<'a, 'b, 't> {
        fn new(_data: &'a mut [u8]) -> Result<Self, ProgramError> {
            Ok(Self {
                _data,
                pubkeys: [ElusivOption::None; CHILD_ACCOUNT_COUNT],
                accounts: vec![None; CHILD_ACCOUNT_COUNT],
            })
        }
    }

    impl<'a, 'b, 't> ParentAccount<'a, 'b, 't> for TestParentAccount<'a, 'b, 't> {
        const COUNT: usize = CHILD_ACCOUNT_COUNT;
        type Child = TestChildAccount;

        unsafe fn get_child_account_unsafe(
            &self,
            child_index: usize,
        ) -> Result<&AccountInfo<'t>, ProgramError> {
            match self.accounts[child_index] {
                Some(account) => Ok(account),
                None => Err(ProgramError::NotEnoughAccountKeys),
            }
        }

        fn set_child_accounts(parent: &mut Self, child_accounts: Vec<Option<&'b AccountInfo<'t>>>) {
            parent.accounts.copy_from_slice(&child_accounts)
        }

        fn get_child_pubkey(&self, index: usize) -> Option<Pubkey> {
            self.pubkeys[index].option()
        }

        fn set_child_pubkey(&mut self, index: usize, pubkey: ElusivOption<Pubkey>) {
            self.pubkeys[index] = pubkey
        }
    }

    #[test]
    #[should_panic]
    fn test_get_child_account_unsafe() {
        parent_account!(account, TestParentAccount);
        unsafe {
            _ = account.get_child_account_unsafe(3);
        }
    }

    #[test]
    fn test_execute_on_child_account() {
        parent_account!(account, TestParentAccount);

        for i in 0..CHILD_ACCOUNT_COUNT {
            account
                .execute_on_child_account_mut(i, |data| {
                    data[0] = i as u8 + 1;
                })
                .unwrap();
        }

        for i in 0..CHILD_ACCOUNT_COUNT {
            assert_eq!(account.accounts[i].unwrap().data.borrow()[1], i as u8 + 1);
        }

        for i in 0..CHILD_ACCOUNT_COUNT {
            account
                .execute_on_child_account(i, |data| {
                    assert_eq!(data[0], i as u8 + 1);
                })
                .unwrap();
        }
    }

    fn test_find(
        pubkey_is_setup: [bool; CHILD_ACCOUNT_COUNT],
        provided_accounts: Vec<Option<usize>>,
        expected_accounts: Vec<usize>,
    ) {
        parent_account!(mut parent, TestParentAccount);
        for (i, &is_setup) in pubkey_is_setup.iter().enumerate() {
            if is_setup {
                parent.set_child_pubkey(i, ElusivOption::Some(*parent.accounts[i].unwrap().key));
            }
        }

        account_info!(unused_account, Pubkey::new_unique(), vec![1, 0]);
        let account_info_iter = &mut provided_accounts.iter().map(|i| match i {
            Some(i) => parent.accounts[*i].unwrap(),
            None => &unused_account,
        });

        let matched_accounts =
            TestParentAccount::find_child_accounts(&parent, &crate::ID, false, account_info_iter)
                .unwrap();

        assert_eq!(matched_accounts.len(), TestParentAccount::COUNT);

        let mut indices = Vec::new();
        for (i, value) in matched_accounts.iter().enumerate() {
            if let Some(account) = value {
                assert_eq!(parent.accounts[i].unwrap().key, account.key);

                indices.push(i);
            }
        }

        assert_eq!(indices, expected_accounts);
    }

    #[test]
    fn test_find_child_accounts() {
        // All None
        test_find(
            [false, false, false],
            vec![Some(0), Some(1), Some(2)],
            vec![],
        );

        // First account set
        test_find(
            [true, false, false],
            vec![Some(0), Some(1), Some(2)],
            vec![0],
        );

        // Middle account set
        test_find(
            [false, true, false],
            vec![Some(0), Some(1), Some(2)],
            vec![1],
        );

        // Last account set
        test_find(
            [false, false, true],
            vec![Some(0), Some(1), Some(2)],
            vec![2],
        );

        // Different account at start
        test_find(
            [true, true, true],
            vec![None, Some(0), Some(1), Some(2)],
            vec![],
        );

        // Wrong order
        test_find([true, true, true], vec![Some(2), Some(1), Some(0)], vec![2]);

        // Correct order
        test_find(
            [true, true, true],
            vec![Some(0), Some(1), Some(2)],
            vec![0, 1, 2],
        );

        // Accounts at end ignored
        test_find(
            [true, true, true],
            vec![Some(0), Some(1), Some(2), None, None],
            vec![0, 1, 2],
        );
    }

    #[test]
    fn test_unverified_account_info() {
        account_info!(account, Pubkey::new_unique());
        let mut unverified_account_info = UnverifiedAccountInfo::new(&account);

        assert_eq!(
            unverified_account_info.get_safe().unwrap_err(),
            ProgramError::AccountBorrowFailed
        );

        unverified_account_info.get_unsafe();

        assert_eq!(
            unverified_account_info.get_safe().unwrap_err(),
            ProgramError::AccountBorrowFailed
        );

        unverified_account_info.get_unsafe_and_set_is_verified();

        assert!(unverified_account_info.get_safe().is_ok());
    }
}
