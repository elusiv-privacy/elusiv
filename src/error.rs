use std::fmt;
use solana_program::program_error::ProgramError;

pub type ElusivResult = Result<(), ElusivError>;

#[derive(Copy, Clone, Debug)]
pub enum ElusivError {
    InvalidInstructionData,
    InvalidAmount,
    InsufficientFunds,
    InvalidRecipient,
    InvalidAccount,
    InvalidAccountSize,
    InvalidAccountBalance,
    NonScalarValue,
    MissingSubAccount,

    SenderIsNotSigner,
    SenderIsNotWritable,

    // Merkle tree
    MerkleTreeIsNotInitialized,
    InvalidMerkleTreeAccess,
    InvalidMerkleRoot,

    // Nullifier
    NullifierAlreadyExists,
    NoRoomForNullifier,
    InvalidNullifierAccount,
    NullifierAccountDoesNotExist,

    // Commitment
    CommitmentAlreadyExists,
    NoRoomForCommitment,
    Commitment,
    HashingIsAlreadyComplete,
    InvalidBatchingRate,

    // Proof
    InvalidProof,
    InvalidPublicInputs,
    InvalidFeePayer,
    InvalidTimestamp,
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