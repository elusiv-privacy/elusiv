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

/// Recursive max construction
macro_rules! max {
    ($x: expr) => ($x);
    ($x: expr, $($z: expr),+) => { if $x < max!($($z),*) { max!($($z),*) } else { $x } };
}

macro_rules! pda_account {
    ($ty: ty, $seed: expr) => {
        impl crate::state::program_account::PDAAccount for $ty {
            const SEED: &'static [u8] = $seed;
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
pub(crate) use max;
pub(crate) use pda_account;
pub(crate) use multi_instance_account;