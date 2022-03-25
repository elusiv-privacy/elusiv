use std::fmt;
use solana_program::program_error::ProgramError;

#[derive(Copy, Clone)]
#[derive(Debug)]
pub enum ElusivError {
    InvalidInstruction, // 0

    SenderIsNotSigner, // 1
    SenderIsNotWritable, // 2
    InvalidAmount, // 3
    InvalidProof, // 4
    CouldNotParseProof, // 5
    CouldNotProcessProof, // 6
    InvalidMerkleRoot, // 7

    InvalidAccount, // 8
    InvalidAccountSize, // 9
    CouldNotCreateMerkleTree, // 10

    NullifierAlreadyUsed, // 11
    NoRoomForNullifier, // 12

    CommitmentAlreadyUsed, // 13
    NoRoomForCommitment, // 14

    DidNotFinishHashing, // 15

    InvalidRecipient, // 16
    QueueIsEmpty, // 17
    QueueIsFull, // 18

    InvalidPublicInputs,    // 19
    InvalidVerificationKey,    // 20
    ProofAccountCannotBeReset,    // 21
    ProofComputationIsAlreadyFinished,    // 22
    ProofComputationIsNotYetFinished,    // 23
    InvalidMerkleTreeAccess,
    CommitmentComputationIsNotYetFinished,
    CommitmentComputationIsAlreadyFinished,
    HashingIsAlreadyComplete,
}

impl From<ElusivError> for ProgramError {
    fn from(e: ElusivError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl fmt::Display for ElusivError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidInstruction => write!(f, "InvalidInstruction"),
            Self::SenderIsNotSigner => write!(f, "SenderIsNotSigner"),
            Self::SenderIsNotWritable => write!(f, "SenderIsNotWritable"),
            Self::InvalidAmount => write!(f, "InvalidAmount"),
            Self::InvalidProof => write!(f, "InvalidProof"),
            Self::CouldNotProcessProof => write!(f, "CouldNotProcessProof"),
            Self::InvalidMerkleRoot => write!(f, "InvalidMerkleRoot"),
            Self::InvalidAccount => write!(f, "InvalidAccount"),
            Self::InvalidAccountSize => write!(f, "InvalidAccountSize"),
            Self::CouldNotCreateMerkleTree => write!(f, "CouldNotCreateMerkleTree"),
            Self::NullifierAlreadyUsed => write!(f, "NullifierAlreadyUsed"),
            Self::NoRoomForNullifier => write!(f, "NoRoomForNullifier"),
            Self::CommitmentAlreadyUsed => write!(f, "CommitmentAlreadyUsed"),
            Self::NoRoomForCommitment => write!(f, "NoRoomForCommitment"),
            Self::DidNotFinishHashing => write!(f, "DidNotFinishHashing"),
            Self::InvalidRecipient => write!(f, "InvalidRecipient"),
            Self::CouldNotParseProof => write!(f, "CouldNotParseProof"),
            Self::QueueIsEmpty => write!(f, "QueueIsEmpty"),
            Self::QueueIsFull => write!(f, "QueueIsFull"),
            Self::InvalidPublicInputs => write!(f, "InvalidPublicInputs"),
            Self::InvalidVerificationKey => write!(f, "InvalidVerificationKey"),
            Self::ProofAccountCannotBeReset => write!(f, "ProofAccountCannotBeReset"),
            Self::ProofComputationIsAlreadyFinished => write!(f, "ProofComputationIsAlreadyFinished"),
            Self::ProofComputationIsNotYetFinished => write!(f, "ProofComputationIsNotYetFinished"),
            Self::InvalidMerkleTreeAccess => write!(f, "InvalidMerkleTreeAccess"),
            Self::CommitmentComputationIsNotYetFinished => write!(f, "CommitmentComputationIsNotYetFinished"),
            Self::CommitmentComputationIsAlreadyFinished => write!(f, "CommitmentComputationIsAlreadyFinished"),
            Self::HashingIsAlreadyComplete => write!(f, "HashingIsAlreadyComplete"),
        }
    }
}