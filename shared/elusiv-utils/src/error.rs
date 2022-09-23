use std::fmt;

#[derive(Copy, Clone, Debug)]
pub enum UtilsError {
    AmountTooLowForRentExemption,
    InvalidAccount,
}

impl fmt::Display for UtilsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &format!("{:?}", self))
    }
}