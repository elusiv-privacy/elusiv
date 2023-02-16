use solana_program::program_error::ProgramError;
use std::fmt;

#[derive(Copy, Clone, Debug)]
pub enum ElusivWardenNetworkError {
    InvalidSignature = 0x00,
    InvalidInstructionData = 0x01,
    InvalidSigner = 0x02,
    WardenRegistrationError = 0x03,
    ProposalError = 0x04,
    VotingError = 0x05,
    StatsError = 0x06,
    TimestampError = 0x07,

    Overflow = 0x08,
    Underflow = 0x09,
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
