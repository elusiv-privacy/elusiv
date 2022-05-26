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

pub(crate) use guard;
pub(crate) use multi_instance_account;