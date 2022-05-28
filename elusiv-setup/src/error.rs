use std::fmt;

#[derive(Copy, Clone, Debug)]
pub enum UtilsError {
    AmountTooLowForRentExemption,
}

impl fmt::Display for UtilsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &format!("{:?}", self))
    }
}