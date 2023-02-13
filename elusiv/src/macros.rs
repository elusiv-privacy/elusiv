pub use elusiv_derive::*;
pub use elusiv_proc_macros::*;
pub use elusiv_utils::{guard, pda_account, two_pow};

/// Creates a dummy pyth-price-account [`solana_program::account_info::AccountInfo`] for testing
///
/// # Usage
///
/// `pyth_price_account_info!($id: ident, $token_id: ident, $price: expr)`
#[cfg(test)]
macro_rules! pyth_price_account_info {
    ($id: ident, $token_id: ident, $price: expr) => {
        let data = crate::token::pyth_price_account_data(&$price).unwrap();
        let key = crate::token::TOKENS[$token_id as usize].pyth_usd_price_key;
        crate::macros::account_info!($id, key, data);
    };
}

/// Create a dummy [`solana_program::account_info::AccountInfo`] for testing
///
/// # Usage
///
/// - `account_info!($id: ident, $pubkey: expr)`
/// - `account_info!($id: ident, $pubkey: expr, $is_signer: literal)`
/// - `account_info!($id: ident, $pubkey: expr, $data: expr)`
/// - `account_info!($id: ident, $pubkey: expr, $data: expr, $owner: expr, $is_signer: literal)`
/// - `account_info!($id: ident, $pubkey: expr, $data_id: ident, $data: expr, $owner: expr, $is_signer: literal)`
#[cfg(test)]
macro_rules! account_info {
    ($id: ident, $pubkey: expr) => {
        crate::macros::account_info!($id, $pubkey, false);
    };
    ($id: ident, $pubkey: expr, $is_signer: literal) => {
        let pubkey = $pubkey;
        crate::macros::account_info!($id, pubkey, data, vec![], crate::id(), $is_signer);
    };
    ($id: ident, $pubkey: expr, $data: expr) => {
        let pubkey = $pubkey;
        crate::macros::account_info!($id, pubkey, data, $data, crate::id(), false);
    };
    ($id: ident, $pubkey: expr, $data: expr, $owner: expr, $is_signer: literal) => {
        let pubkey = $pubkey;
        crate::macros::account_info!($id, pubkey, data, $data, $owner, $is_signer);
    };
    ($id: ident, $pubkey: expr, $data_id: ident, $data: expr, $owner: expr, $is_signer: literal) => {
        let mut lamports = u32::MAX as u64;
        let mut $data_id = $data;
        let owner = $owner;
        let $id = solana_program::account_info::AccountInfo::new(
            &$pubkey,
            $is_signer,
            false,
            &mut lamports,
            &mut $data_id,
            &owner,
            false,
            0,
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
        crate::macros::account_info!($id, pk, vec![])
    };
    ($id: ident, $data_size: expr) => {
        let pk = solana_program::pubkey::Pubkey::new_unique();
        crate::macros::account_info!($id, pk, vec![0; $data_size])
    };
    ($id: ident, $data_size: expr, $owner: expr) => {
        let pk = solana_program::pubkey::Pubkey::new_unique();
        crate::macros::account_info!($id, pk, vec![0; $data_size], $owner, false)
    };
}

/// Creates a signing [`solana_program::account_info::AccountInfo`] for testing
///
/// # Usage
///
/// - `test_account_info!($id: ident)`
#[cfg(test)]
macro_rules! signing_test_account_info {
    ($id: ident) => {
        let pk = solana_program::pubkey::Pubkey::new_unique();
        crate::macros::account_info!($id, pk, vec![], crate::id(), true)
    };
}

#[cfg(test)]
macro_rules! test_pda_account_info {
    ($id: ident, $ty: ty) => {
        crate::macros::test_pda_account_info!($id, $ty, None)
    };
    ($id: ident, $ty: ty, $offset: expr) => {
        let (pk, bump) = <$ty as elusiv_types::PDAAccount>::find($offset);
        crate::macros::account_info!($id, pk, vec![bump])
    };
    ($id: ident, $ty: ty, $pubkey: expr, $offset: expr) => {
        let (pk, bump) = <$ty as elusiv_types::PDAAccount>::find_with_pubkey($pubkey, $offset);
        crate::macros::account_info!($id, pk, vec![bump])
    };
}

/// Creates a program-token-account for a specific [`elusiv_types::PDAAccount`] and a token-id
///
/// # Usage
///
/// `program_token_account_info!($id: ident, $pda_ty: ty, $token_id: expr)`
#[cfg(test)]
macro_rules! program_token_account_info {
    ($id: ident, $pda_ty: ty, $token_id: expr) => {
        let pk =
            crate::processor::program_token_account_address::<$pda_ty>($token_id, None).unwrap();
        crate::macros::account_info!($id, pk, vec![], spl_token::id(), false)
    };
}

/// Creates an instance `$id` of a [`elusiv_types::ProgramAccount`], specified by `$ty`
///
/// # Usage
///
/// - `zero_program_account!($id: ident, $ty: ty)`
/// - mutable instance: `zero_program_account!(mut $id: ident, $ty: ty)`
#[cfg(test)]
macro_rules! zero_program_account {
    (mut $id: ident, $ty: ty) => {
        let mut data = vec![0; <$ty as elusiv_types::SizedAccount>::SIZE];
        let mut $id = <$ty as elusiv_types::ProgramAccount>::new(&mut data).unwrap();
    };
    ($id: ident, $ty: ty) => {
        let mut data = vec![0; <$ty as elusiv_types::SizedAccount>::SIZE];
        let $id = <$ty as elusiv_types::ProgramAccount>::new(&mut data).unwrap();
    };
}

/// Creates an instance `$id` of a `$ty` implementing [`elusiv_types::accounts::ParentAccount`]
///
/// # Notes
///
/// - This only works for up to 32 child-accounts.
///
/// # Usage
///
/// - `parent_account!($id: ident, $ty: ty)`
/// - mutable instance: `parent_account!(mut $id: ident, $ty: ty)`
#[cfg(test)]
macro_rules! parent_account {
    (internal $ty: ty, $child_accounts: ident, $data: ident) => {
        let mut $data = vec![0; <$ty as elusiv_types::accounts::SizedAccount>::SIZE];

        let mut child_accounts = Vec::with_capacity(<$ty as elusiv_types::accounts::ParentAccount>::COUNT);
        elusiv_proc_macros::repeat!({
            let pk = solana_program::pubkey::Pubkey::new_unique();
            crate::macros::account_info!(acc_index, pk, vec![0; <<$ty as elusiv_types::accounts::ParentAccount>::Child as elusiv_types::accounts::SizedAccount>::SIZE]);
            child_accounts.push(Some(&acc_index));
        }, 32);

        let $child_accounts = child_accounts[..<$ty as elusiv_types::accounts::ParentAccount>::COUNT].to_vec();
    };

    ($id: ident, $ty: ty) => {
        crate::macros::parent_account!(internal $ty, child_accounts, data);
        let $id = <$ty as elusiv_types::ParentAccount>::new_with_child_accounts(&mut data, child_accounts).unwrap();
    };
    (mut $id: ident, $ty: ty) => {
        crate::macros::parent_account!(internal $ty, child_accounts, data);
        let mut $id = <$ty as elusiv_types::ParentAccount>::new_with_child_accounts(&mut data, child_accounts).unwrap();
    };
}

#[cfg(test)]
pub(crate) use account_info;
#[cfg(test)]
pub(crate) use parent_account;
#[cfg(test)]
pub(crate) use program_token_account_info;
#[cfg(test)]
pub(crate) use pyth_price_account_info;
#[cfg(test)]
pub(crate) use signing_test_account_info;
#[cfg(test)]
pub(crate) use test_account_info;
#[cfg(test)]
pub(crate) use test_pda_account_info;
#[cfg(test)]
pub(crate) use zero_program_account;
