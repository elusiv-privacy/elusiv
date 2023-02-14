/// Guard statement
/// - if the assertion evaluates to false, the error is raised
#[macro_export]
macro_rules! guard {
    ($assertion: expr, $error: expr) => {
        if !$assertion {
            return Err($error.into());
        }
    };
}

/// Checked two_pow into usize (exp u32)
#[macro_export]
macro_rules! two_pow {
    ($exp: expr) => {
        match 2usize.checked_pow($exp) {
            Some(v) => v,
            None => panic!(),
        }
    };
}

/// mut? $id: ident, $ty: ty, $account_info: ident
#[macro_export]
macro_rules! pda_account {
    ($id: ident, $ty: ty, $account_info: expr) => {
        let mut data = &mut $account_info.data.borrow_mut()[..];
        let $id = <$ty as elusiv_types::accounts::ProgramAccount>::new(&mut data)?;
    };
    (mut $id: ident, $ty: ty, $account_info: expr) => {
        let mut data = &mut $account_info.data.borrow_mut()[..];
        let mut $id = <$ty as elusiv_types::accounts::ProgramAccount>::new(&mut data)?;
    };
}
