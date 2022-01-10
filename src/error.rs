use solana_program::program_error::ProgramError;
use std::fmt;

#[derive(Copy, Clone)]
pub enum ElusivError {
    InvalidInstruction,

    SenderIsNotSigner,
    SenderIsNotWritable,
    InvalidAmount,
    InvalidProof,
    CouldNotProcessProof,
    InvalidMerkleRoot,

    InvalidStorageAccount,
    InvalidStorageAccountSize,
    CouldNotCreateMerkleTree,

    NullifierAlreadyUsed,
    NoRoomForNullifier,

    CommitmentAlreadyUsed,
    NoRoomForCommitment,
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
        }
    }
}