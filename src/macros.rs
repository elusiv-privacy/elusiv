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

macro_rules! multi_instance_account {
    ($ty: ty, $max_instances: literal) => {
        impl<'a> crate::state::program_account::MultiInstanceAccount for $ty {
            const MAX_INSTANCES: u64 = $max_instances;
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
macro_rules! generate_storage_accounts {
    ($arr: ident, $s: expr) => {
        let mut pks = Vec::new();
        for _ in 0..StorageAccount::COUNT { pks.push(Pubkey::new_unique()); }

        account!(a0, pks[0], vec![0; $s[0]]);
        account!(a1, pks[1], vec![0; $s[1]]);
        account!(a2, pks[2], vec![0; $s[2]]);
        account!(a3, pks[3], vec![0; $s[3]]);
        account!(a4, pks[4], vec![0; $s[4]]);
        account!(a5, pks[5], vec![0; $s[5]]);
        account!(a6, pks[6], vec![0; $s[6]]);

        let $arr = [a0, a1, a2, a3, a4, a5, a6];
    };
}

#[cfg(extended_logging)]
macro_rules! capture_compute_units_a {
    () => {
        solana_program::msg!("Capture A:");
        solana_program::log::sol_log_compute_units();
    };
}

#[cfg(extended_logging)]
macro_rules! capture_compute_units_b {
    () => {
        solana_program::msg!("Capture B:");
        solana_program::log::sol_log_compute_units();
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

pub(crate) use guard;
pub(crate) use multi_instance_account;
pub(crate) use two_pow;

#[cfg(extended_logging)] pub(crate) use capture_compute_units_a;
#[cfg(extended_logging)] pub(crate) use capture_compute_units_b;

#[cfg(test)] pub(crate) use account;
#[cfg(test)] pub(crate) use generate_storage_accounts;
#[cfg(test)] pub(crate) use generate_storage_accounts_valid_size;