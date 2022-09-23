use std::fmt;
use solana_program::program_error::ProgramError;

pub type ElusivResult = Result<(), ElusivError>;

#[derive(Copy, Clone, Debug)]
pub enum ElusivError {
    InvalidInstructionData,
    InvalidAmount,
    InsufficientFunds,
    InvalidAccount,
    InvalidAccountState,
    NonScalarValue,
    MissingSubAccount,
    FeatureNotAvailable,
    UnsupportedToken,
    OracleError,

    // Merkle tree
    InvalidMerkleRoot,

    // Nullifier
    CouldNotInsertNullifier,

    // Commitment
    NoRoomForCommitment,
    InvalidBatchingRate,

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
    AccountCannotBeReset,
    ComputationIsNotYetStarted,
    ComputationIsNotYetFinished,
    ComputationIsAlreadyFinished,

    // Fee
    InvalidFee,
    InvalidFeeVersion,

    // Accounts
    SubAccountAlreadyExists,
    SubAccouttDoesNotExists,
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