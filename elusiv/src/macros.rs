pub use elusiv_derive::*;
pub use elusiv_proc_macros::*;
pub use elusiv_utils::{
    guard,
    two_pow,
    pda_account,
};

#[cfg(test)]
macro_rules! pyth_price_account_info {
    ($id: ident, $token_id: ident, $price: expr) => {
        let data = crate::token::pyth_price_account_data(&$price).unwrap();
        let key = crate::token::TOKENS[$token_id as usize].pyth_usd_price_key;
        crate::macros::account!($id, key, data);
    };
}

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
        let mut lamports = u32::MAX as u64;
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
        let mut $data = vec![0; <NullifierAccount as elusiv_types::accounts::SizedAccount>::SIZE];
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

#[cfg(test)]
macro_rules! token_pda_account {
    ($id: ident, $token_account_id: ident, $ty: ty, $token_id: expr) => {
        test_account_info!($token_account_id, 0, spl_token::id());
        let mut data = vec![0; <$ty>::SIZE];
        let mut pool = <$ty>::new(&mut data).unwrap();
        pool.set_accounts($token_id as usize - 1, &ElusivOption::Some($token_account_id.key.to_bytes()));
        account!($id, <$ty>::find(None).0, data); 
    };
}

//#[cfg(test)] pub(crate) use hash_map;
#[cfg(test)] pub(crate) use pyth_price_account_info;
#[cfg(test)] pub(crate) use account;
#[cfg(test)] pub(crate) use test_account_info;
#[cfg(test)] pub(crate) use zero_account;
#[cfg(test)] pub(crate) use storage_account;
#[cfg(test)] pub(crate) use nullifier_account;
#[cfg(test)] pub(crate) use token_pda_account;