use std::fmt;
use solana_program::program_error::ProgramError;

#[derive(Copy, Clone)]
pub enum ElusivError {
    InvalidInstruction, // 0

    SenderIsNotSigner, // 1
    SenderIsNotWritable, // 2
    InvalidAmount, // 3
    InvalidProof, // 4
    CouldNotProcessProof, // 5
    InvalidMerkleRoot, // 6

    InvalidStorageAccount, // 7
    InvalidStorageAccountSize, // 8
    CouldNotCreateMerkleTree, // 9

    NullifierAlreadyUsed, // 10
    NoRoomForNullifier, // 11

    CommitmentAlreadyUsed, // 12
    NoRoomForCommitment, // 13

    DidNotFinishHashing, // 14

    ExplicitLogError, // 15
}

impl From<ElusivError> for ProgramError {
    fn from(e: ElusivError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl fmt::Display for ElusivError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidInstruction =>
                write!(f, "InvalidInstruction"),
            Self::SenderIsNotSigner =>
                write!(f, "SenderIsNotSigner"),
            Self::SenderIsNotWritable =>
                write!(f, "SenderIsNotWritable"),
            Self::InvalidAmount =>
                write!(f, "InvalidAmount"),
            Self::InvalidProof =>
                write!(f, "InvalidProof"),
            Self::CouldNotProcessProof =>
                write!(f, "CouldNotProcessProof"),
            Self::InvalidMerkleRoot =>
                write!(f, "InvalidMerkleRoot"),
            Self::InvalidStorageAccount =>
                write!(f, "InvalidStorageAccount"),
            Self::InvalidStorageAccountSize =>
                write!(f, "InvalidStorageAccountSize"),
            Self::CouldNotCreateMerkleTree =>
                write!(f, "CouldNotCreateMerkleTree"),
            Self::NullifierAlreadyUsed =>
                write!(f, "NullifierAlreadyUsed"),
            Self::NoRoomForNullifier =>
                write!(f, "NoRoomForNullifier"),
            Self::CommitmentAlreadyUsed =>
                write!(f, "CommitmentAlreadyUsed"),
            Self::NoRoomForCommitment =>
                write!(f, "NoRoomForCommitment"),
            Self::DidNotFinishHashing =>
                write!(f, "DidNotFinishHashing"),
            Self::ExplicitLogError =>
                write!(f, "ExplicitLogError"),
        }
    }
}