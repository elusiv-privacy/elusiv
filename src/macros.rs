pub use elusiv_macros::*;

/// Guard statement
/// - if the assertion evaluates to false, the error is raised
macro_rules! guard {
    ($assertion: expr, $error: expr) => {
        if !$assertion {
            return Err($error.into())
        } 
    };
}

/// Generates a PDA (Pubkey) from a seed
macro_rules! pda {
    ($seed: expr) => {
        solana_program::pubkey::Pubkey::find_program_address($seed, &crate::id()).0
    };
}

/// Checks that the AccountInfo's key matches the seeded PDA
macro_rules! guard_pda_account {
    ($account: expr, $seed: expr) => {
        guard!(
            solana_program::pubkey::Pubkey::find_program_address($seed, &crate::id()).0 == *$account.key,
            crate::error::ElusivError::InvalidAccount
        );
    };
}

/// Returns a mutable reference to an accounts data
macro_rules! account_data_mut {
    ($account_info: expr) => {
        &mut $account_info.data.borrow_mut()[..]
    };
}

/// Returns a reference to an accounts data
macro_rules! account_data {
    ($account_info: expr) => {
        &$account_info.data.borrow_mut()[..]
    };
}

/// Writes the bytes from an iterator into an writer (overriding any existing bytes)
macro_rules! write_into {
    ($writer: expr, $bytes: expr) => {
        for (i, &byte) in $bytes.iter().enumerate() {
            $writer[i] = byte;
        }
    };
}

/// Raises two to the power of the supplied exponent
macro_rules! two_pow {
    ($exponent: expr) => {
        1 << ($exponent)
    };
}

pub(crate) use guard;
pub(crate) use pda;
pub(crate) use guard_pda_account;
pub(crate) use account_data_mut;
pub(crate) use account_data;
pub(crate) use write_into;
pub(crate) use two_pow;