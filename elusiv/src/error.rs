use solana_program::program_error::ProgramError;
use std::fmt;

pub type ElusivResult = Result<(), ElusivError>;

#[derive(Copy, Clone, PartialEq)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug))]
pub enum ElusivError {
    InvalidInstructionData,
    InputsMismatch,
    InvalidOtherInstruction,
    InvalidAmount,
    InsufficientFunds,
    InvalidAccount,
    InvalidRecipient,
    InvalidAccountState,
    NonScalarValue,
    MissingChildAccount,
    FeatureNotAvailable,
    UnsupportedToken,
    OracleError,
    DuplicateValue,
    MissingValue,

    // Merkle tree
    InvalidMerkleRoot,

    // Nullifier
    CouldNotInsertNullifier,

    // Commitment
    NoRoomForCommitment,
    InvalidBatchingRate,
    InvalidRecentCommitmentIndex,

    // Proof
    InvalidPublicInputs,
    CouldNotProcessProof,

    // Queue
    QueueIsEmpty,
    QueueIsFull,
    InvalidQueueAccess,

    // Archiving
    UnableToArchiveNullifierAccount,
    MerkleTreeIsNotFullYet,

    // Partial computations
    PartialComputationError,
    ComputationIsNotYetStarted,
    ComputationIsNotYetFinished,
    ComputationIsAlreadyFinished,

    // Fee
    InvalidFee,
    InvalidFeeVersion,

    // Accounts
    ChildAccountAlreadyExists,
    ChildAccouttDoesNotExists,
}

#[cfg(not(tarpaulin_include))]
impl From<ElusivError> for ProgramError {
    fn from(e: ElusivError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

#[cfg(not(tarpaulin_include))]
impl fmt::Display for ElusivError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", *self as u32)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use elusiv_types::TokenError;

    #[test]
    fn test_sdk_error_codes() {
        assert_eq!(ProgramError::Custom(105), TokenError::PriceError.into());
    }
}
