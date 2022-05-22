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

pub(crate) use guard;
pub(crate) use max;