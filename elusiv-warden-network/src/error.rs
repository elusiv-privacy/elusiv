use solana_program::program_error::ProgramError;
use std::fmt;

#[cfg_attr(test, derive(strum::EnumIter))]
#[derive(Copy, Clone, Debug)]
pub enum ElusivWardenNetworkError {
    InvalidSignature,
    InvalidInstructionData,
    InvalidSigner,
    WardenRegistrationError,
    ProposalError,
    VotingError,
    StatsError,
    TimestampError,

    Overflow,
    Underflow,
}

impl From<ElusivWardenNetworkError> for ProgramError {
    fn from(e: ElusivWardenNetworkError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl fmt::Display for ElusivWardenNetworkError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", *self as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn test_error_codes_never_change() {
        // If this test fails at compile time, just add the variant to the `match` block below.
        // If this test fails at runtime, the probable cause is a reorg.
        // Reorganizing the enum will lead to changes in the resulting error codes, making it easy
        // to mistake one error for another.
        for variant in ElusivWardenNetworkError::iter() {
            use ElusivWardenNetworkError::*;
            let expected_code = match variant {
                InvalidSignature => 0,
                InvalidInstructionData => 1,
                InvalidSigner => 2,
                WardenRegistrationError => 3,
                ProposalError => 4,
                VotingError => 5,
                StatsError => 6,
                TimestampError => 7,
                Overflow => 8,
                Underflow => 9,
            };

            assert_eq!(variant as u32, expected_code);
        }
    }
}