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
macro_rules! account {
    ($id: ident, $pubkey: expr, $data: expr) => {
        crate::macros::account!($id, $pubkey, data, $data);
    };
    ($id: ident, $pubkey: expr, $data_id: ident, $data: expr) => {
        let mut lamports = u64::MAX / 2;
        let mut $data_id = $data;
        let owner = crate::id();
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
macro_rules! test_account_info {
    ($id: ident, $data_size: expr) => {
        let pk = solana_program::pubkey::Pubkey::new_unique();
        crate::macros::account!($id, pk, vec![0; $data_size]) 
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
    (setup $id: ident, $id_data: ident) => {
        // Note: for testing we only use one AccountInfo, since without a ledger they cannot be modified
        crate::macros::test_account_info!(account, StorageAccount::ACCOUNT_SIZE);

        let mut $id = std::collections::HashMap::new();
        let mut $id_data = vec![0; StorageAccount::SIZE];
        for i in 0..StorageAccount::COUNT { $id.insert(i, &account); }
    };
    ($id: ident) => {
        crate::macros::storage_account!(setup map, data);
        let $id = StorageAccount::new(&mut data, map).unwrap();
    };
    (mut $id: ident) => {
        crate::macros::storage_account!(setup map, data);
        let mut $id = StorageAccount::new(&mut data, map).unwrap();
    };
}

#[cfg(test)]
macro_rules! nullifier_account {
    (setup $id: ident, $id_data: ident) => {
        crate::macros::test_account_info!(account, NullifierAccount::ACCOUNT_SIZE);

        let mut $id = std::collections::HashMap::new();
        let mut $id_data = vec![0; NullifierAccount::SIZE];
        for i in 0..NullifierAccount::COUNT { $id.insert(i, &account); }
    };
    ($id: ident) => {
        crate::macros::nullifier_account!(setup map, data);
        let $id = NullifierAccount::new(&mut data, map).unwrap();
    };
    (mut $id: ident) => {
        crate::macros::nullifier_account!(setup map, data);
        let mut $id = NullifierAccount::new(&mut data, map).unwrap();
    };
}

pub(crate) use guard;
pub(crate) use two_pow;

#[cfg(test)] pub(crate) use hash_map;
#[cfg(test)] pub(crate) use account;
#[cfg(test)] pub(crate) use test_account_info;
#[cfg(test)] pub(crate) use zero_account;
#[cfg(test)] pub(crate) use storage_account;
#[cfg(test)] pub(crate) use nullifier_account;