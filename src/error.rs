use std::fmt;
use solana_program::program_error::ProgramError;

#[derive(Copy, Clone)]
#[derive(Debug)]
pub enum ElusivError {
    InvalidInstructionData, // 0
    InvalidAmount,
    InvalidRecipient,
    InvalidAccount,
    InvalidAccountSize,

    SenderIsNotSigner,  // 5
    SenderIsNotWritable,

    // Merkle tree
    InvalidMerkleTreeAccess,
    InvalidMerkleRoot,

    // Nullifier
    NullifierAlreadyExists,
    NoRoomForNullifier, // 10
    InvalidNullifierAccount,
    NullifierAccountDoesNotExist,

    // Commitment
    CommitmentAlreadyExists,
    NoRoomForCommitment,
    CommitmentComputationIsNotYetFinished,  // 15
    CommitmentComputationIsAlreadyFinished,
    HashingIsAlreadyComplete,
    DidNotFinishHashing,

    // Proof
    InvalidProof,
    InvalidPublicInputs,    // 20
    InvalidVerificationKey,
    CouldNotParseProof,
    CouldNotProcessProof,
    ProofAccountCannotBeReset,
    ProofComputationIsAlreadyFinished,  // 25
    ProofComputationIsNotYetFinished,

    // Queue
    QueueIsEmpty,
    QueueIsFull,

    // Archiving
    UnableToArchiveNullifierAccount,
    MerkleTreeIsNotFullYet, // 30
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