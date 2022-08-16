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

/// Checked two_pow into usize (exp u32)
macro_rules! two_pow {
    ($exp: expr) => {
        match 2usize.checked_pow($exp) {
            Some(v) => v,
            None => panic!()
        }
    };
}

/// mut? $id: ident, $ty: ty, $account_info: ident
macro_rules! pda_account {
    ($id: ident, $ty: ty, $account_info: ident) => {
        let mut data = &mut $account_info.data.borrow_mut()[..];
        let $id = <$ty>::new(&mut data)?;
    };
    (mut $id: ident, $ty: ty, $account_info: ident) => {
        let mut data = &mut $account_info.data.borrow_mut()[..];
        let mut $id = <$ty>::new(&mut data)?;
    };
}

/*macro_rules! log {
    ($msg: expr) => {
        #[cfg(feature = "testing")]
        solana_program::msg!($msg);
    };
    ($($arg:tt)*) => (
        solana_program::msg!($($arg)*)
    );
}*/

#[cfg(test)]
macro_rules! hash_map {
    (internal $id: ident, $x:expr, $y:expr) => {
        $id.insert($x, $y);
    };
    (internal $id: ident, $($x:expr, $y:expr),+) => {
        hash_map!(internal $id, $($i),+)
    };
    ($id: ident, $(($x:expr, $y:expr)),+) => {
        let mut $id = std::collections::HashMap::new(); 
        hash_map!(internal $id, $($x, $y),+)
    };
}

// Test macros
#[cfg(test)]
/// $id: ident, $pubkey: expr, $data: expr, ($owner: expr)?
macro_rules! account {
    ($id: ident, $pubkey: expr, $data: expr) => {
        let pubkey = $pubkey;
        crate::macros::account!($id, pubkey, data, $data, crate::id());
    };
    ($id: ident, $pubkey: expr, $data: expr, $owner: expr) => {
        let pubkey = $pubkey;
        crate::macros::account!($id, pubkey, data, $data, $owner);
    };
    ($id: ident, $pubkey: expr, $data_id: ident, $data: expr, $owner: expr) => {
        let mut lamports = u64::MAX / 2;
        let mut $data_id = $data;
        let owner = $owner;
        let $id = solana_program::account_info::AccountInfo::new(
            &$pubkey,
            false, false, &mut lamports,
            &mut $data_id,
            &owner,
            false,
            0
        );
    };
}

#[cfg(test)]
/// $id: ident, $data_size: expr, $owner: expr?
macro_rules! test_account_info {
    ($id: ident, $data_size: expr) => {
        let pk = solana_program::pubkey::Pubkey::new_unique();
        crate::macros::account!($id, pk, vec![0; $data_size]) 
    };
    ($id: ident, $data_size: expr, $owner: expr) => {
        let pk = solana_program::pubkey::Pubkey::new_unique();
        crate::macros::account!($id, pk, vec![0; $data_size], $owner) 
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

#[cfg(test)]
macro_rules! storage_account {
    (internal $sub_accounts: ident, $data: ident) => {
        let mut $data = vec![0; StorageAccount::SIZE];
        let mut sub_accounts = std::collections::HashMap::new();
        elusiv_proc_macros::repeat!({
            let pk = solana_program::pubkey::Pubkey::new_unique();
            crate::macros::account!(acc_index, pk, vec![0; StorageAccount::ACCOUNT_SIZE]);
            sub_accounts.insert(_index, &acc_index);
        }, 25);
        let $sub_accounts = sub_accounts;
    };

    ($id: ident) => {
        crate::macros::storage_account!(internal sub_accounts, data);
        let $id = StorageAccount::new(&mut data, sub_accounts).unwrap();
    };
    (mut $id: ident) => {
        crate::macros::storage_account!(internal sub_accounts, data);
        let mut $id = StorageAccount::new(&mut data, sub_accounts).unwrap();
    };
}

#[cfg(test)]
macro_rules! nullifier_account {
    (internal $sub_accounts: ident, $data: ident) => {
        let mut $data = vec![0; NullifierAccount::SIZE];
        let mut sub_accounts = std::collections::HashMap::new();
        elusiv_proc_macros::repeat!({
            let pk = solana_program::pubkey::Pubkey::new_unique();
            crate::macros::account!(acc_index, pk, vec![0; NullifierAccount::ACCOUNT_SIZE]);
            sub_accounts.insert(_index, &acc_index);
        }, 16);
        let $sub_accounts = sub_accounts;
    };

    ($id: ident) => {
        crate::macros::nullifier_account!(internal sub_accounts, data);
        let $id = NullifierAccount::new(&mut data, sub_accounts).unwrap();
    };
    (mut $id: ident) => {
        crate::macros::nullifier_account!(internal sub_accounts, data);
        let mut $id = NullifierAccount::new(&mut data, sub_accounts).unwrap();
    };
}

pub(crate) use guard;
pub(crate) use two_pow;
pub(crate) use pda_account;
//pub(crate) use log;

#[cfg(test)] pub(crate) use hash_map;
#[cfg(test)] pub(crate) use account;
#[cfg(test)] pub(crate) use test_account_info;
#[cfg(test)] pub(crate) use zero_account;
#[cfg(test)] pub(crate) use storage_account;
#[cfg(test)] pub(crate) use nullifier_account;