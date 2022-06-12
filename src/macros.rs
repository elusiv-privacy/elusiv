pub use elusiv_derive::*;
pub use elusiv_proc_macros::*;

/// Guard statement
/// - if the assertion evaluates to false, the error is raised
macro_rules! guard {
    ($assertion: expr, $error: expr) => {
        if !$assertion {
            return Err($error.into())
        } 
    };
}

macro_rules! two_pow {
    ($exp: expr) => {
        match 2usize.checked_pow($exp) {
            Some(v) => v,
            None => panic!()
        }
    };
}

// Test macros
#[cfg(test)]
macro_rules! account {
    ($id: ident, $pubkey: expr, $data: expr) => {
        let mut lamports = u64::MAX / 2;
        let mut data = $data;
        let owner = crate::id();
        let $id = AccountInfo::new(
            &$pubkey,
            false, false, &mut lamports,
            &mut data,
            &owner,
            false,
            0
        );
    };
}

#[cfg(test)]
macro_rules! test_account_info {
    ($id: ident, $data_size: expr) => {
        let pk = solana_program::pubkey::Pubkey::new_unique();
        account!($id, pk, vec![0; $data_size]) 
    };
}

#[cfg(test)]
macro_rules! generate_storage_accounts {
    ($arr: ident, $s: expr) => {
        let mut pks = Vec::new();
        for _ in 0..StorageAccount::COUNT { pks.push(solana_program::pubkey::Pubkey::new_unique()); }

        account!(a0, pks[0], vec![0; $s[0]]);
        account!(a1, pks[1], vec![0; $s[1]]);
        account!(a2, pks[2], vec![0; $s[2]]);
        account!(a3, pks[3], vec![0; $s[3]]);
        account!(a4, pks[4], vec![0; $s[4]]);
        account!(a5, pks[5], vec![0; $s[5]]);
        account!(a6, pks[6], vec![0; $s[6]]);

        let $arr = vec![&a0, &a1, &a2, &a3, &a4, &a5, &a6];
    };
}

#[cfg(test)]
macro_rules! generate_storage_accounts_valid_size {
    ($arr: ident) => {
        generate_storage_accounts!($arr, [
            StorageAccount::INTERMEDIARY_ACCOUNT_SIZE,
            StorageAccount::INTERMEDIARY_ACCOUNT_SIZE,
            StorageAccount::INTERMEDIARY_ACCOUNT_SIZE,
            StorageAccount::INTERMEDIARY_ACCOUNT_SIZE,
            StorageAccount::INTERMEDIARY_ACCOUNT_SIZE,
            StorageAccount::INTERMEDIARY_ACCOUNT_SIZE,
            StorageAccount::LAST_ACCOUNT_SIZE,
        ]);
    };
}

#[cfg(test)]
macro_rules! zero_account {
    (mut $id: ident, $ty: ty) => {
        let mut data = vec![0; <$ty>::SIZE];
        let mut $id = <$ty>::new(&mut data).unwrap();
    };
    ($id: ident, $ty: ty) => {
        let mut data = vec![0; <$ty>::SIZE];
        let $id = <$ty>::new(&mut data).unwrap();
    };
}

pub(crate) use guard;
pub(crate) use two_pow;

#[cfg(test)] pub(crate) use account;
#[cfg(test)] pub(crate) use test_account_info;
#[cfg(test)] pub(crate) use zero_account;
#[cfg(test)] pub(crate) use generate_storage_accounts;
#[cfg(test)] pub(crate) use generate_storage_accounts_valid_size;