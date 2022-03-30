use std::fmt;
use solana_program::program_error::ProgramError;

#[derive(Copy, Clone)]
#[derive(Debug)]
pub enum ElusivError {
    InvalidInstructionData,
    InvalidAmount,
    InvalidRecipient,
    InvalidAccount,
    InvalidAccountSize,

    SenderIsNotSigner,
    SenderIsNotWritable,

    // Merkle tree
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
    CommitmentComputationIsNotYetFinished,
    CommitmentComputationIsAlreadyFinished,
    HashingIsAlreadyComplete,
    DidNotFinishHashing,

    // Proof
    InvalidProof,
    InvalidPublicInputs,
    InvalidVerificationKey,
    CouldNotParseProof,
    CouldNotProcessProof,
    ProofAccountCannotBeReset,
    ProofComputationIsAlreadyFinished,
    ProofComputationIsNotYetFinished,

    // Queue
    QueueIsEmpty,
    QueueIsFull,

    // Archiving
    UnableToArchiveNullifierAccount,
    MerkleTreeIsNotFullYet,
}

impl From<ElusivError> for ProgramError {
    fn from(e: ElusivError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl fmt::Display for ElusivError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &format!("{:?}", self))
    }
}