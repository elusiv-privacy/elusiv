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

/// Creates an [`solana_program::account_info::AccountInfo`] for testing
/// 
/// # Usage
/// 
/// - `test_account_info!($id: ident)`
/// - `test_account_info!($id: ident, $data_size: expr)`
/// - `test_account_info!($id: ident, $data_size: expr, $owner: expr)`
#[cfg(test)]
macro_rules! test_account_info {
    ($id: ident) => {
        let pk = solana_program::pubkey::Pubkey::new_unique();
        crate::macros::account!($id, pk, vec![]) 
    };
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
macro_rules! test_pda_account_info {
    ($id: ident, $ty: ty) => {
        crate::macros::test_pda_account_info!($id, $ty, None)
    };
    ($id: ident, $ty: ty, $offset: expr) => {
        let (pk, bump) = <$ty as elusiv_types::PDAAccount>::find($offset);
        crate::macros::account!($id, pk, vec![bump]) 
    };
}

/// Creates a instance `$id` of a [`elusiv_types::ProgramAccount`], specified by `$ty`
/// 
/// # Usage
/// 
/// - `zero_account!($id: ident, $ty: ty)`
/// - mutable instance: `zero_account!(mut $id: ident, $ty: ty)`
#[cfg(test)]
macro_rules! zero_account {
    (mut $id: ident, $ty: ty) => {
        let mut data = vec![0; <$ty as elusiv_types::SizedAccount>::SIZE];
        let mut $id = <$ty as elusiv_types::ProgramAccount>::new(&mut data).unwrap();
    };
    ($id: ident, $ty: ty) => {
        let mut data = vec![0; <$ty as elusiv_types::SizedAccount>::SIZE];
        let $id = <$ty as elusiv_types::ProgramAccount>::new(&mut data).unwrap();
    };
}

/// Creates a program-token-account for a specific [`elusiv_types::PDAAccount`] and a token-id
/// 
/// # Usage
/// 
/// `program_token_account!($id: ident, $pda_ty: ty, $token_id: expr)`
#[cfg(test)]
macro_rules! program_token_account {
    ($id: ident, $pda_ty: ty, $token_id: expr) => {
        let pk = crate::processor::program_token_account_address::<$pda_ty>($token_id, None).unwrap();
        crate::macros::account!($id, pk, vec![], spl_token::id())
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

#[cfg(test)] pub(crate) use pyth_price_account_info;
#[cfg(test)] pub(crate) use account;
#[cfg(test)] pub(crate) use test_account_info;
#[cfg(test)] pub(crate) use test_pda_account_info;
#[cfg(test)] pub(crate) use zero_account;
#[cfg(test)] pub(crate) use program_token_account;
#[cfg(test)] pub(crate) use storage_account;
#[cfg(test)] pub(crate) use nullifier_account;