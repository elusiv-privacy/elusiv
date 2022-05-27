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

macro_rules! multi_instance_account {
    ($ty: ty, $max_instances: literal) => {
        impl<'a> crate::state::program_account::MultiInstanceAccount for $ty {
            const MAX_INSTANCES: u64 = $max_instances;
        }
    };
}

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

pub(crate) use guard;
pub(crate) use multi_instance_account;
#[cfg(test)]
pub(crate) use account;