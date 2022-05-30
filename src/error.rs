use std::fmt;
use solana_program::program_error::ProgramError;

#[derive(Copy, Clone, Debug)]
pub enum ElusivError {
    InvalidInstructionData,
    InvalidAmount,
    InvalidRecipient,
    InvalidAccount,
    InvalidAccountSize,
    InvalidAccountBalance,
    NonScalarValue,

    SenderIsNotSigner,
    SenderIsNotWritable,

    // Merkle tree
    InvalidMerkleTreeAccess,
    InvalidMerkleRoot, // 10

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
    DidNotFinishHashing,

    // Proof
    InvalidProof, // 20
    InvalidPublicInputs,
    InvalidFeePayer,
    InvalidTimestamp,
    InvalidVerificationKey,
    CouldNotParseProof,
    CouldNotProcessProof,
    CannotFinalizeUnaryProof,
    CannotFinalizeBinaryProof,

    // Queue
    QueueIsEmpty,
    QueueIsFull, // 30
    ElementIsNotBeingProcessed,
    ElementIsAlreadyBeingProcessed,

    // Archiving
    UnableToArchiveNullifierAccount,
    MerkleTreeIsNotFullYet,

    // Partial computations
    PartialComputationError,
    AccountCannotBeReset,
    ComputationIsNotYetFinished,
    ComputationIsAlreadyFinished,
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